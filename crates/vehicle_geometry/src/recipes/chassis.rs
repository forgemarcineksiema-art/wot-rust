//! Shared chassis shapes: the extruded hull body and the mirrored tracks and road wheels.

use glam::{Vec2, Vec3};

use super::{SG_HARD, SG_WHEEL};
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
};

// Shared lower-gear contact shading: darken low/occluded running gear without touching upper
// armour. Pinned for T-55A by `t55a_surface_shading_darkens_lower_running_gear`.
const SHADE_FLOOR_Y: f32 = -0.05;
const SHADE_BRIGHT_Y: f32 = 1.05;
const SHADE_LOW: f32 = 0.56;

/// Side-profile plan for an armoured hull body, swept across the vehicle width.
pub(crate) struct HullPlan {
    /// Half-length of the hull body along Z.
    pub half_len: f32,
    /// Y of the hull belly (its flat underside, above the track line).
    pub belly_y: f32,
    /// Y of the hull deck (its flat top).
    pub deck_y: f32,
    /// How far behind the nose the glacis top sits — `0.0` is a vertical front, larger is more
    /// raked.
    pub glacis_run: f32,
    /// Raises the lower-front edge above the belly to suggest a chin / lower plate (`0.0` = flat).
    pub nose_rise: f32,
    /// Half-width of the hull body (the extrusion depth, per side).
    pub half_width: f32,
}

/// Extrude the hull side silhouette across the width. The section traces, in `(z, y)`:
/// rear-bottom → front-bottom → raked glacis top → rear-top, so the front face carries the
/// glacis slope directly instead of being a separate tilted slab.
pub(crate) fn hull_body(plan: &HullPlan, material: MaterialRole) -> MeshBuilder {
    let section = vec![
        Vec2::new(-plan.half_len, plan.belly_y),
        Vec2::new(plan.half_len, plan.belly_y + plan.nose_rise),
        Vec2::new(plan.half_len - plan.glacis_run, plan.deck_y),
        Vec2::new(-plan.half_len, plan.deck_y),
    ];
    MeshBuilder::new().extrude(
        Vec3::ZERO,
        ExtrudeSpec {
            section,
            axis: Axis::X,
            half_depth: plan.half_width,
            material,
            smoothing: SG_HARD,
        },
    )
}

/// Tracks and road wheels, mirrored to both hull sides.
pub(crate) struct RunningGear {
    /// Half-extents of each side track run `(thickness, height, length)`.
    pub track_half: Vec3,
    /// `|x|` of each track-run centre.
    pub track_center_x: f32,
    /// Y of each track-run centre.
    pub track_center_y: f32,
    /// Z of each track-run centre.
    pub track_center_z: f32,
    pub wheel_radius: f32,
    pub wheel_count: usize,
    /// Y of the wheel axle line.
    pub wheel_y: f32,
    pub wheel_first_z: f32,
    pub wheel_last_z: f32,
    /// `|x|` of the inner and outer rubber faces (the disc spans between them).
    pub wheel_inner_x: f32,
    pub wheel_outer_x: f32,
    pub wheel_segments: usize,
}

/// Build the right-side track run and road-wheel discs in a separate [`MeshBuilder`], mirror to
/// get the left side, then append the result into the hull builder.
pub(crate) fn add_running_gear(builder: MeshBuilder, gear: &RunningGear) -> MeshBuilder {
    let gear_mesh = build_running_gear(gear);
    builder.append(&gear_mesh)
}

fn build_running_gear(gear: &RunningGear) -> GeometryMesh {
    let mut builder = MeshBuilder::new();
    // right track run
    builder = builder.chamfered_prism(
        Vec3::new(gear.track_center_x, gear.track_center_y, gear.track_center_z),
        gear.track_half,
        0.08,
        MaterialRole::TrackMetal,
        SG_HARD,
    );
    builder = add_track_shoes(builder, gear);
    // right road wheels, arrayed along Z
    let step = if gear.wheel_count <= 1 {
        0.0
    } else {
        (gear.wheel_last_z - gear.wheel_first_z) / (gear.wheel_count - 1) as f32
    };
    let mut wheel_builder = MeshBuilder::new();
    wheel_builder = wheel_builder.capped_revolve_at(
        Vec3::new(0.0, gear.wheel_y, gear.wheel_first_z),
        RevolveSpec {
            profile: vec![
                ProfilePoint::new(gear.wheel_radius, gear.wheel_inner_x),
                ProfilePoint::new(gear.wheel_radius, gear.wheel_outer_x),
            ],
            axis: Axis::X,
            segments: gear.wheel_segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        },
    );
    if gear.wheel_count > 1 {
        wheel_builder = wheel_builder.array_along(Axis::Z, step, gear.wheel_count);
    }
    builder = builder.append(&wheel_builder.build());
    builder.mirror(Axis::X).build()
}

fn add_track_shoes(mut builder: MeshBuilder, gear: &RunningGear) -> MeshBuilder {
    let shoe_count = ((gear.track_half.z * 2.0) / 0.55).round() as usize;
    let shoe_count = shoe_count.clamp(10, 12);
    let shoe_step = (gear.track_half.z * 1.72) / (shoe_count - 1) as f32;
    let first_z = gear.track_center_z - shoe_step * (shoe_count - 1) as f32 * 0.5;
    let shoe_half_x = (gear.track_half.x * 0.16).clamp(0.025, 0.045);
    let shoe_half_z = (shoe_step * 0.22).clamp(0.055, 0.090);
    let shoe_half_y = gear.track_half.y * 0.76;
    let outer_x = gear.track_center_x + gear.track_half.x;

    for index in 0..shoe_count {
        let center = Vec3::new(
            outer_x - shoe_half_x,
            gear.track_center_y,
            first_z + shoe_step * index as f32,
        );
        let section = vec![
            Vec2::new(-shoe_half_z, -shoe_half_y),
            Vec2::new(shoe_half_z, -shoe_half_y),
            Vec2::new(shoe_half_z, shoe_half_y),
            Vec2::new(-shoe_half_z, shoe_half_y),
        ];
        builder = builder.extrude(
            center,
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: shoe_half_x,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        );
    }
    builder
}

/// Apply the shared lower-gear contact shading to a finished hull mesh.
pub(crate) fn shade_hull(mesh: GeometryMesh) -> GeometryMesh {
    mesh.with_height_shade(SHADE_FLOOR_Y, SHADE_BRIGHT_Y, SHADE_LOW)
}
