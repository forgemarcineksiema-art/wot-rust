//! Blueprint-driven chassis: a shaped hull (glacis built from the armour slope, raked rear, side
//! fenders) and a wrapped track belt (a stadium that hugs the wheel line) with road wheels, a drive
//! sprocket, an idler, and return rollers — the realistic running gear, replacing the legacy box.

use std::f32::consts::{FRAC_PI_2, PI};

use game_core::{HullShape, TrackShape};
use glam::{Vec2, Vec3};

use super::{SG_HARD, SG_WHEEL};
use crate::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec};

/// The hull body: a side silhouette whose glacis is built from `glacis_slope_deg` (so the visible
/// plate is the same angle the armour model uses) plus a raked rear plate and flat side fenders
/// over the tracks.
pub(crate) fn blueprint_hull(hull: &HullShape, material: MaterialRole) -> MeshBuilder {
    let height = (hull.deck_y - hull.belly_y - hull.nose_rise).max(0.1);
    let glacis_run = height * hull.glacis_slope_deg.to_radians().tan();
    let rear_run = (hull.deck_y - hull.belly_y).max(0.1) * hull.rear_slope_deg.to_radians().tan();
    let section = vec![
        Vec2::new(-hull.half_len, hull.belly_y),
        Vec2::new(hull.half_len, hull.belly_y + hull.nose_rise),
        Vec2::new(hull.half_len - glacis_run, hull.deck_y),
        Vec2::new(-hull.half_len + rear_run, hull.deck_y),
    ];
    let mut builder = MeshBuilder::new().extrude(
        Vec3::ZERO,
        ExtrudeSpec { section, axis: Axis::X, half_depth: hull.half_width, material, smoothing: SG_HARD },
    );

    // Flat fenders overhanging each track, just below the deck.
    if hull.sponson_overhang > 0.0 {
        let fender_x = hull.half_width + hull.sponson_overhang * 0.5;
        let fender_half =
            Vec3::new(hull.sponson_overhang * 0.5 + 0.02, 0.04, hull.half_len * 0.96);
        builder = builder.chamfered_prism(
            Vec3::new(fender_x, hull.deck_y - 0.06, 0.0),
            fender_half,
            0.02,
            material,
            SG_HARD,
        );
        builder = builder.chamfered_prism(
            Vec3::new(-fender_x, hull.deck_y - 0.06, 0.0),
            fender_half,
            0.02,
            material,
            SG_HARD,
        );
    }
    builder
}

/// The wrapped running gear for one blueprint, mirrored to both sides.
pub(crate) fn blueprint_running_gear(track: &TrackShape) -> GeometryMesh {
    let cy = (track.top_y + track.bottom_y) * 0.5;
    let radius = (track.top_y - track.bottom_y) * 0.5;
    let cz = (track.wheel_first_z + track.wheel_last_z) * 0.5;
    let half_run = (track.wheel_last_z - track.wheel_first_z) * 0.5;

    // The track belt as a stadium (rounded rectangle) hugging the wheel line, extruded across width.
    let mut builder = MeshBuilder::new().extrude(
        Vec3::new(track.center_x, 0.0, 0.0),
        ExtrudeSpec {
            section: stadium_section(cz, cy, half_run, radius, 6),
            axis: Axis::X,
            half_depth: track.belt_half_thickness,
            material: MaterialRole::TrackMetal,
            smoothing: SG_HARD,
        },
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
    builder = add_track_shoes(builder, track, cz, half_run);

    builder.mirror(Axis::X).build()
}

/// Repeated shoe links riding the outer face of the top and bottom runs (cheap extruded boxes, so
/// the belt reads as a segmented track without blowing the triangle budget).
fn add_track_shoes(
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

/// A stadium (rounded-rectangle) outline in the (z, y) plane: two semicircles of `radius` at the
/// run ends joined by straight top and bottom edges. Convex, so the extruder accepts it directly.
fn stadium_section(cz: f32, cy: f32, half_run: f32, radius: f32, arc: usize) -> Vec<Vec2> {
    let mut section = Vec::with_capacity(2 * (arc + 1));
    for i in 0..=arc {
        let t = -FRAC_PI_2 + PI * (i as f32 / arc as f32);
        section.push(Vec2::new(cz + half_run + radius * t.cos(), cy + radius * t.sin()));
    }
    for i in 0..=arc {
        let t = FRAC_PI_2 + PI * (i as f32 / arc as f32);
        section.push(Vec2::new(cz - half_run + radius * t.cos(), cy + radius * t.sin()));
    }
    section
}
