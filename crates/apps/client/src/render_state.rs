use std::collections::HashMap;

use engine::TankMotion;
use game_core::TankId;
use game_core::math::{horizontal_forward, lerp_angle, wrap_angle};
use glam::Vec3;
use net::{ShellSnapshot, Snapshot, TankSnapshot};

/// Seconds of one authoritative server tick — `server_tick` deltas convert to time with it.
const SERVER_TICK_SECONDS: f32 = 1.0 / sim::DEFAULT_SERVER_TICK_HZ as f32;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InterpolatedBattleState {
    previous: Option<Snapshot>,
    latest: Option<Snapshot>,
    interpolation_alpha: f32,
    /// Tick-domain motion per tank, derived from the snapshot pair at accept time (constant,
    /// known denominator) — never from presented positions against the render clock.
    motions: HashMap<TankId, TankMotion>,
    /// The roster interpolated at the CURRENT alpha, rebuilt once per phase change (snapshot
    /// accept or alpha set) and shared by every consumer. The scene, battle scars, reticle and
    /// HUD used to each re-interpolate the full roster — up to five rebuilds per frame plus one
    /// per fixed tick, all producing identical data.
    interpolated: Vec<TankSnapshot>,
}

impl InterpolatedBattleState {
    pub fn accept_authoritative_snapshot(&mut self, snapshot: Snapshot) {
        if self.latest.as_ref().is_some_and(|latest| snapshot.server_tick <= latest.server_tick) {
            return;
        }
        // Motion first: it pairs the incoming snapshot with the still-buffered latest.
        self.derive_motions(&snapshot);
        self.previous = self.latest.take();
        self.latest = Some(snapshot);
        self.interpolation_alpha = 0.0;
        self.rebuild_interpolated();
    }

    /// Set the render-time interpolation phase toward the latest snapshot, `0..=1`. The caller
    /// derives it from the fixed-tick phase (ticks since the snapshot plus the sub-tick
    /// remainder), which is the same clock the snapshots are produced on — integrating render
    /// frame deltas instead lets the two clocks drift, freezing at 1.0 and then jumping.
    pub fn set_interpolation_alpha(&mut self, alpha: f32) {
        self.interpolation_alpha = alpha.clamp(0.0, 1.0);
        self.rebuild_interpolated();
    }

    /// Authoritative ticks between the two buffered snapshots (the interpolation window), if
    /// both exist.
    pub fn snapshot_interval_ticks(&self) -> Option<u64> {
        let previous = self.previous.as_ref()?;
        let latest = self.latest.as_ref()?;
        Some((latest.server_tick - previous.server_tick).max(1))
    }

    pub fn previous_snapshot(&self) -> Option<&Snapshot> {
        self.previous.as_ref()
    }

    pub fn latest_snapshot(&self) -> Option<&Snapshot> {
        self.latest.as_ref()
    }

    pub fn interpolation_alpha(&self) -> f32 {
        self.interpolation_alpha
    }

    /// Tick-domain motion of a replicated tank, derived from the snapshot pair. Zero until two
    /// snapshots have seen the tank.
    pub fn motion_of(&self, tank_id: TankId) -> TankMotion {
        self.motions.get(&tank_id).copied().unwrap_or_default()
    }

    /// Tanks interpolated between the previous and latest snapshots (shortest-angle for
    /// yaw/turret). Health and reload come from the latest authoritative snapshot. This is the
    /// memoized roster — free to call from every consumer in the frame.
    pub fn interpolated_tanks(&self) -> &[TankSnapshot] {
        &self.interpolated
    }

    /// One tank interpolated the same way — for the callers that only need the player (the
    /// per-tick sight solve among them).
    pub fn interpolated_tank(&self, tank_id: TankId) -> Option<TankSnapshot> {
        self.interpolated.iter().find(|tank| tank.tank_id == tank_id).cloned()
    }

    /// Rebuild the memoized interpolated roster for the current snapshot pair and alpha. The
    /// buffer is reused across frames, so the steady-state cost is the lerp math alone.
    fn rebuild_interpolated(&mut self) {
        let mut cache = std::mem::take(&mut self.interpolated);
        cache.clear();
        if let Some(latest) = self.latest.as_ref() {
            cache.extend(latest.tanks.iter().map(|tank| self.interpolate(tank)));
        }
        self.interpolated = cache;
    }

    fn interpolate(&self, tank: &TankSnapshot) -> TankSnapshot {
        let alpha = self.interpolation_alpha;
        match self.previous_tank(tank.tank_id) {
            Some(previous) => TankSnapshot {
                tank_id: tank.tank_id,
                team: tank.team,
                vehicle: tank.vehicle,
                position: lerp3(previous.position, tank.position, alpha),
                yaw_rad: lerp_angle(previous.yaw_rad, tank.yaw_rad, alpha),
                hull_pitch_rad: lerp_angle(previous.hull_pitch_rad, tank.hull_pitch_rad, alpha),
                hull_roll_rad: lerp_angle(previous.hull_roll_rad, tank.hull_roll_rad, alpha),
                turret_yaw_rad: lerp_angle(previous.turret_yaw_rad, tank.turret_yaw_rad, alpha),
                turret_yaw_velocity_rad_s: tank.turret_yaw_velocity_rad_s,
                gun_pitch_rad: lerp_angle(previous.gun_pitch_rad, tank.gun_pitch_rad, alpha),
                hit_points: tank.hit_points,
                reload_remaining_s: tank.reload_remaining_s,
                aim_dispersion_mrad: tank.aim_dispersion_mrad,
                module_hit_points: tank.module_hit_points,
                destroyed_modules_mask: tank.destroyed_modules_mask,
                track_damage_mask: tank.track_damage_mask,
                track_hp: [game_core::TRACK_HP_MAX; 2],
                ammo_counts: tank.ammo_counts,
                selected_ammo: tank.selected_ammo,
                spotted_by_teams_mask: tank.spotted_by_teams_mask,
            },
            None => tank.clone(),
        }
    }

    /// Shells extrapolated from the latest snapshot with the authoritative flight integrator, so
    /// fast tracers move smoothly between 20 Hz snapshots without visually shedding gravity or
    /// drag. Substeps never exceed the sim tick: presentation follows the same integration order.
    pub fn interpolated_shells(&self, snapshot_interval_seconds: f32) -> Vec<ShellSnapshot> {
        let Some(latest) = self.latest.as_ref() else {
            return Vec::new();
        };
        let dt = self.interpolation_alpha * snapshot_interval_seconds;
        latest
            .shells
            .iter()
            .map(|shell| {
                let mut position = Vec3::from_array(shell.position);
                let mut velocity = Vec3::from_array(shell.velocity_mps);
                let max_step = 1.0 / sim::DEFAULT_SIMULATION_TICK_HZ as f32;
                let steps = (dt / max_step).ceil().max(1.0) as usize;
                let step = dt / steps as f32;
                for _ in 0..steps {
                    game_core::math::integrate_shell_step(&mut velocity, shell.drag_per_s, step);
                    position += velocity * step;
                }
                ShellSnapshot {
                    position: position.to_array(),
                    velocity_mps: velocity.to_array(),
                    age_seconds: shell.age_seconds + dt,
                    ..*shell
                }
            })
            .collect()
    }

    fn previous_tank(&self, tank_id: TankId) -> Option<&TankSnapshot> {
        self.previous.as_ref()?.tanks.iter().find(|tank| tank.tank_id == tank_id)
    }

    /// Rebuild the per-tank motion table from the pair (outgoing latest, incoming snapshot).
    /// The interval is exact (authoritative tick counts), so the derived speed and yaw rate are
    /// tick-domain quantities; the acceleration additionally needs the previous pair's speed,
    /// carried over from the outgoing table.
    fn derive_motions(&mut self, incoming: &Snapshot) {
        let Some(outgoing) = self.latest.as_ref() else {
            self.motions.clear();
            return;
        };
        let interval_ticks = (incoming.server_tick - outgoing.server_tick).max(1);
        let interval_s = interval_ticks as f32 * SERVER_TICK_SECONDS;
        let mut motions = HashMap::with_capacity(incoming.tanks.len());
        for tank in &incoming.tanks {
            let Some(previous) =
                outgoing.tanks.iter().find(|candidate| candidate.tank_id == tank.tank_id)
            else {
                continue;
            };
            let velocity = (Vec3::from_array(tank.position) - Vec3::from_array(previous.position))
                / interval_s;
            let forward_speed = velocity.dot(horizontal_forward(tank.yaw_rad));
            let previous_speed = self.motions.get(&tank.tank_id).map(|m| m.forward_speed_mps);
            motions.insert(
                tank.tank_id,
                TankMotion {
                    forward_speed_mps: forward_speed,
                    accel_long_mps2: previous_speed
                        .map_or(0.0, |prev| (forward_speed - prev) / interval_s),
                    yaw_rate_rad_s: wrap_angle(tank.yaw_rad - previous.yaw_rad) / interval_s,
                },
            );
        }
        self.motions = motions;
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    Vec3::from_array(a).lerp(Vec3::from_array(b), t).to_array()
}
