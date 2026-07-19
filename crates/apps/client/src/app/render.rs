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

    fn weather_elapsed_s(&self) -> f32 {
        (self.session.authoritative_tick() as f32 + self.loop_driver.render_alpha())
            / sim::DEFAULT_SIMULATION_TICK_HZ as f32
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

    /// Rebuild the battle scene mesh for the current cover phases and re-upload it, so collapsed
    /// buildings show as rubble and cleared foliage disappears. The rebuilt mesh also replaces the
    /// cached battle scene, so a garage round-trip re-uploads the current (damaged) world, not the
    /// pristine one. Runs only on the frame the states changed (`scene_cover_dirty`).
    pub(super) fn rebuild_cover_scene_if_dirty(&mut self) {
        // Harvest a finished background rebuild first (F7): the upload is cheap; only the
        // 25 ms CPU bake ever lived on the render thread, and that is what moved off it. The
        // world keeps drawing the pre-collapse statics for the couple of frames the bake
        // takes — a building settling one beat later is invisible; a hitch is not.
        if let Some(receiver) = &self.scene_rebuild_rx {
            match receiver.try_recv() {
                Ok((vertices, indices)) => {
                    self.scene_rebuild_rx = None;
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_terrain(&vertices, &indices);
                    }
                    if let Some(meshes) = self.battle_scene_meshes.as_mut() {
                        meshes.statics_vertices = vertices;
                        meshes.statics_indices = indices;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // A panicked bake never uploads; fall through so a dirty flag can retry.
                    self.scene_rebuild_rx = None;
                }
            }
        }
        if !self.scene_cover_dirty {
            return;
        }
        self.scene_cover_dirty = false;
        // Cover phases only touch the STATICS slot: the ground mesh and its baked splat/macro
        // maps are invariant under destruction, so a collapsing building re-uploads a fraction
        // of the world instead of forcing a 1024^2 rebake. The bake itself runs on a worker
        // thread; a collapse arriving while one is in flight re-marks the flag and rebakes
        // with the newest phases once the harvest above completes.
        let (tx, rx) = std::sync::mpsc::channel();
        self.scene_rebuild_rx = Some(rx);
        let battlefield = self.battlefield.clone();
        let phases = self.cover_phase_bytes.clone();
        let scars = self.cover_scar_list.clone();
        std::thread::spawn(move || {
            let _ =
                tx.send(crate::battlefield_statics_mesh_with_scars(&battlefield, &phases, &scars));
        });
    }

    /// Re-mesh the ground for the current crater ledger (true deformation, protocol v31) — the
    /// same F7 shape as the cover rebuild above: bake on a worker, harvest and swap here. The
    /// baked splat/macro maps stay bound (geometry-only), and the cached battle meshes take the
    /// deformed ground so a garage round-trip re-uploads the shelled field, not a pristine one.
    /// Until the harvest lands, the eye keeps the old ground for the couple of frames physics
    /// has already been standing in the hole — invisible at snapshot cadence.
    pub(super) fn rebuild_ground_if_dirty(&mut self) {
        if let Some(receiver) = &self.ground_rebuild_rx {
            match receiver.try_recv() {
                Ok(rebuilt) => {
                    self.ground_rebuild_rx = None;
                    let ((vertices, indices), (dressing_v, dressing_i)) = rebuilt;
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.update_battlefield_ground_geometry(&vertices, &indices);
                        // The card meadow follows the same ledger: the burst that dug the
                        // hole also mowed the cards around it (Żywy Step P2).
                        renderer.set_dressing(&dressing_v, &dressing_i);
                    }
                    if let Some(meshes) = self.battle_scene_meshes.as_mut() {
                        meshes.ground_vertices = vertices;
                        meshes.ground_indices = indices;
                        meshes.dressing_vertices = dressing_v;
                        meshes.dressing_indices = dressing_i;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // A panicked bake never uploads; fall through so a dirty flag can retry.
                    self.ground_rebuild_rx = None;
                }
            }
        }
        if !self.ground_deform_dirty {
            return;
        }
        self.ground_deform_dirty = false;
        let (tx, rx) = std::sync::mpsc::channel();
        self.ground_rebuild_rx = Some(rx);
        // The clone carries the heightmap's crater overlay — the bake reads sample_height,
        // the exact deformed truth the sim and predictor stand on. The ground maps ride along
        // (a few MB, cloned once per crater event) so the card meadow rebakes from the same
        // splat the first bake used.
        let battlefield = self.battlefield.clone();
        let ground_maps =
            self.battle_scene_meshes.as_ref().map(|meshes| meshes.ground_maps.clone());
        let materials = scene_build::terrain_maps::terrain_material_set_for(self.session.map_id());
        std::thread::spawn(move || {
            let ground = crate::battlefield_ground_mesh(&battlefield);
            let dressing = ground_maps
                .map(|maps| {
                    scene_build::grass_cards::grass_card_dressing_mesh(
                        &battlefield,
                        &maps,
                        &materials,
                    )
                })
                .unwrap_or_default();
            let _ = tx.send((ground, dressing));
        });
    }

    /// Swap each wreck's hull render object (the first object for that tank) to its dented
    /// per-instance mesh. Turret, gun, and gear keep the shared meshes.
    pub(super) fn apply_wreck_deform(&self, objects: &mut [renderer_api::RenderObject]) {
        if self.wreck_hull_meshes.is_empty() {
            return;
        }
        for (&id, &handle) in &self.wreck_hull_meshes {
            if let Some(hull) = objects.iter_mut().find(|object| object.tank_id == Some(id)) {
                hull.mesh = handle;
            }
        }
    }

    /// Draw every shed track ribbon (D6): the same unit shoe-link mesh the live track scrolls,
    /// instanced along the frozen S-curve where the throw laid it. Dark bare steel — shed links
    /// stop wearing the vehicle's paint the moment they leave it.
    /// The freshly thrown band still hanging over the drive sprocket (phase 2): for the first
    /// beats after a throw the last stretch of the loop stays draped over the front wrap, then
    /// gravity wins and it slides off to join the band lying on the field. Hull-attached (it
    /// rides the live pose), deterministic, and gone for good once slid — or the moment the
    /// crew re-seats the track (the break heals on the wire).
    pub(super) fn collect_track_remnants(
        &mut self,
        tanks: &[engine::PresentationTank],
    ) -> Vec<renderer_api::RenderObject> {
        const HANG_S: f32 = 7.0;
        let mut objects = Vec::new();
        let ribbons = std::mem::take(&mut self.track_ribbons);
        for ribbon in &ribbons {
            let side_index = match ribbon.side {
                game_core::TrackSide::Left => 0,
                game_core::TrackSide::Right => 1,
            };
            let Some(tank) = tanks.iter().find(|tank| tank.id == ribbon.tank_id) else {
                continue;
            };
            if tank.track_break_t[side_index].is_none() {
                continue; // re-seated: the remnant story is over
            }
            let slide_m = if ribbon.age_s <= HANG_S {
                0.0
            } else {
                let falling = ribbon.age_s - HANG_S;
                2.6 * falling * falling
            };
            let pose = crate::vehicle::pose::VehiclePose::new_with_attitude(
                tank.vehicle,
                glam::Vec3::from_array(tank.translation),
                tank.hull_yaw_rad,
                0.0,
                0.0,
                [tank.attitude_pitch_rad, tank.attitude_roll_rad, tank.attitude_heave_m],
            );
            let hull = glam::Mat4::from_translation(pose.hull_translation())
                * glam::Mat4::from_mat3(pose.hull_basis());
            objects.extend(crate::vehicle::track_ribbon::thrown_remnant_objects(
                &mut self.vehicle_asset_catalog,
                ribbon.tank_id,
                ribbon.vehicle,
                ribbon.side,
                hull,
                slide_m,
            ));
        }
        self.track_ribbons = ribbons;
        objects
    }

    pub(super) fn append_track_ribbons(&mut self, objects: &mut Vec<renderer_api::RenderObject>) {
        let ribbons = std::mem::take(&mut self.track_ribbons);
        for ribbon in &ribbons {
            objects.extend(crate::vehicle::track_ribbon::ribbon_render_objects(
                &mut self.vehicle_asset_catalog,
                ribbon,
            ));
        }
        self.track_ribbons = ribbons;
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
        // F6: the roster's vehicle bakes and GPU registrations happen HERE, at deployment —
        // never on first sight mid-battle.
        self.preload_battle_vehicle_assets();
        let meshes = self.battle_scene_meshes.as_ref().expect("ensured above");
        let mut renderer = WindowRenderer::new(
            window,
            width,
            height,
            &meshes.statics_vertices,
            &meshes.statics_indices,
        )?;
        renderer.set_battlefield_ground(
            &meshes.ground_vertices,
            &meshes.ground_indices,
            &meshes.ground_maps,
            &scene_build::terrain_maps::terrain_material_set_for(self.session.map_id()),
        );
        // The near-field grass tuft (Materia Świata 1b): one registered unit mesh the battle
        // frame instances around the eye every frame.
        renderer.register_mesh(
            scene_build::grass::GRASS_MESH_HANDLE,
            &scene_build::grass::grass_tuft_mesh(),
        );
        let atlas = crate::hud::font::atlas();
        renderer.set_hud_font_atlas(atlas.width(), atlas.height(), atlas.coverage());
        // The battle scene starts loaded, so its river (if the map has one) starts loaded too.
        renderer.set_water(&meshes.water_vertices, &meshes.water_indices);
        // And its mid-field card meadow (Żywy Step P2) — the dressing slot.
        renderer.set_dressing(&meshes.dressing_vertices, &meshes.dressing_indices);
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
        self.weather_frame = self.weather_timeline.sample(self.weather_elapsed_s());
        let frame_dt = self.presentation.time().delta_seconds;
        self.apply_mouse_look();
        if frame_dt > 0.0 {
            let prior = self.fps_estimate; // EMA-smooth FPS for a steady HUD readout.
            let instant = 1.0 / frame_dt;
            self.fps_estimate = if prior <= 0.0 { instant } else { prior * 0.9 + instant * 0.1 };
            // F9: the p95 window — raw intervals, ~1.5 s deep, for the HUD's worst-frame
            // readout (an average hides exactly the drops being hunted).
            if self.frame_dt_history.len() >= 96 {
                self.frame_dt_history.pop_front();
            }
            self.frame_dt_history.push_back(raw_dt);
        }
        self.render_state.set_interpolation_alpha(self.remote_interpolation_alpha());
        self.hit_indicator.tick(frame_dt);
        self.damage_log.tick(frame_dt);
        self.track_feedback.tick(frame_dt);
        self.incoming_hits.tick(frame_dt);
        self.kill_confirm_age_s = self
            .kill_confirm_age_s
            .map(|age| age + frame_dt)
            .filter(|age| *age < crate::hud::kill_marker::KILL_CONFIRM_TTL_S);
        self.fx.tick(frame_dt);
        self.terrain_scars.tick(frame_dt);
        self.track_marks.tick(frame_dt);
        // Shed bands age: the freshly hung remnant over the sprocket slides off with time.
        for ribbon in &mut self.track_ribbons {
            ribbon.age_s += frame_dt;
        }
        self.tick_battle_scars(frame_dt);
        // Cover that collapsed or cleared since the last frame: rebuild and re-upload the scene so
        // the rubble mounds and cleared foliage actually show.
        self.rebuild_cover_scene_if_dirty();
        // Fresh craters since the last snapshot (protocol v31): re-mesh and swap the ground so
        // the hole physics already stands in actually shows.
        self.rebuild_ground_if_dirty();

        let alpha = self.loop_driver.render_alpha();
        // The death spectate (D9): the kill gets its audience.
        let player_dead = self.tick_death_spectate();
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
        self.tick_motion_fx(&presentation_tanks, frame_dt);
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
        // The hanging remnants ride the live poses, so collect them BEFORE the visibility
        // pass consumes the presentation list (a handful of links; culling them is not worth
        // losing the drape on a tank at the screen edge).
        let remnant_objects = self.collect_track_remnants(&presentation_tanks);
        let visible_tanks = self.visible_render_tanks(presentation_tanks, view_proj, camera.eye);
        let player_gun_scale = self.player_barrel_scale();
        let mut vehicles = split_pbr_vehicle_render_frame_on_terrain(
            &mut self.vehicle_asset_catalog,
            visible_tanks,
            self.player_tank,
            player_gun_scale,
            Some(&self.battlefield.heightmap),
            self.render_state.latest_snapshot().map_or(0, |snapshot| snapshot.server_tick),
        );
        // A decapitated wreck flies its turret: replace that tank's turret and gun transforms with
        // the deterministic pop-off arc (the snapshot pose is ignored, freezing the turret yaw).
        self.apply_turret_popoffs(&mut vehicles.objects);
        // A wreck also draws its dented hull instead of the pristine shared one.
        self.apply_wreck_deform(&mut vehicles.objects);
        // Thrown tracks lie where they were shed (D6), instanced from the shoe-link mesh.
        self.append_track_ribbons(&mut vehicles.objects);
        // And the freshly thrown band still hanging over the sprocket (phase 2).
        vehicles.objects.extend(remnant_objects);
        let vehicle_frame = RenderFrame {
            objects: vehicles.objects,
            armor_damage: vehicles.armor_damage,
            ..RenderFrame::default()
        };
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
        // The denial pulse ages with the frame clock and expires with its flash.
        self.fire_denied_age_s =
            self.fire_denied_age_s.map(|age| age + frame_dt).filter(|age| *age < 0.4);
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
            frame_p95_ms: {
                let mut sorted: Vec<f32> = self.frame_dt_history.iter().copied().collect();
                sorted.sort_by(f32::total_cmp);
                if sorted.is_empty() { 0.0 } else { sorted[(sorted.len() * 95) / 100] * 1000.0 }
            },
            speed_kmh: self.player_speed_kmh(),
            zoom_factor: self.camera_controller.zoom_factor(),
            damage_log: self.damage_log.visible(),
            track_feedback: self.track_feedback.model(),
            incoming_hits: self.incoming_hits.screen_hits(camera_forward_xz),
            ammo: Some(self.player_ammo_hud()),
            modules: self.player_module_hud(),
            minimap,
            battle_outcome: self.battle_outcome,
            battle_clock_remaining_s: self.session.battle_time_remaining_s(),
            kill_confirm_age_s: self.kill_confirm_age_s,
            reload_ready_age_s: self.reload_ready_age_s,
            fire_denied_age_s: self.fire_denied_age_s,
            scope_fade: self.camera_controller.scope_dressing(),
        };
        // The death spectate clears the stage (D9): no vitals, no reticle, no bars — the wreck
        // epilogue IS the picture. The end-of-battle overlay still comes through when it lands.
        let spectating = player_dead && self.battle_outcome.is_none();
        let mut hud =
            if spectating { Vec::new() } else { crate::hud::build_battle_hud(&hud_model, aspect) };
        if !spectating {
            hud.extend(enemy_bars);
            hud.extend(self.hit_indicator.render_vertices(view_proj, aspect));
        }
        let fx_vertices = self.fx_frame_vertices(camera.eye, camera.target);
        let scene_time_s = self.presented_time_s();
        self.ensure_scene(SceneKind::Battle);
        let weather = self.weather_frame;
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.set_outdoor_sky(weather.sky.0, weather.sky.1, weather.sky.2);
        renderer.set_scene_lighting(weather.lighting);
        renderer.set_rain_intensity(weather.rain_intensity);
        renderer.set_wetness(weather.surface_wetness);
        renderer.set_weather_dynamics(
            weather.puddle_fill,
            weather.cloud_offset,
            weather.rain_phase_s,
        );
        for (handle, mesh) in self.vehicle_asset_catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(handle, &mesh);
        }
        for (handle, maps) in self.vehicle_asset_catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(handle, &maps);
        }
        // Near-field grass (Materia Swiata 1b): a deterministic tuft field conjured around
        // the eye through the scene pipeline's instanced path. Cached between frames — the
        // scatter is a pure function of (eye cell, crater ledger), so it re-conjures only
        // when the eye moves a real step or a fresh crater burns a patch out.
        let eye = glam::Vec3::from_array(camera.eye);
        let crater_count = self.battlefield.heightmap.crater_records().len();
        let moved =
            self.grass_cache_eye.is_none_or(|cached| cached.distance_squared(eye) > 1.5 * 1.5);
        if moved || crater_count != self.grass_cache_crater_count {
            self.grass_cache = scene_build::grass::grass_frame_objects(
                &self.battlefield.heightmap,
                self.battlefield.water,
                &self.battle_scene_meshes.as_ref().expect("ensured above").ground_maps,
                &scene_build::terrain_maps::terrain_material_set_for(self.session.map_id()),
                eye,
            );
            self.grass_cache_eye = Some(eye);
            self.grass_cache_crater_count = crater_count;
        }
        renderer.set_render_frame(&RenderFrame {
            objects: self.grass_cache.clone(),
            ..RenderFrame::default()
        });
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
