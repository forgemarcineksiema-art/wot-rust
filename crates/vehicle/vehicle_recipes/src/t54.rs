//! T-54-specific geometry overrides for the benchmark vehicle.

use game_core::{HullShape, TrackShape, TurretShape};
use glam::{Vec2, Vec3};

use super::{SG_CAST, SG_HARD};
use vehicle_geometry::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder};

pub(crate) fn t54_hull(hull: &HullShape, track: &TrackShape) -> MeshBuilder {
    let rear_run = (hull.deck_y - hull.belly_y).max(0.1) * hull.rear_slope_deg.to_radians().tan();
    let rear_bottom = Vec2::new(-hull.half_len, hull.belly_y);
    let front_bottom = Vec2::new(hull.half_len, hull.belly_y + hull.nose_rise);
    let front_lower_top = Vec2::new(hull.half_len, track.top_y - 0.10);
    // The wide upper hull starts at the sponson step (it overhangs the tracks from there up), so
    // its front-bottom corner sits on the sponson line, not dipped into the track band.
    let front_upper_low = Vec2::new(hull.half_len - 0.68, hull.sponson_y + 0.045);
    let front_top = Vec2::new(hull.half_len - 2.05, hull.deck_y);
    let rear_sponson = Vec2::new(-hull.half_len + rear_run, hull.sponson_y);
    let rear_top = Vec2::new(-hull.half_len + rear_run, hull.deck_y);

    let lower_tub = vec![rear_bottom, front_bottom, front_lower_top, rear_sponson];
    let upper_hull = vec![rear_sponson, front_upper_low, front_top, rear_top];
    let mut builder = MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: lower_tub,
                axis: Axis::X,
                half_depth: hull.lower_half_width,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        )
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: upper_hull,
                axis: Axis::X,
                half_depth: hull.half_width,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        )
        .plate_box(
            Vec3::new(0.0, (front_bottom.y + front_lower_top.y) * 0.5, hull.half_len + 0.02),
            Vec3::new(hull.lower_half_width * 0.96, 0.32, 0.04),
            0.035,
            MaterialRole::RolledArmor,
            SG_HARD,
        )
        .chamfered_prism(
            Vec3::new(0.0, (front_bottom.y + front_lower_top.y) * 0.5, hull.half_len + 0.075),
            Vec3::new(hull.lower_half_width * 0.86, 0.035, 0.025),
            0.015,
            MaterialRole::RolledArmor,
            SG_HARD,
        );

    // The fender shelf rides over the exposed track band (outer face flush with the 3.27 m
    // overall width), a hand above the top run — the kit line of the narrow-box hull.
    let fender_half_x = (track.outer_x - track.inner_x) * 0.5 + 0.005;
    let fender_center_x = (track.outer_x + track.inner_x) * 0.5;
    let fender_half_z = (track.wheel_last_z - track.wheel_first_z) * 0.5 + track.end_radius * 0.75;
    for side in [-1.0, 1.0] {
        builder = builder.chamfered_prism(
            Vec3::new(side * fender_center_x, hull.sponson_y + 0.12, 0.0),
            Vec3::new(fender_half_x, 0.045, fender_half_z),
            0.025,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }

    builder
}

pub(crate) fn t54_turret_front(
    _turret: &TurretShape,
    trunnion_y: f32,
    mantlet: Option<(f32, f32, f32)>,
) -> GeometryMesh {
    let Some((radius, back_z, front_z)) = mantlet else {
        return MeshBuilder::new().build();
    };
    let span = (front_z - back_z).max(0.18);
    let z = back_z + span * 0.46;
    let cheek_half_z = span * 0.45;
    let mut builder = MeshBuilder::new();

    for side in [-1.0, 1.0] {
        builder = builder.chamfered_prism(
            Vec3::new(side * radius * 1.78, trunnion_y + 0.01, z + 0.02),
            Vec3::new(radius * 1.20, radius * 0.92, cheek_half_z),
            0.11,
            MaterialRole::CastArmor,
            SG_CAST,
        );
    }

    builder
        .chamfered_prism(
            Vec3::new(0.0, trunnion_y + radius * 0.88, z + 0.02),
            Vec3::new(radius * 2.10, radius * 0.30, cheek_half_z * 0.82),
            0.09,
            MaterialRole::CastArmor,
            SG_CAST,
        )
        .chamfered_prism(
            Vec3::new(0.0, trunnion_y - radius * 0.86, z),
            Vec3::new(radius * 1.88, radius * 0.24, cheek_half_z * 0.72),
            0.07,
            MaterialRole::CastArmor,
            SG_CAST,
        )
        .build()
}
