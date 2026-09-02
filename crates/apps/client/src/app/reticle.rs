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
        // The ballistic solution flies the round the player actually has loaded (optimistic on
        // an ammo switch, reconciled from snapshots) — not the spec's stock shell.
        let shell = self.predictor.selected_shell();
        // The sight may not promise an elevation this gun cannot reach.
        let limits = self.player_spec().gun_pitch_limits_rad();
        // The SAME solution the reticle previews (`crate::aim::firing_solution`): the gun chases
        // exactly the shot the sight promises, because both read one function.
        if let Some(solution) = crate::aim::firing_solution(
            muzzle,
            aim,
            hull,
            limits,
            shell.muzzle_velocity_mps,
            shell.drag_per_s(),
            self.player_spec().has_fixed_casemate(),
        ) {
            return Some(SightSolution {
                pitch_rad: solution.gun_pitch_rad,
                turret_yaw_rad: Some(solution.turret_yaw_rad),
            });
        }
        // Sight point on the muzzle: no bearing to solve, so hold the traverse and keep the
        // elevation the world arc asks for.
        let world_pitch = crate::aim::gun_pitch_to_hit(
            muzzle,
            aim,
            shell.muzzle_velocity_mps,
            shell.drag_per_s(),
        );
        Some(SightSolution {
            pitch_rad: world_pitch.clamp(limits.0, limits.1),
            turret_yaw_rad: None,
        })
    }

    /// Elevation-rate command (in [-1, 1]) that traverses the gun toward the pitch needed to
    /// land a shell on the desired sight point. With no sight solution (no local tank yet) the gun
    /// holds — `desired_aim.pitch` is a *world* sight elevation now, not a hull-relative gun target,
    /// so it cannot be commanded raw.
    pub(super) fn gun_elevation_command_for(&self, solution: Option<&SightSolution>) -> f32 {
        let Some(solution) = solution else { return 0.0 };
        ((solution.pitch_rad - self.player_gun_pitch()) * GUN_TRACK_GAIN).clamp(-1.0, 1.0)
    }

    #[cfg(test)]
    pub(super) fn gun_elevation_command(&self) -> f32 {
        self.gun_elevation_command_for(self.sight_solution().as_ref())
    }

    /// `alpha` is the render sub-tick blend of the PRESENTED camera. The reticle's world points
    /// (muzzle, gun ray) must come from the SAME blended pose: projected through the camera, a
    /// full-tick pose is up to one tick of hull motion ahead of the view, and at sniper FOV that
    /// mismatch jitters the gun marker and impact X across a third of the screen every frame.
    pub(super) fn hud_reticle(
        &self,
        camera: &Camera,
        view_projection: [[f32; 4]; 4],
        alpha: f32,
    ) -> Option<HudReticle> {
        let sight = self.sight_point(camera)?;
        let aim = sight.point;
        let tank = self.interpolated_local_tank(alpha)?;
        let tanks = self.render_state.interpolated_tanks();
        // The reticle trace and pen hint fly the SELECTED shell (the predictor holds the custom
        // loadout and the optimistic slot choice); the snapshot-derived spec is stock by kind.
        // This is the ONE caller that mutates its copy — it flies the selected shell, not the
        // stock one — so it clones the borrowed spec deliberately, where the others just read.
        let mut player_spec = self.player_spec().clone();
        player_spec.gun.shell = self.predictor.selected_shell();
        let muzzle_velocity = player_spec.gun.shell.muzzle_velocity_mps;
        let muzzle = self.muzzle_position_of(&tank);
        let report =
            crate::hud::reticle::reticle_report(crate::hud::reticle::ReticleFeedbackQuery {
                heightmap: &self.battlefield.heightmap,
                cover: self.live_cover.blocking(),
                water: self.battlefield.water_view(),
                gun_pitch_limits_rad: player_spec.gun_pitch_limits_rad(),
                hull_pose: tank.hull_pose(),
                tanks,
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
                drag_per_s: player_spec.gun.shell.drag_per_s(),
            });
        let feedback = report.feedback;
        let pen_hint = report.penetration;

        let (reload_remaining, reload_max) = self.player_reload();
        let mode = if self.camera_controller.mode() == crate::BattleCameraMode::Sniper {
            crate::hud::reticle::ReticleMode::Sniper
        } else {
            crate::hud::reticle::ReticleMode::ThirdPerson
        };
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
            gun_clip: crate::hud::reticle::world_to_clip_xy(
                feedback.gun_world_point,
                view_projection,
            ),
            aim_radius_clip: self.player_aim_radius_clip(camera.vertical_fov_degrees),
            // Open sky has no range: the only number available there is how far the sight ray
            // itself reaches, which is not a fact about the battlefield.
            target_distance_m: sight
                .on_surface
                .then(|| (feedback.aim_world_point - muzzle).length()),
            // A refusal names its cause: metres to whatever eats the round. The range above it
            // answers "how far is what I am pointing at", which while blocked is a distance to
            // something this gun cannot reach.
            block_distance_m: feedback.block_distance_m,
            arc_limit: feedback.arc_limit,
            status: feedback.status,
            penetration_hint: pen_hint,
            reload_fraction: 1.0 - (reload_remaining / reload_max.max(0.001)).clamp(0.0, 1.0),
            hit_confirm: self.hit_indicator.recent_confirm(),
            converged: self.player_aim_converged(),
            mode,
            // The TARGET colour; `render_now` eases the drawn one toward it with the frame clock.
            // Scaled by the scope dressing so the verdict arrives with the optics rather than
            // snapping while the camera is still travelling into them.
            marker_color: crate::hud::reticle_overlay::marker_color(
                mode,
                pen_hint,
                self.camera_controller.scope_dressing(),
            ),
        })
    }

    pub(super) fn aim_world_point(&self, camera: &Camera) -> Option<Vec3> {
        self.sight_point(camera).map(|sight| sight.point)
    }

    /// The sight ray's end AND whether it landed on anything — the readouts must not print a
    /// distance to open sky (that number is the ray's own reach, not a target's).
    pub(super) fn sight_point(&self, camera: &Camera) -> Option<crate::aim::SightPoint> {
        let eye = Vec3::from_array(camera.eye);
        let forward = (Vec3::from_array(camera.target) - eye).normalize_or_zero();
        (forward != Vec3::ZERO).then(|| {
            crate::aim::aim_point_with_sweep(
                &self.battlefield.heightmap,
                self.live_cover.blocking(),
                self.battlefield.water_view(),
                self.render_state.interpolated_tanks(),
                self.player_tank,
                self.player_team(),
                eye,
                forward,
                // The sight probes the world with the body the SELECTED shell has, so the
                // crosshair can never land on something this round would graze on the way.
                self.predictor.selected_shell().collision_radius_m(),
            )
        })
    }

    /// World-space muzzle, pivoted about the trunnion and ring exactly like the rendered gun and
    /// the server's shell spawn — `muzzle_position_matches_server_shell_origin` locks the three
    /// together.
    pub(super) fn muzzle_position(&self) -> Vec3 {
        let Some(tank) = self.local_render_tank() else {
            let mounts = MountFrames::for_vehicle(self.player_spec().kind);
            return game_core::math::muzzle_world_position_scaled(
                &mounts,
                self.predictor.position(),
                self.predictor.hull_pose(),
                self.predictor.turret_yaw(),
                self.predictor.gun_pitch(),
                self.player_barrel_scale(),
            );
        };
        self.muzzle_position_of(&tank)
    }

    /// The muzzle of a specific local-tank pose — the HUD path feeds the render-blended pose here
    /// so its markers stay glued to the presented camera.
    fn muzzle_position_of(&self, tank: &net::TankSnapshot) -> Vec3 {
        let mounts = MountFrames::for_vehicle(tank.vehicle);
        game_core::math::muzzle_world_position_scaled(
            &mounts,
            Vec3::from_array(tank.position),
            tank.hull_pose(),
            tank.turret_yaw_rad,
            tank.gun_pitch_rad,
            self.player_barrel_scale(),
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

    /// Whether the live dispersion has settled onto the gun's minimum (within a small margin):
    /// the ring brightens as the ready-to-fire signal. Uses the same source as the drawn ring.
    ///
    /// The minimum is THIS gun's, damage included. A wounded gun recovers toward a wider floor
    /// (`sim::base_dispersion_mrad` raises it by up to 2.5x), so measuring against the pristine
    /// spec number meant that from the first splinter in the barrel the circle could never be
    /// called settled: the ready-to-fire signal died for the rest of the battle, exactly when a
    /// hurt tank needs to know its aim is as good as it will get.
    fn player_aim_converged(&self) -> bool {
        if !self.predictor.is_seeded() {
            // Before prediction seeds, the ring is drawn from the spec minimum: it IS settled.
            return true;
        }
        let settled = self.predictor.settled_dispersion_mrad();
        self.predictor.aim_dispersion_mrad() <= settled * 1.12
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
                app.live_cover.camera_obstacles(),
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

    /// The HUD reticle's world points must ride the SAME sub-tick blended pose as the presented
    /// camera. Fed the full-tick pose instead, the gun marker projects up to one tick of hull
    /// motion ahead of the view — at sniper FOV that reads as the marker jittering across the
    /// screen every frame while the hull moves or the turret traverses.
    #[test]
    fn hud_reticle_markers_ride_the_render_blended_pose() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.camera_controller.set_mode(BattleCameraMode::Sniper);
        // A small offset keeps the still-traversing gun inside the sniper FOV so its marker
        // projects on screen at both blend ends.
        app.desired_aim = crate::aim::DesiredAim::new(0.12, 0.02);
        app.run_fixed_ticks(2); // mid-traverse: the previous pose differs from the current one

        let tank = app.local_render_tank().expect("local tank");
        let camera = app.camera_from_tank(tank);
        let view_proj = renderer_api::view_projection_matrix(&camera, 16.0 / 9.0, 0.1, 2000.0);
        let start = app.hud_reticle(&camera, view_proj, 0.0).expect("reticle at tick start");
        let end = app.hud_reticle(&camera, view_proj, 1.0).expect("reticle at tick end");

        let apart = |a: Option<[f32; 2]>, b: Option<[f32; 2]>| match (a, b) {
            (Some(a), Some(b)) => ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() > 1.0e-6,
            _ => false,
        };
        assert!(
            apart(start.gun_clip, end.gun_clip),
            "the gun marker must blend across the tick exactly like the camera: start {:?} end {:?}",
            start.gun_clip,
            end.gun_clip,
        );
    }

    /// A wounded gun recovers toward a WIDER floor, and reaching that floor is still "the aim has
    /// been taken". Measured against the pristine spec number instead, the ring stopped
    /// brightening from the first splinter in the barrel: the ready-to-fire cue died for the rest
    /// of the battle, exactly when a hurt tank most needs to know its aim is as good as it gets.
    #[test]
    fn the_convergence_signal_survives_a_wounded_gun() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(2);

        // Both cases are driven through an authoritative snapshot rather than by waiting for the
        // gun to settle on its own: what is under test is the FLOOR the signal is measured
        // against, not how many ticks a particular spawn takes to reach it.
        let mut healthy = app.player_snapshot().cloned().expect("player snapshot");
        healthy.aim_dispersion_mrad = app.player_spec().gun.dispersion_mrad;
        app.predictor.sync_to(&healthy);
        assert!(app.player_aim_converged(), "a healthy gun at its minimum reads settled");

        let mut wounded = healthy;
        let slot = game_core::ModuleSlot::Gun;
        wounded.module_hit_points[slot.wire_index()] /= 2;
        // As settled as this gun now gets: fully recovered for the damage it carries.
        let floor = sim::base_dispersion_mrad(app.player_spec(), 0.5);
        wounded.aim_dispersion_mrad = floor;
        app.predictor.sync_to(&wounded);

        assert!(
            floor > app.player_spec().gun.dispersion_mrad,
            "half a gun module widens the floor the circle falls to"
        );
        assert!(app.player_aim_converged(), "a settled wounded gun is still settled");
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
