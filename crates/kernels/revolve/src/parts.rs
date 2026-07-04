//! Round fittings built from the revolve generator. The running gear (wheels, idler, sprocket,
//! track links) is *not* built here: the animated path in `vehicle_geometry` reads the blueprint's
//! `TrackShape` and instances its own unit meshes, so a second revolve copy would only drift.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

use crate::{revolve, translate};

/// A short upright drum (cylinder about Y) centred at `center`: the basis of round fittings such as
/// the cupola hatch lid and the glacis headlight.
pub fn drum(
    center: Vec3,
    radius: f32,
    half_height: f32,
    segments: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let profile =
        [(-half_height, 0.0), (-half_height, radius), (half_height, radius), (half_height, 0.0)];
    translate(&revolve(Vec3::Y, &profile, segments, material, smoothing), center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drum_is_a_closed_upright_cylinder_at_its_centre() {
        let mesh = drum(
            Vec3::new(0.5, 2.0, -0.25),
            0.2,
            0.05,
            16,
            MaterialRole::RolledArmor,
            SmoothingGroup::hard_edges(),
        );
        assert!(mesh.triangle_count() > 0);
        let bounds = mesh.bounds().expect("non-empty drum");
        assert!((bounds.min.y - 1.95).abs() < 1.0e-4 && (bounds.max.y - 2.05).abs() < 1.0e-4);
        assert!((bounds.max.x - 0.7).abs() < 1.0e-3 && (bounds.max.z + 0.05).abs() < 1.0e-3);
    }
}
