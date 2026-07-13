//! Local constrained armor remeshing. Only a connected patch around one aperture is rebuilt; the
//! shared source mesh stays immutable and the contour comes from the same deterministic descriptor
//! used by collision and analytical clipping.

use std::collections::HashMap;

use game_core::{ApertureLobe, armor_surface_basis};
use glam::{Vec2, Vec3};

use crate::{GeometryMesh, GeometryVertex};

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum DamageRemeshError {
    #[error("no surface triangle contains or surrounds the aperture")]
    SurfaceNotFound,
    #[error("selected patch does not have one simple boundary")]
    InvalidBoundary,
    #[error("constrained triangulation produced no steel patch")]
    TriangulationFailed,
}

/// Rebuild one local surface patch with the aperture as a true polygonal hole. The returned mesh
/// owns new topology; `source` is never changed.
pub fn remesh_aperture(
    source: &GeometryMesh,
    lobe: ApertureLobe,
) -> Result<GeometryMesh, DamageRemeshError> {
    let normal = lobe.entry_normal_local.normalize_or_zero();
    let (u, v) = armor_surface_basis(normal, lobe.direction_local);
    if normal == Vec3::ZERO || u == Vec3::ZERO || v == Vec3::ZERO {
        return Err(DamageRemeshError::SurfaceNotFound);
    }
    let triangles: Vec<[u32; 3]> =
        source.indices().chunks_exact(3).map(|tri| [tri[0], tri[1], tri[2]]).collect();
    let candidate: Vec<bool> = triangles
        .iter()
        .map(|tri| triangle_is_in_patch(source, *tri, lobe, normal, u, v))
        .collect();
    let Some(seed) = candidate.iter().position(|selected| *selected) else {
        return Err(DamageRemeshError::SurfaceNotFound);
    };
    let selected = connected_selection(&triangles, &candidate, seed);
    let boundary = patch_boundary(&triangles, &selected)?;
    if boundary.len() < 3 {
        return Err(DamageRemeshError::InvalidBoundary);
    }

    let mut points: Vec<[f32; 2]> = boundary
        .iter()
        .map(|&index| {
            project_to_surface(source.vertices()[index as usize].position, lobe.entry_local, u, v)
        })
        .collect();
    let hole_start = points.len();
    const CONTOUR_SEGMENTS: usize = 32;
    for segment in 0..CONTOUR_SEGMENTS {
        let angle = segment as f32 / CONTOUR_SEGMENTS as f32 * std::f32::consts::TAU;
        points.push(lobe.outer.point_at(angle, lobe.fracture_seed).to_array());
    }
    let mut triangulator = earcut::Earcut::<f32>::new();
    let mut patch_indices = Vec::new();
    triangulator.earcut(points.iter().copied(), &[hole_start], &mut patch_indices);
    if patch_indices.is_empty() {
        return Err(DamageRemeshError::TriangulationFailed);
    }

    let template = source.vertices()[triangles[seed][0] as usize];
    let mut vertices = source.vertices().to_vec();
    let hole_vertex_start = vertices.len() as u32;
    for point in &points[hole_start..] {
        let point = Vec2::from_array(*point);
        vertices.push(GeometryVertex {
            position: lobe.entry_local + u * point.x + v * point.y,
            normal,
            ..template
        });
    }
    let map_index = |index: usize| -> u32 {
        if index < hole_start {
            boundary[index]
        } else {
            hole_vertex_start + (index - hole_start) as u32
        }
    };
    let mut indices = Vec::with_capacity(source.indices().len() + patch_indices.len());
    for (index, tri) in triangles.iter().enumerate() {
        if !selected[index] {
            indices.extend_from_slice(tri);
        }
    }
    for tri in patch_indices.chunks_exact(3) {
        let mut mapped = [map_index(tri[0]), map_index(tri[1]), map_index(tri[2])];
        let [a, b, c] = mapped.map(|index| vertices[index as usize].position);
        if (b - a).cross(c - a).dot(normal) < 0.0 {
            mapped.swap(1, 2);
        }
        indices.extend_from_slice(&mapped);
    }
    Ok(GeometryMesh::new(vertices, indices))
}

fn triangle_is_in_patch(
    mesh: &GeometryMesh,
    tri: [u32; 3],
    lobe: ApertureLobe,
    normal: Vec3,
    u: Vec3,
    v: Vec3,
) -> bool {
    let p = tri.map(|index| mesh.vertices()[index as usize].position);
    let face_normal = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
    if face_normal.dot(normal) < 0.28 {
        return false;
    }
    let center = (p[0] + p[1] + p[2]) / 3.0;
    if (center - lobe.entry_local).dot(normal).abs() > 0.09 {
        return false;
    }
    let radius = lobe.outer.major_radius_m.max(lobe.outer.minor_radius_m) * 1.65 + 0.035;
    let near = p.into_iter().chain([center]).any(|point| {
        let projected = Vec2::from_array(project_to_surface(point, lobe.entry_local, u, v));
        projected.length() <= radius
    });
    near || point_in_projected_triangle(lobe.entry_local, p, u, v)
}

fn connected_selection(tris: &[[u32; 3]], candidate: &[bool], seed: usize) -> Vec<bool> {
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (index, tri) in tris.iter().enumerate().filter(|(index, _)| candidate[*index]) {
        for edge in triangle_edges(*tri) {
            edges.entry(sorted_edge(edge)).or_default().push(index);
        }
    }
    let mut selected = vec![false; tris.len()];
    let mut stack = vec![seed];
    while let Some(index) = stack.pop() {
        if selected[index] {
            continue;
        }
        selected[index] = true;
        for edge in triangle_edges(tris[index]) {
            if let Some(neighbors) = edges.get(&sorted_edge(edge)) {
                stack.extend(neighbors.iter().copied().filter(|next| !selected[*next]));
            }
        }
    }
    selected
}

fn patch_boundary(tris: &[[u32; 3]], selected: &[bool]) -> Result<Vec<u32>, DamageRemeshError> {
    let mut count: HashMap<(u32, u32), u8> = HashMap::new();
    for tri in tris.iter().zip(selected).filter_map(|(tri, selected)| selected.then_some(*tri)) {
        for edge in triangle_edges(tri) {
            *count.entry(sorted_edge(edge)).or_default() += 1;
        }
    }
    let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
    for ((a, b), edge_count) in count {
        if edge_count == 1 {
            adjacency.entry(a).or_default().push(b);
            adjacency.entry(b).or_default().push(a);
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_unstable();
    }
    if adjacency.is_empty() || adjacency.values().any(|neighbors| neighbors.len() != 2) {
        return Err(DamageRemeshError::InvalidBoundary);
    }
    let start = *adjacency.keys().min().expect("nonempty boundary");
    let mut result = vec![start];
    let mut previous = u32::MAX;
    let mut current = start;
    loop {
        let next = *adjacency[&current]
            .iter()
            .find(|neighbor| **neighbor != previous)
            .ok_or(DamageRemeshError::InvalidBoundary)?;
        if next == start {
            break;
        }
        if result.len() > adjacency.len() {
            return Err(DamageRemeshError::InvalidBoundary);
        }
        result.push(next);
        previous = current;
        current = next;
    }
    (result.len() == adjacency.len()).then_some(result).ok_or(DamageRemeshError::InvalidBoundary)
}

fn triangle_edges(tri: [u32; 3]) -> [(u32, u32); 3] {
    [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])]
}

fn sorted_edge((a, b): (u32, u32)) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

fn project_to_surface(point: Vec3, origin: Vec3, u: Vec3, v: Vec3) -> [f32; 2] {
    let delta = point - origin;
    [delta.dot(u), delta.dot(v)]
}

fn point_in_projected_triangle(point: Vec3, tri: [Vec3; 3], u: Vec3, v: Vec3) -> bool {
    let p = Vec2::ZERO;
    let projected = tri.map(|vertex| Vec2::from_array(project_to_surface(vertex, point, u, v)));
    let signs = (0..3).map(|i| {
        let a = projected[i];
        let b = projected[(i + 1) % 3];
        (b - a).perp_dot(p - a)
    });
    let mut positive = false;
    let mut negative = false;
    for sign in signs {
        positive |= sign > 1.0e-5;
        negative |= sign < -1.0e-5;
    }
    !(positive && negative)
}
