//! Blueprint-driven static chassis geometry. Running gear is deliberately absent: every visible
//! wheel, suspension unit, end wheel, and track link is instanced by the runtime gear path, so
//! terrain travel, tension, and thrown-track state can never diverge from a fused hull mesh.

use game_core::{HullShape, SkirtShape, TrackShape};
use glam::{Vec2, Vec3};

use super::SG_HARD;
use crate::{Axis, GeometryMesh, LoftSection, LoftSpec, MaterialRole, MeshBuilder};

/// The plane-honest prism hull: both body prisms lofted directly from the armor volumes' plane
/// equations — the fold ridge at the sponson step, the glacis leaning `glacis_slope_deg` above
/// it, the derived lower nose below it, the rear pair at `rear_slope_deg`, and upper side walls
/// leaned inward by `side_slope_deg`.
pub(crate) fn blueprint_prism_hull(hull: &HullShape, side_slope_deg: f32) -> MeshBuilder {
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    let lower = (hull.glacis_slope_deg * 0.45).to_radians().tan();
    let rear = hull.rear_slope_deg.to_radians().tan();
    let side = side_slope_deg.to_radians().tan();
    let step = hull.sponson_y;

    let ring = |y: f32| -> Vec<Vec2> {
        let (width, run) = if y >= step {
            (hull.half_width - (y - step) * side, y - step)
        } else {
            (hull.lower_half_width, 0.0)
        };
        let front = if y >= step {
            hull.half_len - run * glacis
        } else {
            hull.half_len - (step - y) * lower
        };
        let back = if y >= step {
            -hull.half_len + run * rear
        } else {
            -hull.half_len + (step - y) * rear
        };
        vec![
            Vec2::new(width, front),
            Vec2::new(width, back),
            Vec2::new(-width, back),
            Vec2::new(-width, front),
        ]
    };
    let prism = |bottom: f32, top: f32| LoftSpec {
        sections: vec![LoftSection::new(bottom, ring(bottom)), LoftSection::new(top, ring(top))],
        axis: Axis::Y,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
        cap_ends: true,
    };
    MeshBuilder::new()
        .loft(Vec3::ZERO, prism(hull.belly_y, step))
        .loft(Vec3::ZERO, prism(step, hull.deck_y))
}

/// Side skirts mirrored to both sides on the same plane used by the spaced-armor model.
pub(crate) fn blueprint_skirts(hull: &HullShape, track: &TrackShape) -> GeometryMesh {
    let Some(skirt): Option<SkirtShape> = hull.skirt else {
        return MeshBuilder::new().build();
    };
    let cx = track.outer_x + skirt.standoff_m + skirt.thickness_m * 0.5;
    let cy = (skirt.top_y + skirt.bottom_y) * 0.5;
    let cz = (skirt.front_z + skirt.rear_z) * 0.5;
    let half = Vec3::new(
        skirt.thickness_m * 0.5,
        (skirt.top_y - skirt.bottom_y).abs() * 0.5,
        (skirt.front_z - skirt.rear_z).abs() * 0.5,
    );
    MeshBuilder::new()
        .chamfered_prism(Vec3::new(cx, cy, cz), half, 0.02, MaterialRole::RolledArmor, SG_HARD)
        .mirror(Axis::X)
        .build()
}
