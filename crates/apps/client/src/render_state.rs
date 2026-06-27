use game_core::math::lerp_angle;
use glam::Vec3;
use net::{ShellSnapshot, Snapshot, TankSnapshot};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InterpolatedBattleState {
    previous: Option<Snapshot>,
    latest: Option<Snapshot>,
    interpolation_alpha: f32,
}

impl InterpolatedBattleState {
    pub fn accept_authoritative_snapshot(&mut self, snapshot: Snapshot) {
        if self.latest.as_ref().is_some_and(|latest| snapshot.server_tick <= latest.server_tick) {
            return;
        }
        self.previous = self.latest.take();
        self.latest = Some(snapshot);
        self.interpolation_alpha = 0.0;
    }

    /// Advance render-time interpolation toward the latest snapshot. Alpha saturates at
    /// 1.0 so we interpolate between the two latest snapshots instead of extrapolating.
    pub fn advance(&mut self, seconds: f32, snapshot_interval_seconds: f32) {
        let step = seconds / snapshot_interval_seconds.max(1.0e-4);
        self.interpolation_alpha = (self.interpolation_alpha + step).clamp(0.0, 1.0);
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

    /// Tanks interpolated between the previous and latest snapshots (shortest-angle for
    /// yaw/turret). Health and reload come from the latest authoritative snapshot.
    pub fn interpolated_tanks(&self) -> Vec<TankSnapshot> {
        let Some(latest) = self.latest.as_ref() else {
            return Vec::new();
        };
        let alpha = self.interpolation_alpha;
        latest
            .tanks
            .iter()
            .map(|tank| match self.previous_tank(tank.tank_id) {
                Some(previous) => TankSnapshot {
                    tank_id: tank.tank_id,
                    team: tank.team,
                    vehicle: tank.vehicle,
                    position: lerp3(previous.position, tank.position, alpha),
                    yaw_rad: lerp_angle(previous.yaw_rad, tank.yaw_rad, alpha),
                    turret_yaw_rad: lerp_angle(previous.turret_yaw_rad, tank.turret_yaw_rad, alpha),
                    turret_yaw_velocity_rad_s: tank.turret_yaw_velocity_rad_s,
                    gun_pitch_rad: lerp_angle(previous.gun_pitch_rad, tank.gun_pitch_rad, alpha),
                    hit_points: tank.hit_points,
                    reload_remaining_s: tank.reload_remaining_s,
                    aim_dispersion_mrad: tank.aim_dispersion_mrad,
                    module_hit_points: tank.module_hit_points,
                    destroyed_modules_mask: tank.destroyed_modules_mask,
                },
                None => tank.clone(),
            })
            .collect()
    }

    /// Shells extrapolated from the latest snapshot by their known velocity, so fast
    /// tracers move smoothly between 20 Hz snapshots.
    pub fn interpolated_shells(&self, snapshot_interval_seconds: f32) -> Vec<ShellSnapshot> {
        let Some(latest) = self.latest.as_ref() else {
            return Vec::new();
        };
        let dt = self.interpolation_alpha * snapshot_interval_seconds;
        latest
            .shells
            .iter()
            .map(|shell| {
                let advanced =
                    Vec3::from_array(shell.position) + Vec3::from_array(shell.velocity_mps) * dt;
                ShellSnapshot { position: advanced.to_array(), ..*shell }
            })
            .collect()
    }

    fn previous_tank(&self, tank_id: game_core::TankId) -> Option<&TankSnapshot> {
        self.previous.as_ref()?.tanks.iter().find(|tank| tank.tank_id == tank_id)
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    Vec3::from_array(a).lerp(Vec3::from_array(b), t).to_array()
}
