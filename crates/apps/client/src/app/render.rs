use std::sync::Arc;
use std::time::Instant;

use renderer_api::{CameraProjectionPolicy, RenderError, RenderFrame, view_projection_matrix};
use renderer_wgpu::WindowRenderer;
use sim::DEFAULT_SNAPSHOT_HZ;
use tracing::error;
use winit::window::Window;

use super::{ClientApp, SceneKind};
use crate::hud::HudVitals;
use crate::{battlefield_scene_mesh, split_pbr_vehicle_render_frame_on_terrain};

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
        self.damage_log.tick(frame_dt);
        self.incoming_hits.tick(frame_dt);
        self.kill_confirm_age_s = self
            .kill_confirm_age_s
            .map(|age| age + frame_dt)
            .filter(|age| *age < crate::hud::kill_marker::KILL_CONFIRM_TTL_S);
        self.fx.tick(frame_dt);
        self.terrain_scars.tick(frame_dt);
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
        let Some(camera) = self.presented_camera_for_player(alpha, raw_dt) else {
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
        let camera_forward_xz =
            [camera.target[0] - camera.eye[0], camera.target[2] - camera.eye[2]];
        let minimap = self.build_minimap(&presentation_tanks, camera_forward_xz);
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
        self.reload_ready_age_s = tick_ready_flash(
            self.reload_ready_age_s,
            self.prev_reload_remaining_s,
            reload_remaining,
            frame_dt,
        );
        // The ready flash's audible twin: the breech clacks the frame the reload completes.
        if self.prev_reload_remaining_s > 0.0 && reload_remaining <= 0.0 {
            self.queue_audio(audio::AudioEvent::GunReady);
        }
        self.prev_reload_remaining_s = reload_remaining;
        self.flush_audio(Some(camera.eye), Some(camera.target));
        let vitals = HudVitals {
            hit_points: self.player_hud_hit_points(),
            max_hit_points: self.player_max_hit_points(),
            reload_remaining_s: reload_remaining,
            reload_seconds: reload_max,
        };
        let hud_model = crate::hud::BattleHudModel {
            vitals,
            reticle: self.hud_reticle(&camera, view_proj, alpha),
            fps: self.fps_estimate,
            speed_kmh: self.player_speed_kmh(),
            zoom_factor: self.camera_controller.zoom_factor(),
            damage_log: self.damage_log.visible(),
            incoming_hits: self.incoming_hits.screen_hits(camera_forward_xz),
            ammo: Some(self.player_ammo_hud()),
            minimap,
            battle_outcome: self.battle_outcome,
            battle_clock_remaining_s: self.local_server.battle_time_remaining_s(),
            kill_confirm_age_s: self.kill_confirm_age_s,
            reload_ready_age_s: self.reload_ready_age_s,
        };
        let mut hud = crate::hud::build_battle_hud(&hud_model, aspect);
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
}

/// Advance the gun-ready flash clock: the beat starts the frame the reload crosses to ready,
/// ages per presented frame, and expires after its TTL. Battle start (reload already at zero)
/// never fires it — only a real reload finishing does.
fn tick_ready_flash(
    age: Option<f32>,
    prev_remaining_s: f32,
    remaining_s: f32,
    dt: f32,
) -> Option<f32> {
    if prev_remaining_s > 0.0 && remaining_s <= 0.0 {
        return Some(0.0);
    }
    age.map(|a| a + dt).filter(|a| *a < crate::hud::reticle_marks::READY_FLASH_TTL_S)
}

#[cfg(test)]
mod ready_flash_tests {
    use super::tick_ready_flash;

    #[test]
    fn the_flash_fires_on_the_ready_crossing_ages_and_expires() {
        // Battle start: reload has always been ready — no flash.
        assert_eq!(tick_ready_flash(None, 0.0, 0.0, 0.016), None);
        // Mid-reload: still none.
        assert_eq!(tick_ready_flash(None, 3.2, 3.0, 0.016), None);
        // The crossing frame starts the beat.
        assert_eq!(tick_ready_flash(None, 0.1, 0.0, 0.016), Some(0.0));
        // Frames age it...
        assert_eq!(tick_ready_flash(Some(0.0), 0.0, 0.0, 0.1), Some(0.1));
        // ...and past the TTL it expires.
        let expired =
            tick_ready_flash(Some(crate::hud::reticle_marks::READY_FLASH_TTL_S), 0.0, 0.0, 0.016);
        assert_eq!(expired, None);
    }
}
