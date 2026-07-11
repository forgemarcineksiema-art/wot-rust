//! Blueprint-driven chassis: a shaped hull (glacis built from the armour slope, raked rear, side
//! fenders) and a wrapped track belt (a stadium that hugs the wheel line) with road wheels, a drive
//! sprocket, an idler, and return rollers — the realistic running gear, replacing the legacy box.

use game_core::{HullShape, SkirtShape, TrackShape};
use glam::{Vec2, Vec3};

use super::SG_HARD;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, LoftSection, LoftSpec, MaterialRole, MeshBuilder,
    ProfilePoint, RevolveSpec,
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

/// The plane-honest prism hull: both body prisms lofted directly from the armor volumes' plane
/// equations — the fold ridge at the sponson step, the glacis leaning `glacis_slope_deg` above
/// it, the derived lower nose below it, the rear pair at `rear_slope_deg`, and (unlike the
/// extruded [`blueprint_hull`]) upper SIDE walls leaned inward by `side_slope_deg`, the same
/// plane the armor model resolves a side shot on. For the sloped German school: what you see
/// leaning is what you shoot.
pub(crate) fn blueprint_prism_hull(hull: &HullShape, side_slope_deg: f32) -> MeshBuilder {
    let glacis = hull.glacis_slope_deg.to_radians().tan();
    // The lower-plate slope derives from the glacis exactly like the armor model's zone table.
    let lower = (hull.glacis_slope_deg * 0.45).to_radians().tan();
    let rear = hull.rear_slope_deg.to_radians().tan();
    let side = side_slope_deg.to_radians().tan();
    let step = hull.sponson_y;

    // One rectangular plan ring per height: front/rear z from the fold planes, width from the
    // side lean above the step (the tub stays vertical below it).
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

/// Welded engine-deck detail on the flat rear deck behind the turret: a raised, beveled deck panel
/// flanked by two access hatches, built from [`MeshBuilder::plate_box`] so they read as cut steel
/// plates. Kept on the flat deck (clear of the sloped glacis and the turret ring) and inside the
/// hull plan, so they add surface detail without touching the silhouette or the collision fit.
pub(crate) fn blueprint_deck_details(bp: &game_core::VehicleBlueprint) -> GeometryMesh {
    let hull = &bp.hull;
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

    // --- Fleet fittings (the polish pass): every piece is derived from blueprint fields, sits
    // --- inside the hitbox, and skips itself when the turret plan covers its spot.
    let glacis_run =
        (hull.deck_y - hull.sponson_y).max(0.0) * hull.glacis_slope_deg.to_radians().tan();
    let deck_front_edge = hull.half_len - glacis_run;
    // Whether a fitting of z-half-extent `half` centred at `z` would intrude on the turret's
    // seat (its plan plus a clearance ring). The WHOLE fitting must clear, not just its centre —
    // a hatch corner grazing the turret bounds reads as the turret sunk into the deck.
    let turret_covers = |z: f32, half: f32| -> bool {
        (z - bp.turret.ring_z).abs() < bp.turret.plan_half_length + half + 0.25
    };

    // Driver's hatch: a low plate on the forward deck, offset to the port side. A pike bow
    // narrows the deck to a ridge there — pike hulls seat their hatches elsewhere, so skip.
    let hatch_z = deck_front_edge - 0.45;
    if hull.pike_sweep_deg == 0.0 && !turret_covers(hatch_z, 0.36) && hatch_z > deck_front {
        builder = builder.plate_box(
            Vec3::new(hull.lower_half_width * 0.45, hull.deck_y + 0.045, hatch_z),
            Vec3::new(0.26, 0.025, 0.30),
            0.05,
            MaterialRole::RolledArmor,
            SG_HARD,
        );
    }

    // Headlight pods on the glacis shoulder corners (a pike has no shoulders to sit on).
    let light_z = deck_front_edge - 0.12;
    if hull.pike_sweep_deg == 0.0 && light_z > deck_front {
        for sign in [-1.0_f32, 1.0] {
            builder = builder.plate_box(
                Vec3::new(sign * hull.half_width * 0.62, hull.deck_y + 0.08, light_z),
                Vec3::new(0.09, 0.08, 0.09),
                0.03,
                MaterialRole::BarrelSteel,
                SG_HARD,
            );
        }
    }

    // Tow hooks: paired stubs seated against the bow and stern plates, kept INSIDE the hull
    // length so no vehicle's honesty gate (visuals within the hitbox) is ever poked. A pike
    // bow carries no flat plate to seat the front pair on — the stern pair stays.
    let hook_stations: &[f32] = if hull.pike_sweep_deg == 0.0 {
        &[hull.half_len - 0.14, -hull.half_len + 0.14]
    } else {
        &[-hull.half_len + 0.14]
    };
    for &z in hook_stations {
        for sign in [-1.0_f32, 1.0] {
            builder = builder.plate_box(
                Vec3::new(sign * hull.lower_half_width * 0.62, hull.sponson_y + 0.10, z),
                Vec3::new(0.10, 0.08, 0.12),
                0.04,
                MaterialRole::RolledArmor,
                SG_HARD,
            );
        }
    }

    // Antenna pot on the rear deck corner.
    let antenna_z = deck_back + 0.35;
    if !turret_covers(antenna_z, 0.16) {
        builder = builder.plate_box(
            Vec3::new(-half_x * 0.8, hull.deck_y + 0.10, antenna_z),
            Vec3::new(0.06, 0.09, 0.06),
            0.02,
            MaterialRole::BarrelSteel,
            SG_HARD,
        );
    }

    builder.build()
}

/// The **static belt band** for one blueprint, mirrored to both sides: a thin top run and bottom
/// run with wrapped metal end loops around the idler/sprocket, so the belt reads as one continuous
/// band. The moving parts — road wheels, drive sprocket, idler, and the shoe links — are no longer
/// baked here; they are instanced and animated at render time from
/// [`crate::running_gear::running_gear_placements`] so the wheels spin and the links scroll.
pub(crate) fn blueprint_running_gear(track: &TrackShape) -> GeometryMesh {
    let cy = (track.top_y + track.bottom_y) * 0.5;
    let cz = (track.wheel_first_z + track.wheel_last_z) * 0.5;
    let half_run = (track.wheel_last_z - track.wheel_first_z) * 0.5;

    let run_len = half_run + track.end_radius * 0.5;
    let run_half = Vec3::new(track.belt_half_thickness, 0.07, run_len);
    MeshBuilder::new()
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
        )
        .capped_revolve_at(Vec3::new(0.0, cy, cz - half_run), track_end_wrap_profile(track))
        .capped_revolve_at(Vec3::new(0.0, cy, cz + half_run), track_end_wrap_profile(track))
        .mirror(Axis::X)
        .build()
}

/// The side skirts, mirrored to both sides: one thin plate run hung outside the track band at
/// the blueprint's standoff — the SAME plane the armor volumes bake the skirt screen on, so the
/// sheet the player sees is the sheet a HEAT jet detonates against. A skirt-less blueprint
/// builds an empty mesh (a no-op append), so every blueprint recipe can call this untouched.
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

fn track_end_wrap_profile(track: &TrackShape) -> RevolveSpec {
    RevolveSpec {
        profile: vec![
            ProfilePoint::new(track.end_radius, track.inner_x),
            ProfilePoint::new(track.end_radius, track.outer_x),
        ],
        axis: Axis::X,
        segments: track.segments.max(12),
        material: MaterialRole::TrackMetal,
        smoothing: SG_HARD,
    }
}

fn point_at_y(a: Vec2, b: Vec2, y: f32) -> Vec2 {
    let t = ((y - a.y) / (b.y - a.y)).clamp(0.0, 1.0);
    a + (b - a) * t
}
