use game_core::{TankId, TeamId};
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
    /// The map's standing water — the preview must splash exactly where the server will.
    pub water: Option<terrain::WaterBody>,
    pub tanks: &'a [TankSnapshot],
    pub owner: TankId,
    pub owner_team: TeamId,
    pub muzzle: Vec3,
    /// World-space unit direction of the barrel â€” already carried through the hull pose, so a
    /// tilted hull previews the tilted arc the server will fire.
    pub gun_direction: Vec3,
    pub muzzle_velocity_mps: f32,
    pub projectile_radius_m: f32,
    /// The previewed shell's linear drag â€” the preview must fly the same air as the server.
    pub drag_per_s: f32,
}

/// Trace the player's shot with the authoritative shell physics ([`sim::trace_shell`]). The reticle
/// preview and the server run the same trajectory + collision, so a previewed impact is one the
/// server will confirm â€” even under input latency.
pub(crate) fn reticle_trace(query: ReticleTraceQuery<'_>) -> TraceOutcome {
    let velocity = query.gun_direction.normalize_or_zero() * query.muzzle_velocity_mps;
    let sets = trace_sets(query.tanks, query.owner, query.owner_team);
    let world = ShellTraceWorld {
        projectile_radius_m: query.projectile_radius_m,
        tanks: &sets.targets,
        blockers: &sets.blockers,
        heightmap: Some(query.heightmap),
        cover: query.cover,
        water: query.water,
    };
    trace_shell(
        query.muzzle,
        velocity,
        query.drag_per_s,
        tick_dt_seconds(),
        SHELL_MAX_AGE_SECONDS,
        &world,
    )
}

/// The battle split the same way the authoritative server splits it (protocol v12 carries the
/// team): live enemies are damageable targets; teammates and wrecks are absorbing blockers.
#[derive(Debug, Default)]
pub(crate) struct TraceSets {
    pub targets: Vec<TraceTank>,
    pub blockers: Vec<TraceTank>,
}

pub(crate) fn trace_sets(tanks: &[TankSnapshot], owner: TankId, owner_team: TeamId) -> TraceSets {
    let mut sets = TraceSets::default();
    for tank in tanks {
        if tank.tank_id == owner {
            continue;
        }
        let trace = TraceTank::for_kind(
            tank.tank_id,
            Vec3::from_array(tank.position),
            tank.hull_pose(),
            tank.turret_yaw_rad,
            tank.vehicle,
        );
        if tank.hit_points > 0 && tank.team != owner_team {
            sets.targets.push(trace);
        } else {
            sets.blockers.push(trace);
        }
    }
    sets
}

/// The authoritative simulation timestep â€” the preview must integrate at the server's `dt`, not a
/// finer one, or its arc would drift from the server's.
pub(crate) fn tick_dt_seconds() -> f32 {
    1.0 / DEFAULT_SIMULATION_TICK_HZ as f32
}

#[cfg(test)]
mod tests {
    use game_core::math::gun_direction;
    use game_core::{ImpactSurface, VehicleKind};
    use sim::SegmentImpact;

    use super::*;

    fn snapshot(id: u64, team: u16, hit_points: u32, position: [f32; 3]) -> TankSnapshot {
        TankSnapshot {
            tank_id: TankId(id),
            team: TeamId(team),
            vehicle: VehicleKind::T55A,
            position,
            yaw_rad: std::f32::consts::PI,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 2.9,
            module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
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

    #[test]
    fn trace_sets_split_enemies_from_teammates_and_wrecks() {
        let tanks = [
            snapshot(1, 1, 1000, [0.0, 0.0, 0.0]),  // owner: excluded
            snapshot(2, 1, 1000, [10.0, 0.0, 0.0]), // live ally: blocker
            snapshot(3, 2, 1000, [20.0, 0.0, 0.0]), // live enemy: target
            snapshot(4, 2, 0, [30.0, 0.0, 0.0]),    // enemy wreck: blocker
            snapshot(5, 1, 0, [40.0, 0.0, 0.0]),    // ally wreck: blocker
        ];

        let sets = trace_sets(&tanks, TankId(1), TeamId(1));

        assert_eq!(sets.targets.iter().map(|t| t.id.0).collect::<Vec<_>>(), vec![3]);
        assert_eq!(sets.blockers.iter().map(|t| t.id.0).collect::<Vec<_>>(), vec![2, 4, 5]);
    }

    #[test]
    fn reticle_trace_reports_an_ally_block_as_an_obstacle_not_a_hit() {
        let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
        let ally = snapshot(2, 1, 1000, [40.0, 0.0, 60.0]);
        let enemy = snapshot(3, 2, 1000, [40.0, 0.0, 120.0]);
        let muzzle = Vec3::new(40.0, 1.5, 40.0);

        let outcome = reticle_trace(ReticleTraceQuery {
            heightmap: &heightmap,
            cover: &[],
            water: None,
            tanks: &[ally, enemy],
            owner: TankId(1),
            owner_team: TeamId(1),
            muzzle,
            gun_direction: Vec3::Z,
            muzzle_velocity_mps: 895.0,
            projectile_radius_m: 0.05,
            drag_per_s: 0.09,
        });

        match outcome {
            TraceOutcome::Obstacle { position, surface } => {
                assert_eq!(surface, ImpactSurface::Hull, "ally must absorb the previewed shot");
                assert!(position.z < 60.0, "block sits on the ally's near face");
            }
            other => panic!("expected an ally block, got {other:?}"),
        }
    }

    #[test]
    fn reticle_trace_delegates_to_the_authoritative_trace_and_hits_the_enemy() {
        let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
        let target = snapshot(2, 2, 1000, [40.0, 0.0, 80.0]);
        let muzzle = Vec3::new(40.0, 1.5, 40.0);

        let outcome = reticle_trace(ReticleTraceQuery {
            heightmap: &heightmap,
            cover: &[],
            water: None,
            tanks: std::slice::from_ref(&target),
            owner: TankId(1),
            owner_team: TeamId(1),
            muzzle,
            gun_direction: Vec3::Z,
            muzzle_velocity_mps: 895.0,
            projectile_radius_m: 0.05,
            drag_per_s: 0.09,
        });

        // The same inputs fed straight through `sim` must produce the identical outcome â€” the
        // reticle is a thin adapter over the authoritative trace, nothing more.
        let velocity = gun_direction(0.0, 0.0) * 895.0;
        let sets = trace_sets(std::slice::from_ref(&target), TankId(1), TeamId(1));
        let world = ShellTraceWorld {
            projectile_radius_m: 0.05,
            tanks: &sets.targets,
            blockers: &sets.blockers,
            heightmap: Some(&heightmap),
            cover: &[],
            water: None,
        };
        let direct =
            trace_shell(muzzle, velocity, 0.09, tick_dt_seconds(), SHELL_MAX_AGE_SECONDS, &world);

        assert_eq!(outcome, direct);
        assert!(
            matches!(outcome, TraceOutcome::Tank { id, .. } if id == TankId(2)),
            "level shot down the line of fire should hit the enemy, got {outcome:?}"
        );
        // And a segment-level sanity check that the shared kernel sees the same world.
        let segment =
            sim::segment_impact(muzzle, muzzle + Vec3::new(0.0, 0.0, 60.0), velocity, &world);
        assert!(matches!(segment, Some(SegmentImpact::Tank { id, .. }) if id == TankId(2)));
    }
}
