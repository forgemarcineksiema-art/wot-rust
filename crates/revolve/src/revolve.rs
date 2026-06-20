//! Surface-of-revolution generator: the third part-generator, for round parts (gun barrels, road
//! wheels, rollers, sprockets). Neither SDF nor CAD — a direct parametric mesh, cheap and clean,
//! the pattern the old kernel sketched as `RevolveSpec`.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

/// Revolve a profile of `(offset_along_axis, radius)` points around `axis` into `segments` wedges.
/// Rings of radius 0 collapse onto the axis to form caps; `weld_and_smooth` then fuses the seam and
/// rebuilds round normals, yielding a closed surface of revolution.
pub fn revolve(
    axis: Vec3,
    profile: &[(f32, f32)],
    segments: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let axis = axis.normalize();
    let u = tangent(axis);
    let w = axis.cross(u);
    let seg = segments.max(3);

    let mut vertices = Vec::with_capacity(profile.len() * seg);
    for &(offset, radius) in profile {
        for s in 0..seg {
            let theta = s as f32 / seg as f32 * std::f32::consts::TAU;
            let pos = axis * offset + u * (radius * theta.cos()) + w * (radius * theta.sin());
            vertices.push(GeometryVertex::new(pos, Vec3::ZERO, material, smoothing));
        }
    }

    let mut indices = Vec::new();
    for ring in 0..profile.len().saturating_sub(1) {
        for s in 0..seg {
            let s1 = (s + 1) % seg;
            let a = (ring * seg + s) as u32;
            let b = (ring * seg + s1) as u32;
            let c = ((ring + 1) * seg + s1) as u32;
            let d = ((ring + 1) * seg + s) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}

fn tangent(axis: Vec3) -> Vec3 {
    let seed = if axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    seed.cross(axis).normalize_or_zero()
}

/// Translate every vertex of `mesh` by `t` (normals and shade preserved).
pub fn translate(mesh: &GeometryMesh, t: Vec3) -> GeometryMesh {
    let vertices =
        mesh.vertices().iter().map(|v| GeometryVertex { position: v.position + t, ..*v }).collect();
    GeometryMesh::new(vertices, mesh.indices().to_vec())
}

/// Concatenate meshes into one, offsetting indices — the repetition primitive (e.g. a wheel train).
pub fn merge(meshes: &[GeometryMesh]) -> GeometryMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for mesh in meshes {
        let base = vertices.len() as u32;
        vertices.extend_from_slice(mesh.vertices());
        indices.extend(mesh.indices().iter().map(|index| index + base));
    }
    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capped_cylinder_fills_its_radius_and_length() {
        let profile = [(-1.0, 0.0), (-1.0, 0.5), (1.0, 0.5), (1.0, 0.0)];
        let mesh = revolve(Vec3::X, &profile, 16, MaterialRole::Rubber, SmoothingGroup(5));
        assert!(mesh.triangle_count() > 0);
        let b = mesh.bounds().expect("non-empty");
        assert!(
            (b.max.x - 1.0).abs() < 1.0e-4 && (b.min.x + 1.0).abs() < 1.0e-4,
            "length spans axis"
        );
        assert!((b.max.y - 0.5).abs() < 0.02, "radius reaches 0.5: {:.3}", b.max.y);
    }

    #[test]
    fn translate_moves_the_bounds() {
        let profile = [(-0.5, 0.0), (-0.5, 0.3), (0.5, 0.3), (0.5, 0.0)];
        let wheel = revolve(Vec3::X, &profile, 12, MaterialRole::Rubber, SmoothingGroup(5));
        let moved = translate(&wheel, Vec3::new(0.0, 2.0, 0.0));
        assert!((moved.bounds().unwrap().max.y - 2.3).abs() < 0.02, "lifted by 2.0");
    }

    #[test]
    fn merge_sums_triangles() {
        let profile = [(-0.5, 0.0), (-0.5, 0.3), (0.5, 0.3), (0.5, 0.0)];
        let a = revolve(Vec3::X, &profile, 12, MaterialRole::Rubber, SmoothingGroup(5));
        let b = translate(&a, Vec3::new(0.0, 0.0, 1.0));
        let n = a.triangle_count() + b.triangle_count();
        assert_eq!(merge(&[a, b]).triangle_count(), n);
    }
}
