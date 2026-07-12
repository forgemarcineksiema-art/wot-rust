use game_core::math::world_to_tank_local;
use game_core::{ArmorBreach, ArmorFrame, ArmorMaterial, ArmorSurfaceId, ArmorZone};
use glam::{Mat3, Vec3};

use crate::TankState;

pub(crate) fn frame_for_zone(zone: ArmorZone) -> ArmorFrame {
    match zone {
        ArmorZone::Mantlet => ArmorFrame::Mantlet,
        ArmorZone::TurretFront
        | ArmorZone::TurretSide
        | ArmorZone::TurretRear
        | ArmorZone::Roof => ArmorFrame::Turret,
        _ => ArmorFrame::Hull,
    }
}

pub(crate) fn world_to_breach_frame(world: Vec3, tank: &TankState, frame: ArmorFrame) -> Vec3 {
    let mut local =
        world_to_tank_local(world, tank.position, tank.spec.hitbox.center_y_m, tank.hull_pose());
    local += Vec3::Y * tank.spec.hitbox.center_y_m;
    if matches!(frame, ArmorFrame::Turret | ArmorFrame::Mantlet) {
        let pivot = tank.spec.mounts.turret_ring.translation;
        local = pivot + Mat3::from_rotation_y(-tank.turret_yaw_rad) * (local - pivot);
    }
    if frame == ArmorFrame::Mantlet {
        let pivot = tank.spec.mounts.gun_trunnion.translation;
        local = pivot + Mat3::from_rotation_x(-tank.gun_pitch_rad) * (local - pivot);
    }
    local
}

pub(crate) fn vector_to_breach_frame(world: Vec3, tank: &TankState, frame: ArmorFrame) -> Vec3 {
    let mut local = tank.hull_pose().basis().transpose() * world;
    if matches!(frame, ArmorFrame::Turret | ArmorFrame::Mantlet) {
        local = Mat3::from_rotation_y(-tank.turret_yaw_rad) * local;
    }
    if frame == ArmorFrame::Mantlet {
        local = Mat3::from_rotation_x(-tank.gun_pitch_rad) * local;
    }
    local.normalize_or_zero()
}

pub(crate) struct BreachImpact {
    pub zone: ArmorZone,
    pub hit_position: Vec3,
    pub plate_normal: Vec3,
    pub direction: Vec3,
    pub caliber_mm: f32,
    pub effective_armor_mm: f32,
    pub residual_penetration_mm: f32,
}

pub(crate) fn make_breach(tank: &TankState, impact: BreachImpact) -> ArmorBreach {
    let BreachImpact {
        zone,
        hit_position,
        plate_normal,
        direction,
        caliber_mm,
        effective_armor_mm,
        residual_penetration_mm,
    } = impact;
    let frame = frame_for_zone(zone);
    let entry_local = world_to_breach_frame(hit_position, tank, frame);
    let direction_local = vector_to_breach_frame(direction, tank, frame);
    let normal_local = vector_to_breach_frame(plate_normal, tank, frame);
    let thickness_m = (effective_armor_mm / 1000.0).clamp(0.01, 0.40);
    ArmorBreach {
        surface: ArmorSurfaceId::new(frame, zone),
        frame,
        zone,
        material: match frame {
            ArmorFrame::Hull => ArmorMaterial::RolledSteel,
            ArmorFrame::Turret | ArmorFrame::Mantlet => ArmorMaterial::CastSteel,
        },
        entry_local,
        exit_local: entry_local + direction_local * thickness_m,
        entry_normal_local: normal_local,
        exit_normal_local: -normal_local,
        direction_local,
        radius_m: (caliber_mm / 2000.0 * 1.18).clamp(0.035, 0.19),
        thickness_m,
        residual_penetration_mm,
    }
}

pub(crate) fn admits_existing_channel(
    tank: &TankState,
    zone: ArmorZone,
    hit_position: Vec3,
    projectile_radius_m: f32,
) -> bool {
    let frame = frame_for_zone(zone);
    let local = world_to_breach_frame(hit_position, tank, frame);
    tank.armor_breaches.passage_at(frame, local, projectile_radius_m).is_some()
}
