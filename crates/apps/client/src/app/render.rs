use std::sync::Arc;
use std::time::Instant;

use net::TankSnapshot;
use renderer_api::{
    Camera, CameraProjectionPolicy, RenderError, RenderFrame, view_projection_matrix,
};
use renderer_wgpu::WindowRenderer;
use sim::DEFAULT_SNAPSHOT_HZ;
use tracing::error;
use winit::window::Window;

use super::{ClientApp, SceneKind};
use crate::hud::{HudVitals, build_hud_with_reticle};
use crate::{
    BattleCameraEnvironment, CameraSubject, battlefield_scene_mesh,
    split_pbr_vehicle_render_frame_on_terrain,
};

const SNAPSHOT_INTERVAL_SECONDS: f32 = 1.0 / DEFAULT_SNAPSHOT_HZ as f32;

impl ClientApp {
    pub(super) fn create_renderer(
        &mut self,
        window: Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        // Terrain plus static cover: everything the simulation collides must be visible.
        let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&self.battlefield);
        let mut renderer =
            WindowRenderer::new(window, width, height, &terrain_vertices, &terrain_indices)?;
        let atlas = crate::hud::font::atlas();
        renderer.set_hud_font_atlas(atlas.width(), atlas.height(), atlas.coverage());
        self.renderer = Some(renderer);
        Ok(())
    }

    pub(super) fn render_now(&mut self) {
        if self.garage.is_open() {
            self.render_garage();
            return;
        }
        let now = Instant::now();
        let raw_dt = now.saturating_duration_since(self.last_render_time).as_secs_f32();
        self.last_render_time = now;
        // Route the frame clock through the presentation world so `engine::Time` is the single
        // render-side time source the rest of the frame reads from.
        self.presentation.advance_time(raw_dt);
        let frame_dt = self.presentation.time().delta_seconds;
        self.apply_mouse_look();
        if frame_dt > 0.0 {
            let prior = self.fps_estimate; // EMA-smooth FPS for a steady HUD readout.
            let instant = 1.0 / frame_dt;
            self.fps_estimate = if prior <= 0.0 { instant } else { prior * 0.9 + instant * 0.1 };
        }
        self.render_state.advance(frame_dt, SNAPSHOT_INTERVAL_SECONDS);
        self.hit_indicator.tick(frame_dt);
        self.fx.tick(frame_dt);
        self.tick_battle_scars(frame_dt);

        let alpha = self.loop_driver.render_alpha();
        // A landing the predictor absorbed since the last frame slams the camera rig once.
        let landing_impact = self.predictor.take_landing_impact_mps();
        if landing_impact > 0.0 {
            self.camera_controller.impact_kick(landing_impact);
        }
        if let Some(local) = self.interpolated_local_tank(alpha) {
            self.camera_controller.advance(local.position, raw_dt);
        }
        let Some(camera) = self.camera_for_player_interpolated(alpha) else {
            return;
        };
        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, WindowRenderer::aspect_ratio);
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        // Project the interpolated (+ locally predicted) tanks into the persistent presentation
        // world, then drive the scene and HUD from the ECS — not from the snapshot vec directly.
        let presentation_tanks = self.project_render_tanks(alpha);
        let enemy_bars = crate::hud::health_bar::enemy_health_bars(
            &presentation_tanks,
            self.player_tank,
            self.player_team(),
            view_proj,
            aspect,
        );
        let visible_tanks = self.visible_render_tanks(presentation_tanks);
        let player_gun_scale = self.player_barrel_scale();
        let vehicles = split_pbr_vehicle_render_frame_on_terrain(
            &mut self.vehicle_asset_catalog,
            visible_tanks,
            self.player_tank,
            player_gun_scale,
            Some(&self.battlefield.heightmap),
        );
        let vehicle_frame = RenderFrame { objects: vehicles.objects, ..RenderFrame::default() };
        let (reload_remaining, reload_max) = self.player_reload();
        let vitals = HudVitals {
            hit_points: self.player_hud_hit_points(),
            max_hit_points: self.player_max_hit_points(),
            reload_remaining_s: reload_remaining,
            reload_seconds: reload_max,
        };
        let mut hud = build_hud_with_reticle(
            vitals,
            aspect,
            self.hud_reticle(&camera, view_proj),
            self.fps_estimate,
            self.player_speed_kmh(),
        );
        hud.extend(enemy_bars);
        hud.extend(self.hit_indicator.render_vertices(view_proj, aspect));
        let fx_vertices = self.fx_frame_vertices(camera.eye, camera.target);
        self.ensure_scene(SceneKind::Battle);
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        for (handle, mesh) in self.vehicle_asset_catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(handle, &mesh);
        }
        for (handle, maps) in self.vehicle_asset_catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(handle, &maps);
        }
        renderer.set_render_frame(&RenderFrame::default());
        renderer.set_vehicle_render_frame(&vehicle_frame);
        // Battle no longer builds a per-frame dynamic mesh (hit marks became on-tank decals in
        // the FX pass); clear whatever the garage left behind.
        renderer.set_dynamic_mesh(&[], &[]);
        renderer.set_fx(&fx_vertices);
        renderer.set_hud(&hud);
        if let Err(error) = renderer.render(view_proj, camera.eye) {
            error!(%error, "frame render failed");
        }
    }

    pub(super) fn render_tanks(&self, alpha: f32) -> Vec<TankSnapshot> {
        let mut tanks = self.render_state.interpolated_tanks();
        if let Some(local) = self.interpolated_local_tank(alpha)
            && let Some(slot) = tanks.iter_mut().find(|tank| tank.tank_id == self.player_tank)
        {
            *slot = local;
        }
        tanks
    }

    /// Sync this frame's rendered tanks into the persistent presentation world and read the
    /// presentation view back out. The renderer and HUD consume this ECS projection rather than
    /// the snapshot vec.
    pub(super) fn project_render_tanks(&mut self, alpha: f32) -> Vec<engine::PresentationTank> {
        let render_tanks = self.render_tanks(alpha);
        self.presentation.sync_tanks(&render_tanks);
        self.presentation.presentation_tanks()
    }

    /// The sniper eye sits at turret-roof height inside the player's own mesh, so the player's
    /// vehicle is hidden in sniper view; everything else always draws.
    pub(super) fn visible_render_tanks(
        &self,
        tanks: Vec<engine::PresentationTank>,
    ) -> Vec<engine::PresentationTank> {
        if self.camera_controller.mode() == crate::BattleCameraMode::Sniper {
            tanks.into_iter().filter(|tank| tank.id != self.player_tank).collect()
        } else {
            tanks
        }
    }

    /// Render camera from the interpolated hull pose: rigid follow, no eye spring, no lag.
    fn camera_for_player_interpolated(&self, alpha: f32) -> Option<Camera> {
        Some(self.camera_from_tank(self.interpolated_local_tank(alpha)?))
    }

    pub(super) fn camera_from_tank(&self, tank: TankSnapshot) -> Camera {
        let gun_pitch = tank.gun_pitch_rad;
        let turret_view_yaw = tank.yaw_rad + tank.turret_yaw_rad;
        let view_yaw = if self.input.free_look {
            self.camera_controller.orbit_yaw_rad()
        } else if self.camera_controller.mode() == crate::BattleCameraMode::ThirdPerson {
            self.desired_aim.yaw_rad()
        } else {
            turret_view_yaw
        };
        let subject = CameraSubject::from_snapshot(tank, gun_pitch)
            .with_view_yaw(view_yaw)
            .with_desired_aim(self.desired_aim.yaw_rad(), self.desired_aim.pitch_rad());
        let environment = BattleCameraEnvironment::with_obstacles(
            &self.battlefield.heightmap,
            &self.camera_obstacles,
        );
        self.camera_controller.render_camera(&subject, &environment)
    }

    /// This frame's full FX batch: the ticked particle pool plus a tracer per in-flight shell
    /// (tracers are stateless — rebuilt each frame from the interpolated shell snapshots).
    pub(super) fn fx_frame_vertices(
        &self,
        eye: [f32; 3],
        target: [f32; 3],
    ) -> Vec<renderer_api::FxVertex> {
        let eye = glam::Vec3::from_array(eye);
        let mut fx_vertices = self.fx.vertices(eye, glam::Vec3::from_array(target));
        let shells = self.render_state.interpolated_shells(SNAPSHOT_INTERVAL_SECONDS);
        crate::fx::append_shell_tracers(&mut fx_vertices, &shells, eye);
        self.append_scar_quads(&mut fx_vertices);
        fx_vertices
    }
}
