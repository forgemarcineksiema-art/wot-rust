use game_core::MountFrames;
use glam::Vec3;
use renderer_api::Camera;

use super::ClientApp;
use crate::hud::HudReticle;

const GUN_TRACK_GAIN: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct DesiredGunSolution {
    pub pitch_rad: f32,
}

impl ClientApp {
    /// Elevation-rate command (in [-1, 1]) that traverses the gun toward the pitch needed to
    /// land a shell on the desired sight point.
    pub(super) fn gun_elevation_command(&self) -> f32 {
        let target_pitch =
            self.desired_gun_solution().map_or(self.desired_aim.pitch_rad(), |aim| aim.pitch_rad);
        ((target_pitch - self.player_gun_pitch()) * GUN_TRACK_GAIN).clamp(-1.0, 1.0)
    }

    pub(super) fn desired_gun_solution(&self) -> Option<DesiredGunSolution> {
        let tank = self.local_render_tank()?;
        let camera = self.camera_from_tank(tank);
        let aim = self.aim_world_point(&camera)?;
        let muzzle = self.muzzle_position();
        let pitch_rad = crate::aim::gun_pitch_to_hit(
            muzzle,
            aim,
            self.player_spec().gun.shell.muzzle_velocity_mps,
        )
        .clamp(sim::MIN_GUN_PITCH_RAD, sim::MAX_GUN_PITCH_RAD);
        Some(DesiredGunSolution { pitch_rad })
    }

    /// World-space turret heading that points the gun from the muzzle *through* the resolved sight
    /// point, so the shell converges on the crosshair. The yaw counterpart of `desired_gun_solution`:
    /// commanding the turret to the raw camera yaw instead runs the barrel parallel to the sight
    /// lane, which — with the over-shoulder camera offset — misses sideways, worst at close range.
    pub(super) fn desired_turret_yaw(&self) -> Option<f32> {
        let tank = self.local_render_tank()?;
        let camera = self.camera_from_tank(tank);
        let aim = self.aim_world_point(&camera)?;
        let muzzle = self.muzzle_position();
        let delta = aim - muzzle;
        // Degenerate only if the sight point sits on the muzzle; otherwise the horizontal bearing
        // is well defined. `gun_direction` uses x = sin(yaw), z = cos(yaw), so yaw = atan2(x, z).
        ((delta.x.abs() > 1.0e-4) || (delta.z.abs() > 1.0e-4)).then(|| delta.x.atan2(delta.z))
    }

    pub(super) fn hud_reticle(
        &self,
        camera: &Camera,
        view_projection: [[f32; 4]; 4],
    ) -> Option<HudReticle> {
        let aim = self.aim_world_point(camera)?;
        let tank = self.local_render_tank()?;
        let tanks = self.render_state.interpolated_tanks();
        let player_spec = self.player_spec();
        let muzzle_velocity = player_spec.gun.shell.muzzle_velocity_mps;
        let muzzle = self.muzzle_position();
        let feedback = crate::reticle::reticle_feedback(crate::reticle::ReticleFeedbackQuery {
            heightmap: &self.battlefield.heightmap,
            cover: &self.battlefield.static_cover,
            tanks: &tanks,
            player_spec: &player_spec,
            owner: self.player_tank,
            owner_team: self.player_team(),
            muzzle,
            aim,
            turret_yaw_rad: tank.yaw_rad + tank.turret_yaw_rad,
            gun_pitch_rad: tank.gun_pitch_rad,
            muzzle_velocity_mps: muzzle_velocity,
        });
        let pen_hint = crate::reticle::penetration_hint(crate::reticle::ReticleFeedbackQuery {
            heightmap: &self.battlefield.heightmap,
            cover: &self.battlefield.static_cover,
            tanks: &tanks,
            player_spec: &player_spec,
            owner: self.player_tank,
            owner_team: self.player_team(),
            muzzle,
            aim,
            turret_yaw_rad: tank.yaw_rad + tank.turret_yaw_rad,
            gun_pitch_rad: tank.gun_pitch_rad,
            muzzle_velocity_mps: muzzle_velocity,
        });

        Some(HudReticle {
            aim_clip: crate::reticle::world_to_clip_xy(feedback.aim_world_point, view_projection)
                .unwrap_or([0.0, 0.0]),
            gun_clip: crate::reticle::world_to_clip_xy(feedback.gun_world_point, view_projection),
            impact_clip: crate::reticle::world_to_clip_xy(
                feedback.actual_impact_world_point,
                view_projection,
            ),
            aim_radius_clip: self.player_aim_radius_clip(),
            target_distance_m: Some((feedback.aim_world_point - muzzle).length()),
            status: feedback.status,
            penetration_hint: pen_hint,
        })
    }

    fn aim_world_point(&self, camera: &Camera) -> Option<Vec3> {
        let eye = Vec3::from_array(camera.eye);
        let forward = (Vec3::from_array(camera.target) - eye).normalize_or_zero();
        (forward != Vec3::ZERO).then(|| {
            crate::aim::aim_point_with_sweep(
                &self.battlefield.heightmap,
                &self.battlefield.static_cover,
                &self.render_state.interpolated_tanks(),
                self.player_tank,
                self.player_team(),
                eye,
                forward,
            )
        })
    }

    fn muzzle_position(&self) -> Vec3 {
        let Some(tank) = self.local_render_tank() else {
            let muzzle = MountFrames::for_vehicle(self.player_spec().kind).muzzle.translation;
            return self.predictor.position() + Vec3::Y * muzzle.y;
        };
        let muzzle = MountFrames::for_vehicle(tank.vehicle).muzzle.translation;
        let yaw = tank.yaw_rad + tank.turret_yaw_rad;
        let direction = game_core::math::gun_direction(yaw, tank.gun_pitch_rad);
        Vec3::from_array(tank.position) + Vec3::Y * muzzle.y + direction * muzzle.z
    }

    fn player_gun_pitch(&self) -> f32 {
        self.predictor.gun_pitch()
    }

    fn player_aim_radius_clip(&self) -> f32 {
        // Once prediction is seeded, the aim circle follows the locally predicted dispersion
        // (evolved at 60 Hz in lockstep with the server) instead of the stale 20 Hz snapshot value.
        let dispersion_mrad = if self.predictor.is_seeded() {
            self.predictor.aim_dispersion_mrad()
        } else {
            self.player_spec().gun.dispersion_mrad
        };
        dispersion_mrad * 0.001 * 18.0
    }
}

#[cfg(test)]
mod tests {
    use game_core::MountFrames;
    use glam::Vec3;

    use super::*;
    use crate::camera::{BattleCameraMode, CameraSubject};

    #[test]
    fn muzzle_position_matches_server_shell_origin() {
        let mut app = ClientApp::new();
        app.camera_controller.set_mode(BattleCameraMode::Sniper);
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.10);
        app.run_fixed_ticks(4);

        let tank = app.local_render_tank().expect("local tank");
        let yaw = tank.yaw_rad + tank.turret_yaw_rad;
        let forward = Vec3::new(
            yaw.sin() * tank.gun_pitch_rad.cos(),
            tank.gun_pitch_rad.sin(),
            yaw.cos() * tank.gun_pitch_rad.cos(),
        )
        .normalize();
        let mount = MountFrames::for_vehicle(tank.vehicle).muzzle.translation;
        let expected = Vec3::from_array(tank.position) + Vec3::Y * mount.y + forward * mount.z;

        assert!((app.muzzle_position() - expected).length() < 1.0e-4);
    }

    #[test]
    fn sniper_camera_frame_uses_desired_pitch() {
        let mut app = ClientApp::new();
        app.camera_controller.set_mode(BattleCameraMode::Sniper);
        app.seed_prediction();
        app.desired_aim = crate::aim::DesiredAim::new(0.0, -0.10);
        let tank = app.local_render_tank().expect("local tank");
        let camera = app.camera_from_tank(tank.clone());

        let subject = CameraSubject::from_snapshot(tank.clone(), tank.gun_pitch_rad)
            .with_desired_aim(0.0, -0.10);
        let expected = app.camera_controller.render_camera(
            &subject,
            &crate::BattleCameraEnvironment::with_obstacles(
                &app.battlefield.heightmap,
                &app.camera_obstacles,
            ),
        );

        assert_eq!(camera.target, expected.target);
    }

    #[test]
    fn gun_elevation_solves_ballistic_pitch_instead_of_raw_sight_pitch() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.camera_controller.set_mode(BattleCameraMode::Sniper);
        app.seed_prediction();
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);

        let command = app.gun_elevation_command();

        assert!(command > 0.0, "level sight ray should still elevate for shell drop");
    }
}
