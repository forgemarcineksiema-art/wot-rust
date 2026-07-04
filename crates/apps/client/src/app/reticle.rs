use game_core::MountFrames;
use glam::Vec3;
use renderer_api::Camera;

use super::ClientApp;
use crate::hud::HudReticle;

const GUN_TRACK_GAIN: f32 = 6.0;

/// Gun commands derived from one resolved sight point: the ballistic elevation and the
/// muzzle->sight turret bearing share a single (expensive) aim sweep per fixed-tick batch.
/// Both targets are HULL-relative — the ballistic arc is solved in world space, then converted
/// through the hull pose (`world_direction_to_turret`), so the commands respect the hull-frame
/// gun limits and hull-down genuinely adds depression.
#[derive(Debug, Clone, Copy)]
pub(super) struct SightSolution {
    /// Hull-relative gun pitch target.
    pub pitch_rad: f32,
    /// Hull-relative turret yaw that points the gun from the muzzle *through* the sight point,
    /// so the shell converges on the crosshair. Commanding the raw camera yaw instead runs the
    /// barrel parallel to the over-shoulder sight lane and misses sideways, worst at close
    /// range. `None` only when the sight point sits on the muzzle.
    pub turret_yaw_rad: Option<f32>,
}

impl ClientApp {
    pub(super) fn sight_solution(&self) -> Option<SightSolution> {
        let tank = self.local_render_tank()?;
        let hull = tank.hull_pose();
        let camera = self.camera_from_tank(tank);
        let aim = self.aim_world_point(&camera)?;
        let muzzle = self.muzzle_position();
        let world_pitch = crate::aim::gun_pitch_to_hit(
            muzzle,
            aim,
            self.player_spec().gun.shell.muzzle_velocity_mps,
        );
        let delta = aim - muzzle;
        if delta.x.abs() <= 1.0e-4 && delta.z.abs() <= 1.0e-4 {
            let pitch_rad = world_pitch.clamp(sim::MIN_GUN_PITCH_RAD, sim::MAX_GUN_PITCH_RAD);
            return Some(SightSolution { pitch_rad, turret_yaw_rad: None });
        }
        // World arc (azimuth muzzle->aim, solved ballistic elevation) -> hull-relative targets.
        let world_direction = game_core::math::gun_direction(delta.x.atan2(delta.z), world_pitch);
        let (turret_yaw, gun_pitch) =
            game_core::math::world_direction_to_turret(hull, world_direction);
        Some(SightSolution {
            pitch_rad: gun_pitch.clamp(sim::MIN_GUN_PITCH_RAD, sim::MAX_GUN_PITCH_RAD),
            turret_yaw_rad: Some(turret_yaw),
        })
    }

    /// Elevation-rate command (in [-1, 1]) that traverses the gun toward the pitch needed to
    /// land a shell on the desired sight point.
    pub(super) fn gun_elevation_command_for(&self, solution: Option<&SightSolution>) -> f32 {
        let target_pitch =
            solution.map_or(self.desired_aim.pitch_rad(), |solution| solution.pitch_rad);
        ((target_pitch - self.player_gun_pitch()) * GUN_TRACK_GAIN).clamp(-1.0, 1.0)
    }

    #[cfg(test)]
    pub(super) fn gun_elevation_command(&self) -> f32 {
        self.gun_elevation_command_for(self.sight_solution().as_ref())
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
        let report =
            crate::hud::reticle::reticle_report(crate::hud::reticle::ReticleFeedbackQuery {
                heightmap: &self.battlefield.heightmap,
                cover: &self.battlefield.static_cover,
                tanks: &tanks,
                player_spec: &player_spec,
                owner: self.player_tank,
                owner_team: self.player_team(),
                muzzle,
                aim,
                gun_direction: game_core::math::gun_direction_world(
                    tank.hull_pose(),
                    tank.turret_yaw_rad,
                    tank.gun_pitch_rad,
                ),
                muzzle_velocity_mps: muzzle_velocity,
            });
        let feedback = report.feedback;
        let pen_hint = report.penetration;

        let (reload_remaining, reload_max) = self.player_reload();
        Some(HudReticle {
            aim_clip: crate::hud::reticle::world_to_clip_xy(
                feedback.aim_world_point,
                view_projection,
            )
            .unwrap_or([0.0, 0.0]),
            impact_clip: crate::hud::reticle::world_to_clip_xy(
                feedback.actual_impact_world_point,
                view_projection,
            ),
            aim_radius_clip: self.player_aim_radius_clip(camera.vertical_fov_degrees),
            target_distance_m: Some((feedback.aim_world_point - muzzle).length()),
            status: feedback.status,
            penetration_hint: pen_hint,
            reload_fraction: 1.0 - (reload_remaining / reload_max.max(0.001)).clamp(0.0, 1.0),
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

    /// World-space muzzle, pivoted about the trunnion and ring exactly like the rendered gun and
    /// the server's shell spawn — `muzzle_position_matches_server_shell_origin` locks the three
    /// together.
    fn muzzle_position(&self) -> Vec3 {
        let barrel_scale = self.player_barrel_scale();
        let Some(tank) = self.local_render_tank() else {
            let mounts = MountFrames::for_vehicle(self.player_spec().kind);
            return game_core::math::muzzle_world_position_scaled(
                &mounts,
                self.predictor.position(),
                self.predictor.hull_pose(),
                self.predictor.turret_yaw(),
                self.predictor.gun_pitch(),
                barrel_scale,
            );
        };
        let mounts = MountFrames::for_vehicle(tank.vehicle);
        game_core::math::muzzle_world_position_scaled(
            &mounts,
            Vec3::from_array(tank.position),
            tank.hull_pose(),
            tank.turret_yaw_rad,
            tank.gun_pitch_rad,
            barrel_scale,
        )
    }

    fn player_gun_pitch(&self) -> f32 {
        self.predictor.gun_pitch()
    }

    fn player_aim_radius_clip(&self, vertical_fov_degrees: f32) -> f32 {
        // Once prediction is seeded, the aim circle follows the locally predicted dispersion
        // (evolved at 60 Hz in lockstep with the server) instead of the stale 20 Hz snapshot value.
        let dispersion_mrad = if self.predictor.is_seeded() {
            self.predictor.aim_dispersion_mrad()
        } else {
            self.player_spec().gun.dispersion_mrad
        };
        // Project the angular dispersion through the actual view: clip-y = tan(theta)/tan(vfov/2)
        // (small-angle theta), so the circle magnifies with sniper zoom instead of sitting at a
        // fixed screen size that matches no FOV.
        let half_fov_tan = (vertical_fov_degrees.to_radians() * 0.5).tan().max(1.0e-4);
        dispersion_mrad * 0.001 / half_fov_tan
    }
}

#[cfg(test)]
mod tests {
    use game_core::MountFrames;
    use glam::Vec3;

    use super::*;
    use crate::camera::{BattleCameraMode, CameraSubject};

    /// The client's muzzle, the server's shell origin, and the rendered barrel all share one
    /// pivot chain: pitch about the trunnion, traverse about the ring, hull yaw about the origin.
    /// The expected value below derives that chain with explicit trigonometry, independent of
    /// `muzzle_world_position`, so a regression in the shared helper turns this red too.
    #[test]
    fn muzzle_position_matches_server_shell_origin() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.camera_controller.set_mode(BattleCameraMode::Sniper);
        app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.10);
        app.run_fixed_ticks(40);

        let tank = app.local_render_tank().expect("local tank");
        assert!(tank.gun_pitch_rad.abs() > 1.0e-3, "pose must exercise a pitched gun");
        let mounts = MountFrames::for_vehicle(tank.vehicle);
        let ring = mounts.turret_ring.translation;
        let trunnion = mounts.gun_trunnion.translation;
        let barrel = mounts.muzzle.translation.z - trunnion.z;

        // Pitch about the trunnion (the gun axis is level: muzzle.y == trunnion.y).
        let pitched = Vec3::new(
            0.0,
            trunnion.y + barrel * tank.gun_pitch_rad.sin(),
            trunnion.z + barrel * tank.gun_pitch_rad.cos(),
        );
        // Traverse about the ring, then the full hull pose about the tank position. The battle
        // terrain is not flat, so the hull carries a real authoritative attitude here — the
        // muzzle must ride it exactly like the server's shell spawn does.
        let traverse = |point: Vec3, pivot: Vec3, yaw: f32| {
            let rel = point - pivot;
            pivot
                + Vec3::new(
                    rel.x * yaw.cos() + rel.z * yaw.sin(),
                    rel.y,
                    rel.z * yaw.cos() - rel.x * yaw.sin(),
                )
        };
        let traversed = traverse(pitched, ring, tank.turret_yaw_rad);
        let basis =
            game_core::math::hull_basis(tank.yaw_rad, tank.hull_pitch_rad, tank.hull_roll_rad);
        let expected = Vec3::from_array(tank.position) + basis * traversed;

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
    fn aim_circle_magnifies_with_sniper_zoom_instead_of_fixed_screen_size() {
        let app = ClientApp::new();

        let wide = app.player_aim_radius_clip(18.0);
        let narrow = app.player_aim_radius_clip(3.0);

        // Same angular dispersion through a narrower FOV covers proportionally more clip space:
        // tan(9 deg)/tan(1.5 deg) ~ 6.05.
        let expected_ratio = (9.0f32.to_radians()).tan() / (1.5f32.to_radians()).tan();
        assert!((narrow / wide - expected_ratio).abs() < 0.05, "got ratio {}", narrow / wide);

        // And the absolute value is the projected angle, not a magic screen constant.
        let dispersion_mrad = app.player_spec().gun.dispersion_mrad;
        let expected = dispersion_mrad * 0.001 / (9.0f32.to_radians()).tan();
        assert!((wide - expected).abs() < 1.0e-6);
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
