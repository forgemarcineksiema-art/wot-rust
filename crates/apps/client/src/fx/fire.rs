//! Client-side fire detection: the protocol replicates no explicit "tank fired" event, but a
//! shot is the only thing that *raises* `reload_remaining_s` between snapshots. Deriving fire
//! events from that jump works identically for the local player, allied tanks, and enemies â€”
//! one code path, no protocol change, and at the in-process 20 Hz snapshot cadence the cue lands
//! within two sim ticks of the authoritative shot.

use game_core::math::{gun_direction_world, muzzle_world_position_scaled};
use game_core::{MountFrames, TankId};
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

/// Reload can only jump UP by (nearly) the full reload on a shot; module repairs and HUD noise
/// move it by far less. Half the spec reload splits the two cleanly.
fn fired_between(previous: &TankSnapshot, latest: &TankSnapshot) -> bool {
    let reload_s = latest.vehicle.spec().gun.reload_seconds;
    latest.hit_points > 0
        && latest.vehicle == previous.vehicle
        && latest.reload_remaining_s - previous.reload_remaining_s > reload_s * 0.5
}

/// Diff two consecutive snapshots into the shots fired between them. `player` fires from its
/// installed (possibly non-stock) barrel via `player_barrel_scale`; remote tanks are stock.
pub(crate) fn detect_fired(
    previous: &[TankSnapshot],
    latest: &[TankSnapshot],
    player: TankId,
    player_barrel_scale: f32,
) -> Vec<FireEvent> {
    latest
        .iter()
        .filter_map(|tank| {
            let before = previous.iter().find(|prev| prev.tank_id == tank.tank_id)?;
            if !fired_between(before, tank) {
                return None;
            }
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
    use game_core::{TeamId, VehicleKind};

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
        }
    }

    #[test]
    fn a_reload_jump_is_a_shot_and_resolves_to_the_muzzle() {
        let reload = VehicleKind::T54_1951.spec().gun.reload_seconds;
        let before = vec![snapshot(1, 0.0, 1000)];
        let after = vec![snapshot(1, reload, 1000)];

        let events = detect_fired(&before, &after, TankId(1), 1.0);

        assert_eq!(events.len(), 1);
        let event = events[0];
        assert_eq!(event.tank_id, TankId(1));
        assert_eq!(event.turret_yaw_rad, 0.2);
        // The muzzle sits meters from the hull origin, along the composed gun chain.
        let offset = event.muzzle - Vec3::new(10.0, 2.0, 20.0);
        assert!(offset.length() > 2.0, "muzzle is out on the barrel: {offset}");
        // The direction matches the shared gun chain (unit length, mostly horizontal here).
        assert!((event.direction.length() - 1.0).abs() < 1.0e-4);
        assert!(event.direction.dot(offset.normalize()) > 0.95, "direction points out the barrel");
    }

    #[test]
    fn a_ticking_reload_or_a_dead_tank_is_not_a_shot() {
        // Reload counting down: no event.
        let before = vec![snapshot(1, 3.0, 1000)];
        let after = vec![snapshot(1, 2.85, 1000)];
        assert!(detect_fired(&before, &after, TankId(1), 1.0).is_empty());

        // Reload jumped but the tank is dead in the latest snapshot: no event.
        let reload = VehicleKind::T54_1951.spec().gun.reload_seconds;
        let before = vec![snapshot(1, 0.0, 1000)];
        let after = vec![snapshot(1, reload, 0)];
        assert!(detect_fired(&before, &after, TankId(1), 1.0).is_empty());

        // A tank absent from the previous snapshot (fresh spawn) is not a shot either.
        let after = vec![snapshot(9, reload, 1000)];
        assert!(detect_fired(&[], &after, TankId(1), 1.0).is_empty());
    }

    #[test]
    fn every_firing_tank_reports_not_just_the_player() {
        let reload = VehicleKind::T54_1951.spec().gun.reload_seconds;
        let before = vec![snapshot(1, 0.0, 1000), snapshot(2, 0.0, 900)];
        let after = vec![snapshot(1, reload, 1000), snapshot(2, reload, 900)];

        let events = detect_fired(&before, &after, TankId(1), 1.0);

        assert_eq!(events.len(), 2, "remote shots get muzzle FX too");
    }
}
