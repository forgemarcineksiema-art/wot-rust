//! Blueprint-driven chassis: a shaped hull (glacis built from the armour slope, raked rear, side
//! fenders) and a wrapped track belt (a stadium that hugs the wheel line) with road wheels, a drive
//! sprocket, an idler, and return rollers — the realistic running gear, replacing the legacy box.

use game_core::{HullShape, TrackShape};
use glam::{Vec2, Vec3};

use super::{SG_HARD, SG_WHEEL};
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
};

/// The hull body: a side silhouette whose glacis is built from `glacis_slope_deg` (so the visible
/// plate is the same angle the armour model uses) plus a raked rear plate and a wider upper
/// sponson over a narrower lower tub.
pub(crate) fn blueprint_hull(hull: &HullShape, material: MaterialRole) -> MeshBuilder {
    let height = (hull.deck_y - hull.belly_y - hull.nose_rise).max(0.1);
    let glacis_run = height * hull.glacis_slope_deg.to_radians().tan();
    let rear_run = (hull.deck_y - hull.belly_y).max(0.1) * hull.rear_slope_deg.to_radians().tan();
    let rear_bottom = Vec2::new(-hull.half_len, hull.belly_y);
    let front_bottom = Vec2::new(hull.half_len, hull.belly_y + hull.nose_rise);
    let front_top = Vec2::new(hull.half_len - glacis_run, hull.deck_y);
    let rear_top = Vec2::new(-hull.half_len + rear_run, hull.deck_y);
    let front_step = point_at_y(front_bottom, front_top, hull.sponson_y);
    let rear_step = point_at_y(rear_bottom, rear_top, hull.sponson_y);
    let lower_section = vec![rear_bottom, front_bottom, front_step, rear_step];
    let upper_section = vec![rear_step, front_step, front_top, rear_top];

    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: lower_section,
                axis: Axis::X,
                half_depth: hull.lower_half_width,
                material,
                smoothing: SG_HARD,
            },
        )
        .extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: upper_section,
                axis: Axis::X,
                half_depth: hull.half_width,
                material,
                smoothing: SG_HARD,
            },
        )
}

/// Welded engine-deck detail on the flat rear deck behind the turret: a raised, beveled deck panel
/// flanked by two access hatches, built from [`MeshBuilder::plate_box`] so they read as cut steel
/// plates. Kept on the flat deck (clear of the sloped glacis and the turret ring) and inside the
/// hull plan, so they add surface detail without touching the silhouette or the collision fit.
pub(crate) fn blueprint_deck_details(hull: &HullShape) -> GeometryMesh {
    // The engine deck spans from just behind the turret ring to short of the rear plate.
    let deck_front = -1.15;
    let deck_back = -hull.half_len + 0.35;
    let center_z = (deck_front + deck_back) * 0.5;
    let half_z = (deck_front - deck_back) * 0.5;
    let half_x = hull.lower_half_width * 0.92;

    let mut builder = MeshBuilder::new().plate_box(
        Vec3::new(0.0, hull.deck_y + 0.03, center_z),
        Vec3::new(half_x, 0.06, half_z),
        0.06,
        MaterialRole::RolledArmor,
        SG_HARD,
    );
    for x in [-half_x * 0.5, half_x * 0.5] {
        builder = builder.plate_box(
            Vec3::new(x, hull.deck_y + 0.10, center_z + half_z * 0.3),
            Vec3::new(half_x * 0.28, 0.05, half_z * 0.35),
            0.04,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }
    builder.build()
}

/// The wrapped running gear for one blueprint, mirrored to both sides.
pub(crate) fn blueprint_running_gear(track: &TrackShape) -> GeometryMesh {
    let cy = (track.top_y + track.bottom_y) * 0.5;
    let cz = (track.wheel_first_z + track.wheel_last_z) * 0.5;
    let half_run = (track.wheel_last_z - track.wheel_first_z) * 0.5;

    // The track as a thin top run and bottom run (not a solid block), so the road wheels show
    // between them; the drive sprocket and idler round the ends.
    let run_len = half_run + track.end_radius * 0.5;
    let run_half = Vec3::new(track.belt_half_thickness, 0.07, run_len);
    let mut builder = MeshBuilder::new()
        .chamfered_prism(
            Vec3::new(track.center_x, track.top_y, cz),
            run_half,
            0.05,
            MaterialRole::TrackMetal,
            SG_HARD,
        )
        .chamfered_prism(
            Vec3::new(track.center_x, track.bottom_y, cz),
            run_half,
            0.05,
            MaterialRole::TrackMetal,
            SG_HARD,
        );

    // Road wheels, arrayed along the run.
    let mut wheels = MeshBuilder::new().capped_revolve_at(
        Vec3::new(0.0, cy, track.wheel_first_z),
        wheel_profile(track.wheel_radius, track.inner_x, track.outer_x),
    );
    if track.wheel_count > 1 {
        let step = (track.wheel_last_z - track.wheel_first_z) / (track.wheel_count - 1) as f32;
        wheels = wheels.array_along(Axis::Z, step, track.wheel_count);
    }
    builder = builder.append(&wheels.build());

    // Drive sprocket (rear) and idler (front): larger discs at the run ends. Kept within the belt
    // width (outer_x) so the running gear stays inside the collision box.
    for z in [cz - half_run, cz + half_run] {
        builder = builder.capped_revolve_at(
            Vec3::new(0.0, cy, z),
            wheel_profile(track.end_radius, track.inner_x - 0.04, track.outer_x),
        );
    }

    // A few return rollers riding the top run.
    let mut roller = MeshBuilder::new().capped_revolve_at(
        Vec3::new(0.0, track.top_y - 0.06, cz - half_run * 0.5),
        wheel_profile(0.12, track.inner_x + 0.02, track.outer_x),
    );
    roller = roller.array_along(Axis::Z, half_run, 2);
    builder = builder.append(&roller.build());

    // Track-shoe links along the top and bottom runs, so the belt reads as a segmented track.
    builder = add_blueprint_track_links(builder, track, cz, half_run);

    builder.mirror(Axis::X).build()
}

/// Repeated shoe links riding the outer face of the top and bottom runs (cheap extruded boxes, so
/// the belt reads as a segmented track without blowing the triangle budget).
fn add_blueprint_track_links(
    mut builder: MeshBuilder,
    track: &TrackShape,
    cz: f32,
    half_run: f32,
) -> MeshBuilder {
    let count = 10usize;
    let step = (2.0 * half_run) / (count - 1) as f32;
    let outer_x = track.center_x + track.belt_half_thickness;
    let shoe_half_x = track.belt_half_thickness * 0.5;
    let shoe_half_y = 0.07;
    let shoe_half_z = step * 0.30;
    let section = vec![
        Vec2::new(-shoe_half_z, -shoe_half_y),
        Vec2::new(shoe_half_z, -shoe_half_y),
        Vec2::new(shoe_half_z, shoe_half_y),
        Vec2::new(-shoe_half_z, shoe_half_y),
    ];
    for run_y in [track.top_y, track.bottom_y] {
        for i in 0..count {
            let z = cz - half_run + step * i as f32;
            builder = builder.extrude(
                Vec3::new(outer_x - shoe_half_x, run_y, z),
                ExtrudeSpec {
                    section: section.clone(),
                    axis: Axis::X,
                    half_depth: shoe_half_x,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_HARD,
                },
            );
        }
    }
    builder
}

fn wheel_profile(radius: f32, inner_x: f32, outer_x: f32) -> RevolveSpec {
    RevolveSpec {
        profile: vec![ProfilePoint::new(radius, inner_x), ProfilePoint::new(radius, outer_x)],
        axis: Axis::X,
        segments: 14,
        material: MaterialRole::Rubber,
        smoothing: SG_WHEEL,
    }
}

fn point_at_y(a: Vec2, b: Vec2, y: f32) -> Vec2 {
    let t = ((y - a.y) / (b.y - a.y)).clamp(0.0, 1.0);
    a + (b - a) * t
}
