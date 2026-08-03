//! Client-side fire cues, driven by the AUTHORITATIVE shot event (protocol v41).
//!
//! This used to derive a shot from `reload_remaining_s` jumping up between snapshots — a good
//! proxy, and not the thing itself. The proxy has holes it cannot close, and every one of them
//! silences a muzzle flash that really happened:
//!
//!   * a tank that fires and DIES inside one snapshot window never reports, because the
//!     derivation requires the shooter alive in the later snapshot;
//!   * a tank whose vehicle changed between snapshots is filtered out;
//!   * two shots inside one window collapse into one;
//!   * a point-blank shell born and absorbed between snapshots appears in no `shells` list
//!     either, so no signal for it exists anywhere on the client.
//!
//! A shot is a fact the server knows exactly once. It is replicated as that fact now, and the
//! muzzle position is still resolved from the shooter's pose because that is presentation.
use game_core::math::{gun_direction_world, muzzle_world_position_scaled};
use game_core::{MountFrames, ShotFired, TankId};
use glam::Vec3;
use net::TankSnapshot;

/// One replicated shot, resolved to the world-space muzzle it left from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FireEvent {
    pub tank_id: TankId,
    /// Hull-relative turret heading at the shot â€” the hull-rock impulse decomposes over it.
    pub turret_yaw_rad: f32,
    pub muzzle: Vec3,
    pub direction: Vec3,
}

/// Resolve this tick's replicated shots against the tanks that fired them. `player` fires from its
/// installed (possibly non-stock) barrel via `player_barrel_scale`; remote tanks are stock.
///
/// A shot whose shooter is not in this snapshot — killed and removed, or filtered out — still
/// happened, and is simply not drawable: there is no pose to hang a flash on. That is a missing
/// POSE, not a missing shot, which is the distinction the reload proxy could never make.
pub(crate) fn resolve_shots(
    shots: &[ShotFired],
    latest: &[TankSnapshot],
    player: TankId,
    player_barrel_scale: f32,
) -> Vec<FireEvent> {
    shots
        .iter()
        .filter_map(|shot| {
            let tank = latest.iter().find(|tank| tank.tank_id == shot.shooter)?;
            let mounts = MountFrames::for_vehicle(tank.vehicle);
            let barrel_scale = if tank.tank_id == player { player_barrel_scale } else { 1.0 };
            let muzzle = muzzle_world_position_scaled(
                &mounts,
                Vec3::from_array(tank.position),
                tank.hull_pose(),
                tank.turret_yaw_rad,
                tank.gun_pitch_rad,
                barrel_scale,
            );
            let direction =
                gun_direction_world(tank.hull_pose(), tank.turret_yaw_rad, tank.gun_pitch_rad);
            Some(FireEvent {
                tank_id: tank.tank_id,
                turret_yaw_rad: tank.turret_yaw_rad,
                muzzle,
                direction,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use game_core::{ShellId, TeamId, VehicleKind};

    use super::*;

    fn snapshot(id: u64, reload_remaining_s: f32, hit_points: u32) -> TankSnapshot {
        TankSnapshot {
            tank_id: TankId(id),
            team: TeamId(1),
            vehicle: VehicleKind::T54_1951,
            position: [10.0, 2.0, 20.0],
            yaw_rad: 0.3,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.2,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.05,
            hit_points,
            reload_remaining_s,
            aim_dispersion_mrad: 1.0,
            module_hit_points: [100; game_core::MODULE_SLOT_COUNT],
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
        }
    }

    fn shot(id: u64) -> ShotFired {
        ShotFired { shooter: TankId(id), shell_id: ShellId::from_shot(TankId(id), 0) }
    }

    #[test]
    fn a_replicated_shot_resolves_to_the_muzzle_it_left() {
        let tanks = vec![snapshot(1, 8.4, 1000)];

        let events = resolve_shots(&[shot(1)], &tanks, TankId(1), 1.0);

        assert_eq!(events.len(), 1);
        let event = events[0];
        assert_eq!(event.tank_id, TankId(1));
        assert_eq!(event.turret_yaw_rad, 0.2);
        // The muzzle sits meters from the hull origin, along the composed gun chain.
        let offset = event.muzzle - Vec3::new(10.0, 2.0, 20.0);
        assert!(offset.length() > 2.0, "muzzle is out on the barrel: {offset}");
        assert!((event.direction.length() - 1.0).abs() < 1.0e-4);
        assert!(event.direction.dot(offset.normalize()) > 0.95, "direction points out the barrel");
    }

    /// THE HOLE THE PROXY COULD NOT CLOSE. A tank that fires and dies inside one snapshot window
    /// showed no flash at all, because the derivation required the shooter alive in the later
    /// snapshot. The shot happened; the shell is in the air; the gun flashed.
    #[test]
    fn a_tank_that_fired_and_died_in_the_same_window_still_flashes() {
        let dead = vec![snapshot(1, 8.4, 0)];

        let events = resolve_shots(&[shot(1)], &dead, TankId(1), 1.0);

        assert_eq!(events.len(), 1, "the gun fired before the hull died — that is not undone");
    }

    /// Two shots inside one window are two flashes. The reload proxy could only ever report one,
    /// because a clock has one value.
    #[test]
    fn two_shots_in_one_window_are_two_cues() {
        let tanks = vec![snapshot(1, 8.4, 1000), snapshot(2, 8.4, 900)];

        let events = resolve_shots(&[shot(1), shot(2)], &tanks, TankId(1), 1.0);

        assert_eq!(events.len(), 2, "remote shots get muzzle FX too");
    }

    /// No shots means no cues — a ticking reload is not an event, which is the whole point.
    #[test]
    fn a_ticking_reload_is_not_a_shot() {
        let tanks = vec![snapshot(1, 2.85, 1000)];
        assert!(resolve_shots(&[], &tanks, TankId(1), 1.0).is_empty());
    }

    /// A shooter with no pose in this snapshot is a missing POSE, not a missing shot: there is
    /// nothing to hang a flash on, and nothing is invented.
    #[test]
    fn a_shot_from_a_tank_with_no_pose_draws_nothing() {
        assert!(resolve_shots(&[shot(9)], &[snapshot(1, 8.4, 1000)], TankId(1), 1.0).is_empty());
    }
}
