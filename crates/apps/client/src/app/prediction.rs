use glam::Vec3;
use net::TankSnapshot;
use physics::TankObstacle;
use sim::DEFAULT_SERVER_TICK_HZ;

use super::ClientApp;

const TICK_DT: f32 = 1.0 / DEFAULT_SERVER_TICK_HZ as f32;
const TURRET_TRACK_GAIN: f32 = 5.0;

impl ClientApp {
    pub(super) fn seed_prediction(&mut self) {
        if self.predictor.is_seeded() {
            return;
        }
        if let Some(tank) = self.player_snapshot().cloned() {
            self.predictor.sync_to(&tank);
            self.camera_controller.set_orbit_yaw(tank.yaw_rad);
            self.desired_aim = crate::aim::DesiredAim::new(tank.yaw_rad, tank.gun_pitch_rad);
        }
    }

    pub(super) fn step_prediction(&mut self, command: &sim::TankCommand) {
        let tank_obstacles = self.tank_obstacles_for_prediction();
        self.predictor.step(
            *command,
            &self.battlefield.heightmap,
            &self.battlefield.static_cover,
            &tank_obstacles,
            TICK_DT,
        );
    }

    fn tank_obstacles_for_prediction(&self) -> Vec<TankObstacle> {
        self.local_server
            .current_snapshot()
            .tanks
            .iter()
            .filter(|tank| tank.tank_id != self.player_tank)
            .map(|tank| {
                TankObstacle::from_hitbox(
                    Vec3::from_array(tank.position),
                    tank.yaw_rad,
                    game_core::HitboxProfile::for_vehicle(tank.vehicle),
                )
            })
            .collect()
    }

    /// Tick-accurate local tank pose for gameplay (aim/reticle/gun elevation).
    pub(super) fn local_render_tank(&self) -> Option<TankSnapshot> {
        self.local_tank_with_pose(self.predictor.interpolated_pose(1.0))
    }

    /// Local tank blended `alpha` into the current tick, for the rendered mesh and camera.
    pub(super) fn interpolated_local_tank(&self, alpha: f32) -> Option<TankSnapshot> {
        self.local_tank_with_pose(self.predictor.interpolated_pose(alpha))
    }

    fn local_tank_with_pose(&self, pose: crate::predict::PredictedPose) -> Option<TankSnapshot> {
        let interpolated = self.render_state.interpolated_tanks();
        let local = interpolated.iter().find(|tank| tank.tank_id == self.player_tank)?;
        Some(TankSnapshot {
            position: pose.position.to_array(),
            yaw_rad: pose.yaw_rad,
            hull_pitch_rad: pose.hull_pitch_rad,
            hull_roll_rad: pose.hull_roll_rad,
            turret_yaw_rad: pose.turret_yaw_rad,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: pose.gun_pitch_rad,
            ..local.clone()
        })
    }

    pub(super) fn player_reload(&self) -> (f32, f32) {
        let remaining = self.player_snapshot().map_or(0.0, |tank| tank.reload_remaining_s);
        (remaining, self.player_spec().gun.reload_seconds)
    }

    pub(super) fn player_hud_hit_points(&self) -> u32 {
        self.player_snapshot().map_or(0, |tank| tank.hit_points)
    }

    /// The rack panel model: counts from the latest authoritative snapshot, the selected slot
    /// from the predictor (optimistic on a 1/2/3 press).
    pub(super) fn player_ammo_hud(&self) -> crate::hud::ammo_panel::AmmoHudModel {
        let counts = self
            .player_snapshot()
            .map_or(self.predictor.spec().ammo.counts, |tank| tank.ammo_counts);
        crate::hud::ammo_panel::AmmoHudModel::new(counts, self.predictor.selected_ammo())
    }

    pub(super) fn player_max_hit_points(&self) -> u32 {
        self.player_spec().hit_points
    }

    pub(super) fn player_speed_kmh(&self) -> f32 {
        self.predictor.speed_mps() * 3.6
    }

    pub(super) fn player_snapshot(&self) -> Option<&TankSnapshot> {
        self.render_state
            .latest_snapshot()?
            .tanks
            .iter()
            .find(|tank| tank.tank_id == self.player_tank)
    }

    /// The player's replicated team — the reticle splits targets from blockers with it, exactly
    /// like the server. Defaults to team 1 before the first snapshot lands.
    pub(super) fn player_team(&self) -> game_core::TeamId {
        self.player_snapshot().map_or(game_core::TeamId(1), |tank| tank.team)
    }

    pub(super) fn player_spec(&self) -> game_core::TankSpec {
        self.player_snapshot()
            .map_or_else(|| self.predictor.spec().clone(), |tank| tank.vehicle.spec())
    }

    #[cfg(test)]
    pub(super) fn predictor_spec(&self) -> &game_core::TankSpec {
        self.predictor.spec()
    }

    /// Command the turret to traverse so the gun converges on the resolved sight point under the
    /// crosshair, not merely parallel to the camera. Falls back to the raw camera yaw when no aim
    /// point is available (e.g. before the local tank exists). Both the bearing and the turret are
    /// measured relative to the hull.
    pub(super) fn turret_tracking_command_for(
        &self,
        solution: Option<&super::reticle::SightSolution>,
    ) -> f32 {
        // The sight solution already carries the HULL-relative turret target (converted through
        // the hull pose); only the raw-camera fallback still subtracts the planar hull yaw.
        let target = solution
            .and_then(|solution| solution.turret_yaw_rad)
            .unwrap_or_else(|| shortest_angle(self.desired_aim.yaw_rad() - self.predictor.yaw()));
        let current = self.predictor.turret_yaw();
        (shortest_angle(target - current) * TURRET_TRACK_GAIN).clamp(-1.0, 1.0)
    }

    #[cfg(test)]
    pub(super) fn turret_tracking_command(&self) -> f32 {
        self.turret_tracking_command_for(self.sight_solution().as_ref())
    }

    /// World-space bearing of the sight solution (hull-relative target folded back through the
    /// hull yaw) — test-only readback.
    #[cfg(test)]
    pub(super) fn desired_turret_yaw(&self) -> Option<f32> {
        self.sight_solution()
            .and_then(|solution| solution.turret_yaw_rad)
            .map(|turret_yaw| turret_yaw + self.predictor.yaw())
    }
}

fn shortest_angle(radians: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let wrapped = radians.rem_euclid(TAU);
    if wrapped > PI { wrapped - TAU } else { wrapped }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_prediction_uses_the_full_local_server_roster_for_tank_obstacles() {
        let app = ClientApp::new();

        let obstacles = app.tank_obstacles_for_prediction();
        let full_roster = app.local_server.current_snapshot().tanks.len();

        assert_eq!(
            obstacles.len(),
            full_roster.saturating_sub(1),
            "prediction collision must see every local-server tank except the player, not just the filtered viewer snapshot"
        );
    }

    #[test]
    fn turret_converges_gun_onto_the_sight_point_not_parallel_to_camera() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.seed_prediction();

        // Let the turret-tracking loop settle against the sight lane.
        for _ in 0..300 {
            app.run_fixed_ticks(1);
        }

        // Settled: the gun holds on the sight point, so no residual traverse is commanded.
        let command = app.turret_tracking_command();
        assert!(
            command.abs() < 1.0e-2,
            "settled turret should hold the sight point, got {command}"
        );

        // The convergence target is the muzzle->sight bearing (with the camera centered the
        // bearing sits near the camera yaw, but the mechanism converges on the SIGHT POINT —
        // the target-forward offset still separates the two paths).
        let bearing = app.desired_turret_yaw().expect("sight point resolves to a bearing");

        // The settled turret actually points along that bearing (world space).
        let settled_world_yaw = app.predictor.yaw() + app.predictor.turret_yaw();
        assert!(
            shortest_angle(settled_world_yaw - bearing).abs() < 2.0e-2,
            "settled turret heading {settled_world_yaw} should match the sight bearing {bearing}"
        );
    }
}
