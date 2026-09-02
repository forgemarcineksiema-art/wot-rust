//! Hit decals on vehicles: every replicated shell strike leaves a mark ON the plate it hit —
//! a permanent hole for a penetration, a dished bare-metal scuff for a bounce, a long gouge for
//! a ricochet. Marks are stored in the frame they belong to (hull marks ride the hull, turret
//! marks traverse with the turret). A penetration is presented by the analytic aperture; a scuff
//! and a gouge are presented as MATERIAL by the vehicle shader (Inny Poziom Z5) from wound
//! records in the same aperture list — never as flat stamps floating over the plate.

use std::sync::Arc;

use game_core::{ArmorFacing, ArmorZone, DamageEvent};
use glam::{Mat3, Vec3};
use net::TankSnapshot;
use renderer_api::{ArmorApertureRender, ArmorMarkKind, FxVertex};
use vehicle_geometry::{DecalPatch, MeshContactIndex, SurfaceContact};

use crate::vehicle::asset_catalog::VehicleContactIndex;
use crate::vehicle::pose::VehiclePose;
use crate::vehicle::variation::{DecalFrame, DecalKind, HitDecal};

/// Half-extent of a penetration hole's conformal patch (matches the penetration decal radius).
const PENETRATION_RADIUS_M: f32 = 0.13;
/// Cap on a patch's clipped triangles — bounds per-tank memory (`MAX_HIT_DECALS` x this).
const PATCH_TRIANGLE_CAP: usize = 64;

/// Start the visual-mesh ray this far back along the shell's heading, ahead of the collision hit,
/// so it always begins outside the (inset) mesh and enters the real surface.
const RAY_BACK_OFF_M: f32 = 0.6;
/// Cast at most this far forward: the shell line only has to cross the collision-to-visual gap.
const RAY_MAX_LEN_M: f32 = 1.2;
/// A ray contact this far or more from the collision hit is a different surface, not the gap —
/// reject it and fall back rather than teleport the mark across the tank.
const RAY_ACCEPT_M: f32 = 0.35;
/// When the ray grazes past the inset mesh, snap the mark to the nearest surface within this.
const NEAREST_SNAP_M: f32 = 0.30;

/// Resolve one replicated shell strike into a decal seated on the target's VISUAL armor. The
/// gameplay hit lands on the coarse armor volumes (inset from the detailed mesh), so the raw hit
/// point floats proud of the surface; casting the shell's line against the real mesh — with a
/// nearest-point snap and an other-frame retry for the turret-ring seam — recovers the true
/// contact. Falls back through the transmitted plate normal to the old cardinal guess, so it is
/// never worse than before. Returns `None` for track-zone hits (the belt is animated gear, not
/// a decal frame) and when the hit degenerates (a corrupt event).
pub fn decal_from_damage_event(
    event: &DamageEvent,
    target: &TankSnapshot,
    contact: Option<&VehicleContactIndex>,
) -> Option<HitDecal> {
    // A mark cannot ride a scrolling belt: the track zones' visible surface is the ANIMATED
    // running gear, which none of the decal frames (hull bake, turret, gun) carry. These hits
    // keep their impact FX and the track's own damage read (HP tiers, the thrown-belt visuals);
    // they leave no hull decal. Until the fender band was measured correctly the seat quietly
    // snapped these marks onto the fender lip that happened to hug the belt — a static scar on
    // sheet metal the shell never touched. The honest daylight band exposed that lie.
    if matches!(event.armor_zone, ArmorZone::LeftTrack | ArmorZone::RightTrack) {
        return None;
    }
    let pose = pose_of(target);
    let facing = event.armor_zone.facing();
    let primary = if event.armor_zone == game_core::ArmorZone::Mantlet {
        DecalFrame::Mantlet
    } else {
        frame_for_facing(facing)
    };

    let seat = seat_on_visual_mesh(event, &pose, primary, contact)
        .unwrap_or_else(|| seat_by_fallback(event, &pose, primary, facing));

    if !seat.local_position.is_finite() || !seat.local_normal.is_finite() {
        return None;
    }
    let kind = if event.penetrated {
        DecalKind::Penetration
    } else if event.ricocheted {
        DecalKind::Gouge
    } else {
        DecalKind::Scuff
    };
    // The mark's in-plane direction: a ricochet's gouge runs along the departure (the shell
    // direction reflected off the plate), any other mark along the shell's own travel across
    // the plate — both projected onto the plate and taken into the seat's frame. A round that
    // came in dead square has no in-plane travel; any perpendicular does then.
    let (_, basis) = frame_basis(&pose, seat.frame);
    let world_normal = (basis * seat.local_normal).normalize_or_zero();
    let travel = if event.ricocheted {
        let d = event.shell_direction;
        let n = event.plate_normal;
        d - n * (2.0 * d.dot(n))
    } else {
        event.shell_direction
    };
    let in_plane = travel - world_normal * travel.dot(world_normal);
    let world_tangent = if in_plane.length_squared() > 1.0e-6 {
        in_plane.normalize()
    } else {
        stable_perpendicular(world_normal)
    };
    let local_tangent = (basis.transpose() * world_tangent).normalize_or_zero();
    let radius = match kind {
        DecalKind::Penetration => 0.13,
        DecalKind::Scuff => 0.20,
        DecalKind::Gouge => 0.14,
    };
    // Only a penetration conforms to the casting; a scuff/gouge stays a flat stamp (soft, fading).
    let patch = if kind == DecalKind::Penetration { seat.patch } else { None };
    Some(HitDecal {
        local_position: seat.local_position.to_array(),
        local_normal: seat.local_normal.to_array(),
        local_tangent: local_tangent.to_array(),
        radius,
        age_s: 0.0,
        kind,
        frame: seat.frame,
        patch,
    })
}

/// A resolved decal seat: where the mark sits and which way it faces, in a chosen rotating frame,
/// plus (for a mesh-seated penetration) the conformal patch clipped from the visual mesh.
struct DecalSeat {
    local_position: Vec3,
    local_normal: Vec3,
    frame: DecalFrame,
    patch: Option<Arc<DecalPatch>>,
}

fn frame_for_facing(facing: ArmorFacing) -> DecalFrame {
    match facing {
        ArmorFacing::TurretFront | ArmorFacing::TurretSide | ArmorFacing::TurretRear => {
            DecalFrame::Turret
        }
        _ => DecalFrame::Hull,
    }
}

fn frame_basis(pose: &VehiclePose, frame: DecalFrame) -> (Vec3, Mat3) {
    match frame {
        DecalFrame::Hull => (pose.hull_translation(), pose.hull_basis()),
        DecalFrame::Turret => (pose.turret_translation(), pose.turret_basis()),
        DecalFrame::Mantlet => (pose.gun_translation(), pose.gun_basis()),
    }
}

fn frame_index(contact: &VehicleContactIndex, frame: DecalFrame) -> &MeshContactIndex {
    match frame {
        DecalFrame::Hull => &contact.hull,
        DecalFrame::Turret => &contact.turret,
        DecalFrame::Mantlet => &contact.gun,
    }
}

/// Cast the shell line against the visual mesh, trying the zone's frame first and the other frame
/// second (the turret-ring seam can zone a hull-lip hit as turret, or vice versa).
fn seat_on_visual_mesh(
    event: &DamageEvent,
    pose: &VehiclePose,
    primary: DecalFrame,
    contact: Option<&VehicleContactIndex>,
) -> Option<DecalSeat> {
    let contact = contact?;
    let direction = event.shell_direction;
    if direction.length_squared() < 1.0e-6 {
        return None;
    }
    let other = match primary {
        DecalFrame::Hull => DecalFrame::Turret,
        DecalFrame::Turret | DecalFrame::Mantlet => DecalFrame::Hull,
    };
    seat_in_frame(event, pose, primary, frame_index(contact, primary), direction)
        .or_else(|| seat_in_frame(event, pose, other, frame_index(contact, other), direction))
}

fn seat_in_frame(
    event: &DamageEvent,
    pose: &VehiclePose,
    frame: DecalFrame,
    index: &MeshContactIndex,
    world_direction: Vec3,
) -> Option<DecalSeat> {
    let (origin, basis) = frame_basis(pose, frame);
    let inv = basis.transpose();
    let local_hit = inv * (event.hit_position - origin);
    let local_dir = (inv * world_direction).normalize_or_zero();
    if !local_hit.is_finite() || local_dir == Vec3::ZERO {
        return None;
    }
    // Ray from just outside the surface, along the shell, accepting only a contact inside the
    // collision-to-visual gap; otherwise snap to the nearest surface.
    let ray_origin = local_hit - local_dir * RAY_BACK_OFF_M;
    let contact = index
        .raycast(ray_origin, local_dir, RAY_MAX_LEN_M)
        .filter(|hit| hit.position.distance(local_hit) <= RAY_ACCEPT_M)
        .or_else(|| index.nearest_point(local_hit, NEAREST_SNAP_M))?;

    // Face the mark outward, toward the incoming shell.
    let mut normal = contact.normal.normalize_or_zero();
    if normal == Vec3::ZERO {
        return None;
    }
    if normal.dot(local_dir) > 0.0 {
        normal = -normal;
    }
    // A penetration hole wraps the casting: clip the mesh under it into a conformal patch (in this
    // same local frame). Scuffs/gouges skip it — they stay flat stamps.
    let patch = event.penetrated.then(|| {
        let oriented = SurfaceContact { normal, ..contact };
        Arc::new(index.clip_patch(&oriented, PENETRATION_RADIUS_M, PATCH_TRIANGLE_CAP))
    });
    Some(DecalSeat { local_position: contact.position, local_normal: normal, frame, patch })
}

/// No visual-mesh contact: keep the mark at the collision hit and orient it by the transmitted
/// plate normal (v19), or — for a pre-v19 event that carries none — the old cardinal facing.
fn seat_by_fallback(
    event: &DamageEvent,
    pose: &VehiclePose,
    frame: DecalFrame,
    facing: ArmorFacing,
) -> DecalSeat {
    let (origin, basis) = frame_basis(pose, frame);
    let inv = basis.transpose();
    let local = inv * (event.hit_position - origin);
    let local_normal = if event.plate_normal.length_squared() > 1.0e-6 {
        (inv * event.plate_normal).normalize_or_zero()
    } else {
        local_facing_normal(facing, local.x)
    };
    // No visual-mesh contact to clip against: the mark falls back to a flat quad.
    DecalSeat { local_position: local, local_normal, frame, patch: None }
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
/// Any unit vector perpendicular to `n` (for a mark with no in-plane travel of its own).
fn stable_perpendicular(n: Vec3) -> Vec3 {
    let mut u = n.cross(Vec3::Y);
    if u.length_squared() < 1.0e-6 {
        u = n.cross(Vec3::X);
    }
    u.normalize_or_zero()
}

/// A deterministic phase in [0, 2π) from a mark's local seat, so its irregular contour is the
/// same on every frame and every client.
fn seat_phase(seat: [f32; 3], salt: u32) -> f32 {
    let mut h = salt.wrapping_mul(0x9E37_79B9);
    for v in seat {
        h ^= v.to_bits().wrapping_mul(0x85EB_CA6B);
        h = h.rotate_left(13).wrapping_mul(0xC2B2_AE35);
    }
    (h >> 8) as f32 / (1u32 << 24) as f32 * std::f32::consts::TAU
}

/// The wound RECORDS of a tank's scuffs and gouges, posed into the world for this frame, for the
/// aperture list the vehicle shader reads (Inny Poziom Z5). A penetration contributes nothing
/// here: the breach path (`armor_damage_instance`) owns the hole and its soot. A scuff is a
/// round-ish dish of the mark's radius; a gouge is three radii long along its tangent — the
/// ricochet's departure — and under half a radius across.
pub fn hit_wound_records(decals: &[HitDecal], tank: &TankSnapshot) -> Vec<ArmorApertureRender> {
    if decals.is_empty() {
        return Vec::new();
    }
    let pose = pose_of(tank);
    let mut records = Vec::new();
    for decal in decals {
        let kind = match decal.kind {
            DecalKind::Penetration => continue,
            DecalKind::Scuff => ArmorMarkKind::Scuff,
            DecalKind::Gouge => ArmorMarkKind::Gouge,
        };
        let (origin, basis) = frame_basis(&pose, decal.frame);
        let center = origin + basis * Vec3::from_array(decal.local_position);
        let normal = (basis * Vec3::from_array(decal.local_normal)).normalize_or_zero();
        let mut tangent = basis * Vec3::from_array(decal.local_tangent);
        tangent -= normal * tangent.dot(normal);
        let tangent = if tangent.length_squared() > 1.0e-6 {
            tangent.normalize()
        } else {
            stable_perpendicular(normal)
        };
        let r = decal.radius.max(0.02);
        let (major, minor, irregularity) = match kind {
            ArmorMarkKind::Scuff => (r, r * 0.9, 0.22),
            ArmorMarkKind::Gouge => (r * 3.0, r * 0.6, 0.10),
            ArmorMarkKind::Breach => unreachable!("breaches never reach the wound list"),
        };
        records.push(ArmorApertureRender {
            center: center.to_array(),
            normal: normal.to_array(),
            tangent: tangent.to_array(),
            major_radius_m: major,
            minor_radius_m: minor,
            rotation_rad: 0.0,
            irregularity,
            phase_a: seat_phase(decal.local_position, 7),
            phase_b: seat_phase(decal.local_position, 29),
            // The plate band the wound reads within: a seat on a curved casting (or a fallback
            // seat off the hitbox plane) sits some centimetres off the drawn skin, and the
            // contour still bounds the wound laterally — so the band is generous, like the
            // breaches', short of the plate behind.
            half_depth_m: (r * 1.5).clamp(0.12, 0.35),
            glow: 0.0,
            glow_tightness: 1.0,
            cut: false,
            kind,
        });
    }
    records
}

/// Merge a tank's wound records into this frame's damage list: onto the instance its breaches
/// already own, or as a fresh instance when the hull has no breach yet.
pub fn append_hit_wounds_to(
    damage: &mut Vec<renderer_api::ArmorDamageInstance>,
    decals: &[HitDecal],
    tank: &TankSnapshot,
) {
    let records = hit_wound_records(decals, tank);
    if records.is_empty() {
        return;
    }
    if let Some(instance) = damage.iter_mut().find(|instance| instance.tank_id == tank.tank_id) {
        instance.apertures.extend(records);
    } else {
        damage
            .push(renderer_api::ArmorDamageInstance { tank_id: tank.tank_id, apertures: records });
    }
}

/// The oriented plane a decal stamps onto: its center (already lifted off the armor) and the
/// two in-plane axes. Shared with the terrain scars, which stamp the same way onto the ground.
#[derive(Clone, Copy)]
pub(super) struct Plate {
    pub(super) center: Vec3,
    pub(super) u: Vec3,
    pub(super) v: Vec3,
}

/// Draw the hard entry hole as a conformal patch: the clipped mesh triangles (local frame) posed
/// into the world, each vertex carrying its decal-plane UV so the FX pass does the same radial
/// falloff a flat stamp would — but now wrapped to the casting instead of hovering across it.
/// One oriented quad on the plate with half-extents along its axes and an edge sharpness.
pub(super) fn push_stamp(
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
pub(super) fn premul(tone: [f32; 3], alpha: f32) -> [f32; 4] {
    [tone[0] * alpha, tone[1] * alpha, tone[2] * alpha, alpha]
}

/// The shared pose both ingest and render use, so the local frame round-trips exactly. Attitude
/// carries the authoritative hull pitch/roll; the presentation spring's extra theatrics are a
/// couple of degrees and not worth a divergent frame.
pub(crate) fn pose_of(tank: &TankSnapshot) -> VehiclePose {
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
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            rack_fire_remaining_s: None,
            crew_unconscious_mask: 0,
            crew_weakened_mask: 0,
            crew_down_remaining_s: Default::default(),
            hull_pitch_velocity_rad_s: 0.0,
            hull_roll_velocity_rad_s: 0.0,
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
    fn a_penetration_leaves_no_wound_record_because_the_breach_owns_it() {
        let tank = target(0.7, 0.0);
        let hit = Vec3::new(41.5, 2.8, 61.0);
        let decal =
            decal_from_damage_event(&event(hit, ArmorZone::HullSide, true, false), &tank, None)
                .expect("decal resolves");
        assert!(
            hit_wound_records(&[decal], &tank).is_empty(),
            "the aperture and rim own penetration presentation"
        );
    }

    #[test]
    fn turret_wounds_traverse_with_the_turret_while_hull_wounds_stay() {
        let still = target(0.0, 0.0);
        let hit = Vec3::new(40.0, 3.6, 61.2); // on the turret front, ahead of the ring
        let decal = decal_from_damage_event(
            &event(hit, ArmorZone::TurretFront, false, false),
            &still,
            None,
        )
        .expect("decal resolves");
        assert_eq!(decal.frame, DecalFrame::Turret);

        // The same wound with the turret slewed 90 degrees: the mark must move with it.
        let at_zero = hit_wound_records(std::slice::from_ref(&decal), &still);
        let slewed = hit_wound_records(
            std::slice::from_ref(&decal),
            &target(0.0, std::f32::consts::FRAC_PI_2),
        );
        let moved =
            Vec3::from_array(at_zero[0].center).distance(Vec3::from_array(slewed[0].center));
        assert!(moved > 0.5, "a turret wound rides the traverse, moved {moved}");

        // A hull wound ignores the traverse entirely.
        let hull_decal = decal_from_damage_event(
            &event(Vec3::new(41.0, 2.4, 60.0), ArmorZone::HullSide, false, false),
            &still,
            None,
        )
        .expect("hull decal");
        let hull_zero = hit_wound_records(std::slice::from_ref(&hull_decal), &still);
        let hull_slewed = hit_wound_records(
            std::slice::from_ref(&hull_decal),
            &target(0.0, std::f32::consts::FRAC_PI_2),
        );
        assert_eq!(hull_zero[0].center, hull_slewed[0].center);
    }

    #[test]
    fn kinds_map_to_hole_scuff_and_gouge_and_every_mark_stays() {
        let tank = target(0.0, 0.0);
        let hit = Vec3::new(40.0, 2.4, 62.0);
        let hole =
            decal_from_damage_event(&event(hit, ArmorZone::UpperGlacis, true, false), &tank, None)
                .unwrap();
        let scuff =
            decal_from_damage_event(&event(hit, ArmorZone::UpperGlacis, false, false), &tank, None)
                .unwrap();
        let gouge =
            decal_from_damage_event(&event(hit, ArmorZone::UpperGlacis, false, true), &tank, None)
                .unwrap();
        assert_eq!(hole.kind, DecalKind::Penetration);
        assert_eq!(scuff.kind, DecalKind::Scuff);
        assert_eq!(gouge.kind, DecalKind::Gouge);

        // Permanence (Z5): a battle later every mark still reads at full strength.
        for decal in [&hole, &scuff, &gouge] {
            let aged = HitDecal { age_s: 1.0e4, ..decal.clone() };
            assert_eq!(aged.opacity(), 1.0);
        }

        // A gouge's record is a long groove; a scuff's a round-ish dish.
        let records = hit_wound_records(&[scuff.clone(), gouge.clone()], &tank);
        assert_eq!(records.len(), 2);
        let aspect = |r: &ArmorApertureRender| r.major_radius_m / r.minor_radius_m.max(1.0e-6);
        assert!(aspect(&records[0]) < 1.3, "a scuff dish is round-ish");
        assert!(aspect(&records[1]) >= 4.5, "a gouge is a long groove");
        assert!(records.iter().all(|r| !r.cut), "a wound never opens the plate");
    }

    /// The gouge runs along the ricochet's DEPARTURE — the shell direction reflected off the
    /// plate and laid onto it — never along a world axis (game-design §13.1).
    #[test]
    fn a_gouge_runs_along_the_ricochets_departure() {
        let tank = target(0.0, 0.0);
        let hit = Vec3::new(40.0, 2.4, 62.0);
        let mut raking = event(hit, ArmorZone::UpperGlacis, false, true);
        raking.shell_direction = Vec3::new(-0.55, -0.35, 0.76).normalize();
        raking.plate_normal = Vec3::new(0.0, 0.6, 0.8).normalize();
        let gouge = decal_from_damage_event(&raking, &tank, None).unwrap();
        let record = hit_wound_records(&[gouge], &tank).remove(0);
        let n = Vec3::from_array(record.normal);
        let t = Vec3::from_array(record.tangent);
        let d = raking.shell_direction;
        let departure = d - raking.plate_normal * (2.0 * d.dot(raking.plate_normal));
        let expected = (departure - n * departure.dot(n)).normalize();
        assert!(t.dot(n).abs() < 1.0e-3, "the groove lies in the plate");
        assert!(t.dot(expected) > 0.95, "the groove follows the departure: {t} vs {expected}");
    }
}
