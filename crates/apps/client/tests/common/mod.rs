//! Shared battle-scene fixtures for the client's integration tests.
#![allow(dead_code)]

use game_core::TankId;
use net::TankSnapshot;

/// A parked T-54 snapshot with everything else at rest — the camera tests' subject.
pub fn tank_snapshot(position: [f32; 3], hull_yaw_rad: f32, turret_yaw_rad: f32) -> TankSnapshot {
    let spec = game_core::VehicleKind::T54_1951.spec();
    TankSnapshot {
        tank_id: TankId(1),
        team: game_core::TeamId(1),
        vehicle: spec.kind,
        position,
        yaw_rad: hull_yaw_rad,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
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
