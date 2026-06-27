//! The bake-time contact-cavity pass: deterministic ambient occlusion authored from a vehicle's
//! semantics rather than from merged-mesh indices, so it survives re-meshing and LOD like the
//! geometry it shades. A [`CavityBand`] is a soft axis-aligned recess (a turret-ring seam, a mantlet
//! seat, the running-gear recess, a weld line); [`GeometryMesh::with_contact_cavity`] multiplies
//! every vertex's `surface_shade` by each band's darkening.

use glam::Vec3;

use crate::mesh::GeometryMesh;

/// A semantic contact-cavity region. Each band darkens `surface_shade` toward `1 - amount` for
/// vertices inside an axis-aligned box around `center`, softening across `falloff` metres beyond the
/// box. Bands are authored from a vehicle's *semantics*, never from merged-mesh indices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CavityBand {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub falloff: f32,
    pub amount: f32,
}

impl CavityBand {
    /// The darkening weight in `0..=1` at `point`: `1` at the box core, fading to `0` at `falloff`
    /// metres outside it.
    pub fn weight(&self, point: Vec3) -> f32 {
        let outside = ((point - self.center).abs() - self.half_extents).max(Vec3::ZERO);
        let d = outside.length();
        if self.falloff <= 0.0 {
            if d <= 0.0 { 1.0 } else { 0.0 }
        } else {
            (1.0 - d / self.falloff).clamp(0.0, 1.0)
        }
    }
}

impl GeometryMesh {
    /// Bake semantic contact cavities into `surface_shade`: each band multiplies the shade toward
    /// `1 - amount` where vertices sit in its recess (turret-ring seam, mantlet seat, running-gear
    /// recess, weld lines). Deterministic and order-independent — a vertex's shade is the product of
    /// every band's darkening, clamped to `0..=1`. Any grounding height shade stays underneath it.
    pub fn with_contact_cavity(mut self, bands: &[CavityBand]) -> Self {
        for vertex in self.vertices_mut() {
            let mut shade = vertex.surface_shade;
            for band in bands {
                shade *= 1.0 - band.amount * band.weight(vertex.position);
            }
            *vertex = vertex.with_surface_shade(shade);
        }
        self
    }
}
