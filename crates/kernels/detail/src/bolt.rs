//! Bolt-head primitive: a short chamfered cylinder revolved about the surface normal.

use glam::Vec3;
use revolve::{revolve, translate};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

/// A bolt head seated at `center`, standing proud along `normal` by `height` with the given
/// `radius`. The top is lightly chamfered so it catches a highlight rather than reading as a disc.
pub fn bolt_head(center: Vec3, normal: Vec3, radius: f32, height: f32) -> GeometryMesh {
    let axis = normal.normalize_or_zero();
    let axis = if axis == Vec3::ZERO { Vec3::Y } else { axis };
    let profile =
        [(0.0, 0.0), (0.0, radius), (height * 0.7, radius), (height, radius * 0.7), (height, 0.0)];
    let mesh = revolve(axis, &profile, 12, MaterialRole::BarrelSteel, SmoothingGroup::hard_edges());
    translate(&mesh, center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bolt_head_stands_proud_of_its_seat() {
        let mesh = bolt_head(Vec3::ZERO, Vec3::Y, 0.05, 0.03);
        let b = mesh.bounds().expect("bolt bounds");
        assert!(b.max.y > 0.0 && b.max.y <= 0.031, "bolt rises along +Y by its height");
        assert!(b.max.x <= 0.051 && b.min.x >= -0.051, "bolt stays within its radius");
        mesh.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).expect("clean bolt");
    }
}
