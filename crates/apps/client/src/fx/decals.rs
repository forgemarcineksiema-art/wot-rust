//! Hit decals on vehicles: every replicated shell strike leaves a mark ON the plate it hit —
//! a permanent black hole for a penetration, a fading bare-metal scuff for a bounce, a long
//! gouge for a ricochet. Marks are stored in the frame they belong to (hull marks ride the
//! hull, turret marks traverse with the turret) and drawn as flat oriented quads through the
//! FX pass, so a battered tank *looks* battered without recolouring a single submesh.

use game_core::{ArmorFacing, DamageEvent};
use glam::Vec3;
use net::TankSnapshot;
use renderer_api::FxVertex;

use crate::vehicle::pose::VehiclePose;
use crate::vehicle::variation::{DecalFrame, DecalKind, HitDecal};

/// How far off the plate the quad floats to never z-fight the armor it marks.
const DECAL_LIFT_M: f32 = 0.05;

/// Resolve one replicated shell strike into a decal in the target's local frame. Returns `None`
/// only when the hit direction degenerates (a corrupt event).
pub(crate) fn decal_from_damage_event(
    event: &DamageEvent,
    target: &TankSnapshot,
) -> Option<HitDecal> {
    let pose = pose_of(target);
    let facing = event.armor_zone.facing();
    let frame = match facing {
        ArmorFacing::TurretFront | ArmorFacing::TurretSide | ArmorFacing::TurretRear => {
            DecalFrame::Turret
        }
        _ => DecalFrame::Hull,
    };
    let (origin, basis) = match frame {
        DecalFrame::Hull => (pose.hull_translation(), pose.hull_basis()),
        DecalFrame::Turret => (pose.turret_translation(), pose.turret_basis()),
    };
    let local = basis.transpose() * (event.hit_position - origin);
    if !local.is_finite() {
        return None;
    }
    let kind = if event.penetrated {
        DecalKind::Penetration
    } else if event.ricocheted {
        DecalKind::Gouge
    } else {
        DecalKind::Scuff
    };
    let radius = match kind {
        DecalKind::Penetration => 0.13,
        DecalKind::Scuff => 0.20,
        DecalKind::Gouge => 0.14,
    };
    Some(HitDecal {
        local_position: local.to_array(),
        local_normal: local_facing_normal(facing, local.x).to_array(),
        radius,
        age_s: 0.0,
        kind,
        frame,
    })
}

/// Outward plate normal in the local frame of the zone's facing. Both hull and turret frames
/// share the +Z-forward convention, so one table serves them.
fn local_facing_normal(facing: ArmorFacing, local_x: f32) -> Vec3 {
    match facing {
        ArmorFacing::HullFront | ArmorFacing::TurretFront => Vec3::Z,
        ArmorFacing::HullRear | ArmorFacing::TurretRear => Vec3::NEG_Z,
        ArmorFacing::HullSide | ArmorFacing::TurretSide => {
            Vec3::new(local_x.signum().max(-1.0), 0.0, 0.0)
        }
    }
}

/// Append one tank's battle scars as flat quads riding its posed hull/turret. `opacity` already
/// folded per decal; a penetration hole stays pitch dark, scuffs thin back to the paint.
pub(crate) fn append_decal_quads(
    vertices: &mut Vec<FxVertex>,
    decals: &[HitDecal],
    tank: &TankSnapshot,
) {
    if decals.is_empty() {
        return;
    }
    let pose = pose_of(tank);
    for decal in decals {
        let opacity = decal.opacity();
        if opacity <= 0.0 {
            continue;
        }
        let (origin, basis) = match decal.frame {
            DecalFrame::Hull => (pose.hull_translation(), pose.hull_basis()),
            DecalFrame::Turret => (pose.turret_translation(), pose.turret_basis()),
        };
        let center = origin
            + basis * Vec3::from_array(decal.local_position)
            + basis * Vec3::from_array(decal.local_normal) * DECAL_LIFT_M;
        let normal = basis * Vec3::from_array(decal.local_normal);
        let mut u = normal.cross(Vec3::Y);
        if u.length_squared() < 1.0e-6 {
            u = normal.cross(Vec3::X);
        }
        let u = u.normalize_or_zero();
        let v = normal.cross(u);
        let plate = Plate { center, u, v };
        match decal.kind {
            DecalKind::Penetration => push_penetration(vertices, plate, decal, opacity),
            DecalKind::Scuff => push_scuff(vertices, plate, decal, opacity),
            DecalKind::Gouge => push_gouge(vertices, plate, decal, opacity),
        }
    }
}

/// The oriented plane a decal stamps onto: its center (already lifted off the armor) and the
/// two in-plane axes.
#[derive(Clone, Copy)]
struct Plate {
    center: Vec3,
    u: Vec3,
    v: Vec3,
}

/// A penetration is three layers, not one blob: a wide SOFT scorch halo (burnt paint), the
/// hard-edged near-black entry hole, and a fan of bright bare-metal splash streaks — the
/// signature look of a shaped hole in rolled armor, readable at battle range.
fn push_penetration(vertices: &mut Vec<FxVertex>, plate: Plate, decal: &HitDecal, opacity: f32) {
    let r = decal.radius;
    push_stamp(vertices, plate, r * 2.6, r * 2.6, 0.9, premul([0.05, 0.04, 0.035], 0.45 * opacity));
    push_stamp(vertices, plate, r, r, 6.0, premul([0.012, 0.011, 0.010], 0.95 * opacity));
    for (angle, length) in splash_angles(decal.local_position) {
        let direction = plate.u * angle.cos() + plate.v * angle.sin();
        let streak = Plate {
            center: plate.center + direction * (r * 0.7 + length * 0.5),
            u: direction,
            v: plate.v.cross(plate.u).cross(direction).normalize_or_zero(),
        };
        push_stamp(
            vertices,
            streak,
            length * 0.5,
            r * 0.14,
            2.5,
            premul([0.42, 0.41, 0.38], 0.55 * opacity),
        );
    }
}

/// A non-penetrating smack: a soft smudge of scorched paint around a smaller bared-metal core.
fn push_scuff(vertices: &mut Vec<FxVertex>, plate: Plate, decal: &HitDecal, opacity: f32) {
    let r = decal.radius;
    push_stamp(vertices, plate, r * 1.6, r * 1.6, 0.9, premul([0.10, 0.095, 0.09], 0.45 * opacity));
    push_stamp(vertices, plate, r * 0.7, r * 0.7, 2.0, premul([0.34, 0.33, 0.31], 0.5 * opacity));
}

/// A ricochet gouge: a hard bright scrape along the plate inside a soft scorched trail.
fn push_gouge(vertices: &mut Vec<FxVertex>, plate: Plate, decal: &HitDecal, opacity: f32) {
    let r = decal.radius;
    push_stamp(vertices, plate, r * 3.0, r * 0.9, 0.9, premul([0.09, 0.085, 0.08], 0.45 * opacity));
    push_stamp(vertices, plate, r * 2.6, r * 0.35, 3.0, premul([0.40, 0.39, 0.37], 0.6 * opacity));
}

/// One oriented quad on the plate with half-extents along its axes and an edge sharpness.
fn push_stamp(
    vertices: &mut Vec<FxVertex>,
    plate: Plate,
    half_u_m: f32,
    half_v_m: f32,
    sharpness: f32,
    color: [f32; 4],
) {
    for uv in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
        let position = plate.center + plate.u * (half_u_m * uv[0]) + plate.v * (half_v_m * uv[1]);
        vertices.push(FxVertex::sharp(position.to_array(), uv, sharpness, color));
    }
}

/// Deterministic splash-streak fan for one hole: angles and lengths hashed from the decal's
/// local position, so every hole looks individual yet renders identically every frame.
fn splash_angles(local_position: [f32; 3]) -> [(f32, f32); 5] {
    let mut seed = local_position.iter().fold(0x9E37_79B9_7F4A_7C15_u64, |acc, component| {
        acc.wrapping_mul(31).wrapping_add(u64::from(component.to_bits()))
    });
    let mut next = || {
        seed = (seed ^ (seed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        ((seed >> 40) as f32) / ((1u64 << 24) as f32)
    };
    let mut fan = [(0.0, 0.0); 5];
    for (index, slot) in fan.iter_mut().enumerate() {
        let angle = index as f32 / 5.0 * std::f32::consts::TAU + next() * 0.9;
        let length = 0.14 + next() * 0.16;
        *slot = (angle, length);
    }
    fan
}

fn premul(tone: [f32; 3], alpha: f32) -> [f32; 4] {
    [tone[0] * alpha, tone[1] * alpha, tone[2] * alpha, alpha]
}

/// The shared pose both ingest and render use, so the local frame round-trips exactly. Attitude
/// carries the authoritative hull pitch/roll; the presentation spring's extra theatrics are a
/// couple of degrees and not worth a divergent frame.
fn pose_of(tank: &TankSnapshot) -> VehiclePose {
    VehiclePose::new_with_attitude(
        tank.vehicle,
        Vec3::from_array(tank.position),
        tank.yaw_rad,
        tank.turret_yaw_rad,
        tank.gun_pitch_rad,
        [tank.hull_pitch_rad, tank.hull_roll_rad, 0.0],
    )
}

#[cfg(test)]
mod tests {
    use game_core::{ArmorZone, DamageCause, TankId, TeamId, VehicleKind};

    use super::*;

    fn target(yaw: f32, turret_yaw: f32) -> TankSnapshot {
        let spec = VehicleKind::T54_1951.spec();
        TankSnapshot {
            tank_id: TankId(9),
            team: TeamId(2),
            vehicle: spec.kind,
            position: [40.0, 2.0, 60.0],
            yaw_rad: yaw,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: turret_yaw,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: spec.hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 1.0,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
        }
    }

    fn event(hit: Vec3, zone: ArmorZone, penetrated: bool, ricocheted: bool) -> DamageEvent {
        DamageEvent {
            source: TankId(1),
            target: TankId(9),
            hit_position: hit,
            damage_hp: 100,
            penetrated,
            ricocheted,
            cause: DamageCause::Shell,
            armor_zone: zone,
            ..Default::default()
        }
    }

    #[test]
    fn a_decal_round_trips_ingest_and_render_back_to_the_hit_point() {
        let tank = target(0.7, 0.0);
        let hit = Vec3::new(41.5, 2.8, 61.0);
        let decal = decal_from_damage_event(&event(hit, ArmorZone::HullSide, true, false), &tank)
            .expect("decal resolves");

        let mut vertices = Vec::new();
        append_decal_quads(&mut vertices, &[decal], &tank);
        // A penetration is layered: scorch halo + hard hole + 5 splash streaks, 6 verts each.
        assert_eq!(vertices.len(), 7 * 6);
        // The HARD-EDGED hole stamp (the sharpest layer) must sit exactly on the hit point.
        let hole: Vec<Vec3> = vertices
            .iter()
            .filter(|vertex| vertex.sharpness >= 5.0)
            .map(|vertex| Vec3::from_array(vertex.position))
            .collect();
        assert_eq!(hole.len(), 6, "exactly one hard hole stamp");
        let center: Vec3 = hole.iter().copied().sum::<Vec3>() / 6.0;
        assert!(
            center.distance(hit) < DECAL_LIFT_M + 0.02,
            "the hole sits on the hit point (plus its z-fight lift), got {}",
            center.distance(hit)
        );
    }

    #[test]
    fn turret_marks_traverse_with_the_turret_while_hull_marks_stay() {
        let still = target(0.0, 0.0);
        let hit = Vec3::new(40.0, 3.6, 61.2); // on the turret front, ahead of the ring
        let decal =
            decal_from_damage_event(&event(hit, ArmorZone::TurretFront, false, false), &still)
                .expect("decal resolves");
        assert_eq!(decal.frame, DecalFrame::Turret);

        // Re-render the same decal with the turret slewed 90 degrees: the mark must move.
        let mut at_zero = Vec::new();
        append_decal_quads(&mut at_zero, &[decal], &still);
        let mut slewed = Vec::new();
        append_decal_quads(&mut slewed, &[decal], &target(0.0, std::f32::consts::FRAC_PI_2));
        let moved =
            Vec3::from_array(at_zero[0].position).distance(Vec3::from_array(slewed[0].position));
        assert!(moved > 0.5, "a turret mark rides the traverse, moved {moved}");

        // A hull mark ignores the traverse entirely.
        let hull_decal = decal_from_damage_event(
            &event(Vec3::new(41.0, 2.4, 60.0), ArmorZone::HullSide, false, false),
            &still,
        )
        .expect("hull decal");
        let mut hull_zero = Vec::new();
        append_decal_quads(&mut hull_zero, &[hull_decal], &still);
        let mut hull_slewed = Vec::new();
        append_decal_quads(
            &mut hull_slewed,
            &[hull_decal],
            &target(0.0, std::f32::consts::FRAC_PI_2),
        );
        assert_eq!(hull_zero[0].position, hull_slewed[0].position);
    }

    #[test]
    fn kinds_map_to_hole_scuff_and_gouge_with_correct_permanence() {
        let tank = target(0.0, 0.0);
        let hit = Vec3::new(40.0, 2.4, 62.0);
        let hole = decal_from_damage_event(&event(hit, ArmorZone::UpperGlacis, true, false), &tank)
            .unwrap();
        let scuff =
            decal_from_damage_event(&event(hit, ArmorZone::UpperGlacis, false, false), &tank)
                .unwrap();
        let gouge =
            decal_from_damage_event(&event(hit, ArmorZone::UpperGlacis, false, true), &tank)
                .unwrap();
        assert_eq!(hole.kind, DecalKind::Penetration);
        assert_eq!(scuff.kind, DecalKind::Scuff);
        assert_eq!(gouge.kind, DecalKind::Gouge);

        // Permanence: an aged hole keeps full opacity, an aged scuff is gone.
        let aged_hole = HitDecal { age_s: 1.0e4, ..hole };
        let aged_scuff = HitDecal { age_s: 1.0e4, ..scuff };
        assert_eq!(aged_hole.opacity(), 1.0);
        assert_eq!(aged_scuff.opacity(), 0.0);

        // A gouge's hard scrape renders elongated; a scuff's hard core stays round. Measure the
        // SHARPEST stamp of each (UV order: v0->v1 spans the u axis, v1->v2 the v axis).
        let core_aspect = |decal: &HitDecal| {
            let mut vertices = Vec::new();
            append_decal_quads(&mut vertices, std::slice::from_ref(decal), &tank);
            let sharpest = vertices
                .chunks(6)
                .max_by(|a, b| a[0].sharpness.total_cmp(&b[0].sharpness))
                .expect("stamps exist");
            let at = |i: usize| Vec3::from_array(sharpest[i].position);
            at(0).distance(at(1)) / at(1).distance(at(2)).max(1.0e-6)
        };
        assert!(
            core_aspect(&gouge) > 3.0,
            "gouge scrapes along the plate, got {}",
            core_aspect(&gouge)
        );
        assert!(core_aspect(&scuff) < 1.5, "scuff stays round, got {}", core_aspect(&scuff));
    }
}
