use std::sync::Arc;
use std::time::Instant;

use renderer_api::{CameraProjectionPolicy, RenderError, RenderFrame, view_projection_matrix};
use renderer_wgpu::WindowRenderer;
use sim::{DEFAULT_SERVER_TICK_HZ, DEFAULT_SNAPSHOT_HZ};
use tracing::error;
use winit::window::Window;

use super::{ClientApp, SceneKind};
use crate::hud::HudVitals;
use crate::split_pbr_vehicle_render_frame_on_terrain;

impl ClientApp {
    /// Remote interpolation phase: how far the fixed-tick clock has advanced from the latest
    /// ingested snapshot toward the next one (`ticks since + sub-tick remainder, over the
    /// snapshot window`). Snapshots are produced by the same fixed-tick loop, so this phase hits
    /// exactly 1 as the next snapshot lands — integrating render-frame deltas instead let the
    /// two clocks drift, freezing remote tanks at the window's end and then jumping them.
    pub(super) fn remote_interpolation_alpha(&self) -> f32 {
        let window_ticks = self
            .render_state
            .snapshot_interval_ticks()
            .unwrap_or((DEFAULT_SERVER_TICK_HZ / DEFAULT_SNAPSHOT_HZ) as u64)
            as f32;
        ((self.ticks_since_snapshot as f32 + self.loop_driver.render_alpha()) / window_ticks)
            .clamp(0.0, 1.0)
    }

    /// The presentation clock shaders animate with (water, foliage, weather): whole fixed ticks
    /// plus the sub-tick render phase, over the tick rate. Purely tick-domain — never integrated
    /// from render-frame deltas, so a jittery frame clock cannot wobble world animation (the same
    /// doctrine as `engine::TankMotion` and `remote_interpolation_alpha`).
    pub(super) fn presented_time_s(&self) -> f32 {
        ((self.client_tick as f64 + f64::from(self.loop_driver.render_alpha()))
            / f64::from(sim::DEFAULT_SIMULATION_TICK_HZ)) as f32
    }
    /// Drive each decapitated wreck's turret and gun render objects from its deterministic pop-off
    /// arc. Per tank the objects are laid out `[hull, turret, gun, ...gear]` contiguously, so the
    /// two objects after the hull are the turret and gun; anything else (a legacy vehicle whose
    /// order differs) is skipped rather than mis-driven.
    pub(super) fn apply_turret_popoffs(&self, objects: &mut [renderer_api::RenderObject]) {
        if self.turret_popoffs.is_empty() {
            return;
        }
        for (&id, popoff) in &self.turret_popoffs {
            let Some(hull) = objects.iter().position(|object| object.tank_id == Some(id)) else {
                continue;
            };
            let owns_turret = objects.get(hull + 1).map(|o| o.tank_id) == Some(Some(id));
            let owns_gun = objects.get(hull + 2).map(|o| o.tank_id) == Some(Some(id));
            if owns_turret {
                objects[hull + 1].transform = popoff.turret_transform().to_cols_array_2d();
            }
            if owns_gun {
                objects[hull + 2].transform = popoff.gun_transform().to_cols_array_2d();
            }
        }
    }

    pub(super) fn create_renderer(
        &mut self,
        window: Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        // Terrain plus static cover: everything the simulation collides must be visible. The
        // bake lands in the app-lifetime cache, so later garage→battle swaps reuse it instead
        // of freezing their first battle frame on a rebake.
        self.ensure_battle_scene_meshes();
        let meshes = self.battle_scene_meshes.as_ref().expect("ensured above");
        let mut renderer = WindowRenderer::new(
            window,
            width,
            height,
            &meshes.terrain_vertices,
            &meshes.terrain_indices,
        )?;
        let atlas = crate::hud::font::atlas();
        renderer.set_hud_font_atlas(atlas.width(), atlas.height(), atlas.coverage());
        // The battle scene starts loaded, so its river (if the map has one) starts loaded too.
        renderer.set_water(&meshes.water_vertices, &meshes.water_indices);
        self.renderer = Some(renderer);
        // The renderer is born holding generic battlefield defaults; the app is born in battle,
        // so dress it in the actual match's weather right away.
        self.apply_match_weather();
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
        self.render_state.set_interpolation_alpha(self.remote_interpolation_alpha());
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
            // Speed comes from the predictor's rigid body (tick domain), not from differencing
            // presented positions against the render clock — that difference is jitter.
            self.camera_controller.advance(local.position, self.predictor.speed_mps(), raw_dt);
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
        let visible_tanks = self.visible_render_tanks(presentation_tanks, view_proj, camera.eye);
        let player_gun_scale = self.player_barrel_scale();
        let mut vehicles = split_pbr_vehicle_render_frame_on_terrain(
            &mut self.vehicle_asset_catalog,
            visible_tanks,
            self.player_tank,
            player_gun_scale,
            Some(&self.battlefield.heightmap),
        );
        // A decapitated wreck flies its turret: replace that tank's turret and gun transforms with
        // the deterministic pop-off arc (the snapshot pose is ignored, freezing the turret yaw).
        self.apply_turret_popoffs(&mut vehicles.objects);
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
        // Bringing the eye to the optics (and lifting it away) clicks — the mechanical half of
        // the transition the irising scope surround plays visually.
        let camera_mode = self.camera_controller.mode();
        if self.prev_camera_mode.is_some_and(|previous| previous != camera_mode) {
            self.queue_audio(audio::AudioEvent::UiClick { accent: false });
        }
        self.prev_camera_mode = Some(camera_mode);
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
            scope_fade: self.camera_controller.scope_dressing(),
        };
        let mut hud = crate::hud::build_battle_hud(&hud_model, aspect);
        hud.extend(enemy_bars);
        hud.extend(self.hit_indicator.render_vertices(view_proj, aspect));
        let fx_vertices = self.fx_frame_vertices(camera.eye, camera.target);
        let scene_time_s = self.presented_time_s();
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
        renderer.set_scene_time_s(scene_time_s);
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
