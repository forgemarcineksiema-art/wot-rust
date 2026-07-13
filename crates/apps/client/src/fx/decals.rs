//! Hit decals on vehicles: every replicated shell strike leaves a mark ON the plate it hit â€”
//! a permanent black hole for a penetration, a fading bare-metal scuff for a bounce, a long
//! gouge for a ricochet. Marks are stored in the frame they belong to (hull marks ride the
//! hull, turret marks traverse with the turret) and drawn as flat oriented quads through the
//! FX pass, so a battered tank *looks* battered without recolouring a single submesh.

use std::sync::Arc;

use game_core::{ArmorFacing, DamageEvent};
use glam::{Mat3, Vec3};
use net::TankSnapshot;
use renderer_api::FxVertex;
use vehicle_geometry::{DecalPatch, MeshContactIndex, SurfaceContact};

use crate::vehicle::asset_catalog::VehicleContactIndex;
use crate::vehicle::pose::VehiclePose;
use crate::vehicle::variation::{DecalFrame, DecalKind, HitDecal};

/// Half-extent of a penetration hole's conformal patch (matches the penetration decal radius).
const PENETRATION_RADIUS_M: f32 = 0.13;
/// Cap on a patch's clipped triangles — bounds per-tank memory (`MAX_HIT_DECALS` x this).
const PATCH_TRIANGLE_CAP: usize = 64;

/// How far off the plate the quad floats to never z-fight the armor it marks. Small now the mark
/// sits on the TRUE surface with the TRUE normal — 6 mm kills z-fighting without visible float.
const DECAL_LIFT_M: f32 = 0.006;
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
/// never worse than before. Returns `None` only when the hit degenerates (a corrupt event).
pub fn decal_from_damage_event(
    event: &DamageEvent,
    target: &TankSnapshot,
    contact: Option<&VehicleContactIndex>,
) -> Option<HitDecal> {
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
pub fn append_decal_quads(vertices: &mut Vec<FxVertex>, decals: &[HitDecal], tank: &TankSnapshot) {
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
            DecalFrame::Mantlet => (pose.gun_translation(), pose.gun_basis()),
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
            // The real aperture is cut by the vehicle/depth/shadow shaders. A flat FX primitive
            // here would close it again with exactly the black disk this system replaces.
            DecalKind::Penetration => {}
            DecalKind::Scuff => push_scuff(vertices, plate, decal, opacity),
            DecalKind::Gouge => push_gouge(vertices, plate, decal, opacity),
        }
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

/// A penetration is three layers, not one blob: a wide SOFT scorch halo (burnt paint), the
/// hard-edged near-black entry hole, and a fan of bright bare-metal splash streaks â€” the
/// signature look of a shaped hole in rolled armor, readable at battle range. When the decal
/// carries a conformal patch (protocol phase 2) the hard hole is drawn WRAPPED to the casting
/// instead of as a flat stamp; the soft halo and streaks stay flat (they read fine flat).
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
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
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
    fn a_penetration_decal_never_draws_a_fake_disk_or_starburst() {
        let tank = target(0.7, 0.0);
        let hit = Vec3::new(41.5, 2.8, 61.0);
        let decal =
            decal_from_damage_event(&event(hit, ArmorZone::HullSide, true, false), &tank, None)
                .expect("decal resolves");

        let mut vertices = Vec::new();
        append_decal_quads(&mut vertices, &[decal], &tank);
        assert!(vertices.is_empty(), "the aperture and rim own penetration presentation");
    }

    #[test]
    fn turret_marks_traverse_with_the_turret_while_hull_marks_stay() {
        let still = target(0.0, 0.0);
        let hit = Vec3::new(40.0, 3.6, 61.2); // on the turret front, ahead of the ring
        let decal = decal_from_damage_event(
            &event(hit, ArmorZone::TurretFront, false, false),
            &still,
            None,
        )
        .expect("decal resolves");
        assert_eq!(decal.frame, DecalFrame::Turret);

        // Re-render the same decal with the turret slewed 90 degrees: the mark must move.
        let mut at_zero = Vec::new();
        append_decal_quads(&mut at_zero, std::slice::from_ref(&decal), &still);
        let mut slewed = Vec::new();
        append_decal_quads(
            &mut slewed,
            std::slice::from_ref(&decal),
            &target(0.0, std::f32::consts::FRAC_PI_2),
        );
        let moved =
            Vec3::from_array(at_zero[0].position).distance(Vec3::from_array(slewed[0].position));
        assert!(moved > 0.5, "a turret mark rides the traverse, moved {moved}");

        // A hull mark ignores the traverse entirely.
        let hull_decal = decal_from_damage_event(
            &event(Vec3::new(41.0, 2.4, 60.0), ArmorZone::HullSide, false, false),
            &still,
            None,
        )
        .expect("hull decal");
        let mut hull_zero = Vec::new();
        append_decal_quads(&mut hull_zero, std::slice::from_ref(&hull_decal), &still);
        let mut hull_slewed = Vec::new();
        append_decal_quads(
            &mut hull_slewed,
            std::slice::from_ref(&hull_decal),
            &target(0.0, std::f32::consts::FRAC_PI_2),
        );
        assert_eq!(hull_zero[0].position, hull_slewed[0].position);
    }

    #[test]
    fn kinds_map_to_hole_scuff_and_gouge_with_correct_permanence() {
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

        // Permanence: an aged hole keeps full opacity, an aged scuff is gone.
        let aged_hole = HitDecal { age_s: 1.0e4, ..hole.clone() };
        let aged_scuff = HitDecal { age_s: 1.0e4, ..scuff.clone() };
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
