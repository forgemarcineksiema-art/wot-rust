//! Controlled surface thickening (renderer-free): give an open, manifold sheet a back face and a
//! bridged rim so it becomes a thin solid — for basket sheets, splash guards and optional fender
//! lips. v1 only accepts an open, consistently-wound manifold; it rejects closed, non-manifold or
//! inconsistently-wound input rather than guessing.
//!
//! It must never be used on gameplay armour unless the *outer* visible surface stays the exact
//! armour surface the combat model uses — so the original surface is preserved untouched as one
//! face, and only a back face and rim are added.

use std::collections::HashMap;

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, OPEN_OR_CLOSED_MESH};

/// Which way the back face is offset from the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellDirection {
    /// Each vertex offsets along its own normal — follows curvature.
    AlongVertexNormals,
    /// Offset along the single averaged `+normal`.
    PositiveNormal,
    /// Offset along the single averaged `-normal`.
    NegativeNormal,
}

/// What happens at the open boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellRimPolicy {
    /// Leave the rim open: two parallel sheets.
    Open,
    /// Bridge the rim into a watertight thin solid.
    Bridge,
}

/// A shelling request over a borrowed surface.
pub struct ShellSpec<'a> {
    pub surface: &'a GeometryMesh,
    pub thickness: f32,
    pub direction: ShellDirection,
    pub rim_policy: ShellRimPolicy,
}

/// Why a surface cannot be shelled.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum ShellError {
    #[error("shell thickness must be finite and positive")]
    NonPositiveThickness,
    #[error("the surface is empty")]
    EmptyInput,
    #[error("the surface must be open (it has no boundary to thicken)")]
    ClosedInput,
    #[error("the surface must be a consistently-wound manifold")]
    NonManifoldInput,
    #[error("the surface has non-finite or invalid data")]
    InvalidInput,
}

/// Thicken `surface` into a thin solid (or twin sheets) per the spec.
pub fn try_shell(spec: &ShellSpec<'_>) -> Result<GeometryMesh, ShellError> {
    if !spec.thickness.is_finite() || spec.thickness <= 0.0 {
        return Err(ShellError::NonPositiveThickness);
    }
    let surface = spec.surface;
    if surface.triangle_count() == 0 {
        return Err(ShellError::EmptyInput);
    }
    let report = surface.quality_report(OPEN_OR_CLOSED_MESH);
    if report.invalid_indices > 0
        || report.non_finite_vertices > 0
        || report.degenerate_triangles > 0
    {
        return Err(ShellError::InvalidInput);
    }
    if report.non_manifold_edges > 0 || report.inconsistent_winding_edges > 0 {
        return Err(ShellError::NonManifoldInput);
    }
    if report.boundary_edges == 0 {
        return Err(ShellError::ClosedInput);
    }

    let verts = surface.vertices();
    let offset = offsets(verts, spec.direction, spec.thickness);

    // Outer face: the original surface, untouched.
    let mut out: Vec<GeometryVertex> = verts.to_vec();
    let mut idx: Vec<u32> = surface.indices().to_vec();

    // Back face: the offset copy, wound the opposite way and with flipped normals.
    let base = out.len() as u32;
    for (v, o) in verts.iter().zip(&offset) {
        let mut back = *v;
        back.position = v.position + *o;
        back.normal = -v.normal;
        out.push(back);
    }
    for tri in surface.indices().chunks_exact(3) {
        idx.extend_from_slice(&[base + tri[0], base + tri[2], base + tri[1]]);
    }

    if spec.rim_policy == ShellRimPolicy::Bridge {
        for (a, b) in boundary_edges(surface.indices()) {
            // The outer face winds this boundary edge a->b, so the rim must wind it b->a to stay
            // consistent; the back face (reversed) winds its copy base_a->base_b.
            idx.extend_from_slice(&[b, a, base + a, b, base + a, base + b]);
        }
    }

    Ok(GeometryMesh::new(out, idx).weld_and_smooth())
}

fn offsets(verts: &[GeometryVertex], direction: ShellDirection, thickness: f32) -> Vec<Vec3> {
    let dir = |n: Vec3| match direction {
        ShellDirection::AlongVertexNormals => n,
        ShellDirection::PositiveNormal => averaged(verts),
        ShellDirection::NegativeNormal => -averaged(verts),
    };
    verts.iter().map(|v| dir(v.normal) * thickness).collect()
}

fn averaged(verts: &[GeometryVertex]) -> Vec3 {
    verts.iter().fold(Vec3::ZERO, |a, v| a + v.normal).normalize_or_zero()
}

/// Boundary directed edges: undirected index pairs used by exactly one triangle, returned in their
/// original winding direction so the rim closes consistently.
fn boundary_edges(indices: &[u32]) -> Vec<(u32, u32)> {
    let mut count: HashMap<(u32, u32), i32> = HashMap::new();
    let mut dir: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
    for tri in indices.chunks_exact(3) {
        for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = (a.min(b), a.max(b));
            *count.entry(key).or_insert(0) += 1;
            dir.entry(key).or_insert((a, b));
        }
    }
    count.into_iter().filter(|(_, c)| *c == 1).map(|(key, _)| dir[&key]).collect()
}
