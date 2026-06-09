use game_core::math::gun_direction;
use game_core::{HitboxProfile, TankId};
use glam::Vec3;
use net::TankSnapshot;
use sim::{
    DEFAULT_SIMULATION_TICK_HZ, SHELL_MAX_AGE_SECONDS, ShellTraceWorld, TraceOutcome, TraceTank,
    trace_shell,
};
use terrain::{HeightMap, StaticCoverObject};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReticleTraceQuery<'a> {
    pub heightmap: &'a HeightMap,
    pub cover: &'a [StaticCoverObject],
    pub tanks: &'a [TankSnapshot],
    pub owner: TankId,
    pub muzzle: Vec3,
    pub yaw_rad: f32,
    pub pitch_rad: f32,
    pub muzzle_velocity_mps: f32,
}

/// Trace the player's shot with the authoritative shell physics ([`sim::trace_shell`]). The reticle
/// preview and the server run the same trajectory + collision, so a previewed impact is one the
/// server will confirm — even under input latency.
pub(crate) fn reticle_trace(query: ReticleTraceQuery<'_>) -> TraceOutcome {
    let velocity = gun_direction(query.yaw_rad, query.pitch_rad) * query.muzzle_velocity_mps;
    let targets = trace_targets(query.tanks, query.owner);
    let world =
        ShellTraceWorld { tanks: &targets, heightmap: Some(query.heightmap), cover: query.cover };
    trace_shell(query.muzzle, velocity, tick_dt_seconds(), SHELL_MAX_AGE_SECONDS, &world)
}

/// Living enemy tanks as neutral trace targets. The client cannot read teams from a snapshot, so it
/// excludes only the owner and the dead; friendly-fire filtering stays a server-side concern.
pub(crate) fn trace_targets(tanks: &[TankSnapshot], owner: TankId) -> Vec<TraceTank> {
    tanks
        .iter()
        .filter(|tank| tank.tank_id != owner && tank.hit_points > 0)
        .map(|tank| TraceTank {
            id: tank.tank_id,
            position: Vec3::from_array(tank.position),
            yaw_rad: tank.yaw_rad,
            turret_yaw_rad: tank.turret_yaw_rad,
            hitbox: HitboxProfile::for_vehicle(tank.vehicle),
        })
        .collect()
}

/// The authoritative simulation timestep — the preview must integrate at the server's `dt`, not a
/// finer one, or its arc would drift from the server's.
pub(crate) fn tick_dt_seconds() -> f32 {
    1.0 / DEFAULT_SIMULATION_TICK_HZ as f32
}

#[cfg(test)]
mod tests {
    use game_core::VehicleKind;

    use super::*;

    fn enemy(id: u64, position: [f32; 3]) -> TankSnapshot {
        TankSnapshot {
            tank_id: TankId(id),
            vehicle: VehicleKind::T55A,
            position,
            yaw_rad: std::f32::consts::PI,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 2.9,
            module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
        }
    }

    #[test]
    fn reticle_trace_delegates_to_the_authoritative_trace_and_hits_the_enemy() {
        let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
        let target = enemy(2, [40.0, 0.0, 80.0]);
        let muzzle = Vec3::new(40.0, 1.5, 40.0);

        let outcome = reticle_trace(ReticleTraceQuery {
            heightmap: &heightmap,
            cover: &[],
            tanks: std::slice::from_ref(&target),
            owner: TankId(1),
            muzzle,
            yaw_rad: 0.0,
            pitch_rad: 0.0,
            muzzle_velocity_mps: 895.0,
        });

        // The same inputs fed straight through `sim` must produce the identical outcome — the
        // reticle is a thin adapter over the authoritative trace, nothing more.
        let velocity = gun_direction(0.0, 0.0) * 895.0;
        let targets = trace_targets(std::slice::from_ref(&target), TankId(1));
        let world = ShellTraceWorld { tanks: &targets, heightmap: Some(&heightmap), cover: &[] };
        let direct =
            trace_shell(muzzle, velocity, tick_dt_seconds(), SHELL_MAX_AGE_SECONDS, &world);

        assert_eq!(outcome, direct);
        assert!(
            matches!(outcome, TraceOutcome::Tank { id, .. } if id == TankId(2)),
            "level shot down the line of fire should hit the enemy, got {outcome:?}"
        );
    }
}
