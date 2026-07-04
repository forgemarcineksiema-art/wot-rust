//! The high-explosive burst: blast damage thrown past the impact point at everything inside the
//! shell's explosive radius, attenuated by distance and soaked by the victim's thinnest external
//! plate — the blast finds the roof and the engine deck, not the glacis, so heavies shrug off
//! what mediums feel.

use game_core::math::world_to_tank_local;
use game_core::{
    ArmorFacing, ArmorProfile, ArmorZone, DamageCause, DamageEvent, ShellType, TankId,
};
use glam::Vec3;

use crate::{ShellState, TankState};

const SPLASH_DAMAGE_FACTOR: f32 = 0.5;
const SPLASH_ARMOR_ABSORPTION: f32 = 1.3;

/// A high-explosive burst throws damage past its impact point: every vehicle inside the
/// explosive radius takes attenuated blast damage. The directly-struck tank already took the
/// surface-burst damage from the armor test and is skipped; allies are protected exactly like
/// direct fire, but the owner's own HE can absolutely hurt the owner.
pub(crate) fn burst_he_splash(
    shell: &ShellState,
    burst_point: Vec3,
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    direct_target: Option<TankId>,
) {
    let radius = shell.shell.explosive_radius_m;
    if shell.shell.shell_type != ShellType::HighExplosive || radius <= 0.0 {
        return;
    }
    let owner_team = tanks.iter().find(|tank| tank.id == shell.owner).map(|tank| tank.team);
    for tank in tanks.iter_mut() {
        if tank.hit_points == 0 || Some(tank.id) == direct_target {
            continue;
        }
        if Some(tank.team) == owner_team && tank.id != shell.owner {
            continue;
        }
        let falloff = 1.0 - distance_to_hull_m(burst_point, tank) / radius;
        if falloff <= 0.0 {
            continue;
        }
        let soaked = thinnest_external_plate_mm(&tank.spec.hull) * SPLASH_ARMOR_ABSORPTION;
        let damage =
            (shell.shell.damage_hp as f32 * SPLASH_DAMAGE_FACTOR * falloff - soaked).round();
        if damage < 1.0 {
            continue;
        }
        let damage = damage as u32;
        tank.hit_points = tank.hit_points.saturating_sub(damage);
        damage_events.push(DamageEvent {
            source: shell.owner,
            target: tank.id,
            hit_position: burst_point,
            damage_hp: damage,
            penetrated: false,
            cause: DamageCause::Splash,
            shell_type: shell.shell.shell_type,
            ..Default::default()
        });
    }
}

/// Distance from the burst point to the tank's hull surface (its hitbox slab), not its center —
/// a burst against the side skirt is a contact burst, whatever the hull's width.
fn distance_to_hull_m(point: Vec3, tank: &TankState) -> f32 {
    let hitbox = tank.spec.hitbox;
    let local = world_to_tank_local(point, tank.position, hitbox.center_y_m, tank.hull_pose());
    let half = Vec3::new(hitbox.half_width_m, hitbox.half_height_m, hitbox.half_length_m);
    local.distance(local.clamp(-half, half))
}

/// The plate a wrapping blast actually finds: the thinnest of roof, side, and rear steel.
fn thinnest_external_plate_mm(armor: &ArmorProfile) -> f32 {
    armor
        .plate(ArmorZone::Roof)
        .nominal_thickness_mm
        .min(armor.nominal_thickness_mm(ArmorFacing::HullSide))
        .min(armor.nominal_thickness_mm(ArmorFacing::HullRear))
}
