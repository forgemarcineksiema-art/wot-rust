use game_core::{TankId, TankSpec, TeamId, TrackHealth};
use glam::Vec3;

use crate::tank_state::TankState;

/// A factory-fresh tank: full health, healthy modules and tracks, the spec's ammo rack loaded,
/// and a level, stationary hull at the given position/heading.
pub(crate) fn fresh_tank(
    id: TankId,
    team: TeamId,
    spec: TankSpec,
    position: Vec3,
    yaw_rad: f32,
) -> TankState {
    let modules = spec.module_health;
    let aim_dispersion_mrad = spec.gun.dispersion_mrad;
    let ammo_counts = spec.ammo.counts;
    let selected_ammo = spec.ammo.initial_selected;
    TankState {
        id,
        team,
        hit_points: spec.hit_points,
        spec,
        last_shot_tick: None,
        rack_fire: false,
        rack_fire_s: 0.0,
        rack_fire_source: None,
        crew: game_core::CrewVitals::default(),
        position,
        yaw_rad,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        velocity_mps: Vec3::ZERO,
        hull_yaw_velocity_rad_s: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        reload_remaining_s: 0.0,
        fire_buffered: false,
        aim_dispersion_mrad,
        dispersion_shot_index: 0,
        tracks: TrackHealth::healthy(),
        modules,
        ammo_counts,
        selected_ammo,
        spotted_mask: 0,
        submerged_s: 0.0,
        repair: crate::repair::CrewRepair::default(),
        turret_detached: false,
        armor_breaches: game_core::ArmorBreachSet::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        fire_source: None,
        fire_s: 0.0,
    }
}
