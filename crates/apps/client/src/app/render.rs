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

/// The CPU population carries a six-metre invisible margin, so a normal four-metre planar
/// cache step cannot stream a tuft that the shader could currently show.
const GRASS_CACHE_REBUILD_M: f32 = 4.0;
const _: () = assert!(GRASS_CACHE_REBUILD_M < scene_build::grass::GRASS_CACHE_MARGIN_M);

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
                Ok(rebuild) => {
                    self.scene_rebuild_rx = None;
                    if let Some(meshes) = self.battle_scene_meshes.as_mut() {
                        for (bucket, fragment) in rebuild.buckets {
                            meshes.statics_buckets[bucket] = fragment;
                        }
                        let (vertices, indices) =
                            crate::assemble_statics_mesh(&meshes.statics_buckets);
                        if let Some(renderer) = self.renderer.as_mut() {
                            renderer.set_terrain(&vertices, &indices);
                        }
                        meshes.statics_vertices = vertices;
                        meshes.statics_indices = indices;
                        meshes.statics_baked_phases = rebuild.phases;
                        meshes.statics_baked_scars = rebuild.scars;
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
        // of the world instead of forcing a 1024^2 rebake. The bake goes further (PR-04): the
        // baseline diff below names the DIRTY BUCKETS, and the worker re-bakes only those —
        // one collapsed building costs one map cell, not the whole statics mesh. A collapse
        // arriving while a bake is in flight re-marks the flag and diffs against the baseline
        // the harvest above just landed.
        let Some(meshes) = self.battle_scene_meshes.as_ref() else {
            return;
        };
        let phases = self.live_cover.phase_bytes().to_vec();
        let scars = self.cover_scar_list.clone();
        let mut dirty = [false; crate::STATICS_BUCKET_COUNT];
        for (index, cover) in self.battlefield.static_cover.iter().enumerate() {
            let now = phases.get(index).copied().unwrap_or(0);
            let then = meshes.statics_baked_phases.get(index).copied().unwrap_or(0);
            if now != then {
                for bucket in crate::statics_buckets_touched_by_cover(&self.battlefield, cover) {
                    dirty[bucket] = true;
                }
            }
        }
        for scar in scars
            .iter()
            .filter(|scar| !meshes.statics_baked_scars.contains(scar))
            .chain(meshes.statics_baked_scars.iter().filter(|scar| !scars.contains(scar)))
        {
            if let Some(cover) = self.battlefield.static_cover.get(scar.cover as usize) {
                for bucket in crate::statics_buckets_touched_by_cover(&self.battlefield, cover) {
                    dirty[bucket] = true;
                }
            }
        }
        let dirty_buckets: Vec<usize> =
            (0..crate::STATICS_BUCKET_COUNT).filter(|&bucket| dirty[bucket]).collect();
        if dirty_buckets.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.scene_rebuild_rx = Some(rx);
        // An `Arc` handle: the worker only reads the map (see `ClientApp::battlefield`).
        let battlefield = std::sync::Arc::clone(&self.battlefield);
        std::thread::spawn(move || {
            let buckets = dirty_buckets
                .into_iter()
                .map(|bucket| {
                    (
                        bucket,
                        crate::battlefield_statics_bucket_mesh(
                            &battlefield,
                            &phases,
                            &scars,
                            bucket,
                        ),
                    )
                })
                .collect();
            let _ = tx.send(super::StaticsRebuild { phases, scars, buckets });
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
                    let super::GroundRebuild { ground: (vertices, indices), dressing } = rebuilt;
                    // The fingerprint is recorded only where the upload actually happened, so a
                    // harvest that arrives before the renderer exists cannot leave the client
                    // believing the GPU holds a meadow it never received.
                    let mut uploaded = None;
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.update_battlefield_ground_geometry(&vertices, &indices);
                        // The card meadow follows the same ledger: the burst that dug the
                        // hole also mowed the cards around it (Żywy Step P2). Usually it mowed
                        // nothing — and then the worker did not even bake one.
                        if let Some(rebuild) = dressing.as_ref() {
                            renderer.set_dressing(&rebuild.mesh.0, &rebuild.mesh.1);
                            uploaded = Some(rebuild.fingerprint);
                        }
                    }
                    if let Some(fingerprint) = uploaded {
                        self.dressing_uploaded_fingerprint = fingerprint;
                    }
                    if let Some(meshes) = self.battle_scene_meshes.as_mut() {
                        meshes.ground_vertices = vertices;
                        meshes.ground_indices = indices;
                        if let Some(rebuild) = dressing {
                            meshes.dressing_vertices = rebuild.mesh.0;
                            meshes.dressing_indices = rebuild.mesh.1;
                            meshes.meadow_footprint = rebuild.footprint;
                            meshes.meadow_baked_craters = rebuild.craters;
                        }
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
        // Both handles are `Arc` clones — pointer bumps, not copies. The battlefield carries the
        // heightmap's crater overlay (the bake reads `sample_height`, the exact deformed truth
        // the sim and predictor stand on) and the ground maps ride along so the card meadow
        // rebakes from the same splat the first bake used. Deep-copying them here used to put a
        // ~12 MB memcpy on the RENDER thread in the very frame an HE round landed — the bake
        // was moved off this thread precisely so the frame would not pay for the crater.
        let battlefield = std::sync::Arc::clone(&self.battlefield);
        let materials = scene_build::terrain_maps::terrain_material_set_for(self.session.map_id());
        let uploaded_dressing = self.dressing_uploaded_fingerprint;
        // Whether the meadow is worth baking at all. The ground always is — the hole is real —
        // but the meadow reads the ledger only through the cards a burst mows and the ground a
        // bowl moves, so a shell that landed clear of every card cannot have changed it. That is
        // decided HERE, from a coarse footprint of where the meadow stands, in microseconds,
        // instead of by baking 130-250 ms of mesh and comparing it afterwards.
        let meadow = self.battle_scene_meshes.as_ref().map(|meshes| {
            (
                std::sync::Arc::clone(&meshes.ground_maps),
                meshes.meadow_footprint.clone(),
                meshes.meadow_baked_craters.clone(),
            )
        });
        let bake_meadow = meadow.as_ref().is_none_or(|(_, footprint, baked)| {
            crate::meadow_changed_by(baked, self.battlefield.heightmap.crater_records(), footprint)
        });
        std::thread::spawn(move || {
            let ground = crate::battlefield_ground_mesh(&battlefield);
            let dressing = bake_meadow
                .then(|| {
                    let (maps, _, _) = meadow.as_ref()?;
                    let mesh = scene_build::grass_cards::grass_card_dressing_mesh(
                        &battlefield,
                        maps,
                        &materials,
                    );
                    // A bake is not a change either. Even when a burst DID reach the meadow's
                    // footprint, the cell it reached may have had nothing standing in it — the
                    // footprint is a cell-resolution answer, deliberately conservative. So the
                    // upload is still decided by comparing what came out against what the GPU
                    // holds; the render thread only ever compares two `u64`s.
                    let fingerprint = renderer_api::scene_mesh_fingerprint(&mesh.0, &mesh.1);
                    (fingerprint != uploaded_dressing).then(|| super::DressingRebuild {
                        footprint: crate::MeadowFootprint::of(
                            &mesh.0,
                            battlefield.heightmap.extent_m(),
                        ),
                        craters: battlefield.heightmap.crater_records().to_vec(),
                        mesh,
                        fingerprint,
                    })
                })
                .flatten();
            let _ = tx.send(super::GroundRebuild { ground, dressing });
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
        for (handle, mesh) in scene_build::grass::grass_species_meshes() {
            renderer.register_mesh(handle, &mesh);
        }
        // The battlefield-tree LOD ladder: three uploads at deployment serve every oak on
        // the map, and the battle frame picks a rung per tree per frame.
        for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
            renderer.register_mesh(handle, &mesh);
        }
        // The procedural leaf atlas (Drzewa 3.0 PR5): replaces the renderer's 1x1 white no-op.
        // Slot 0 keeps uv (0,0) opaque white, so until geometry carries nonzero UVs this is
        // pixel-identical — the leaf cards (PR6) are what starts sampling the real slots.
        let (foliage_color, foliage_normal) =
            scene_build::foliage_atlas_paint::foliage_atlas_chains();
        renderer.set_foliage_atlas(&foliage_color, Some(&foliage_normal));
        let atlas = crate::hud::font::atlas();
        renderer.set_hud_font_atlas(atlas.width(), atlas.height(), atlas.coverage());
        // The battle scene starts loaded, so its river (if the map has one) starts loaded too.
        renderer.set_water(&meshes.water_vertices, &meshes.water_indices);
        // And its mid-field card meadow (Żywy Step P2) — the dressing slot. What went up is
        // recorded, so the first crater rebake can recognise its own output and skip the upload.
        renderer.set_dressing(&meshes.dressing_vertices, &meshes.dressing_indices);
        self.dressing_uploaded_fingerprint = renderer_api::scene_mesh_fingerprint(
            &meshes.dressing_vertices,
            &meshes.dressing_indices,
        );
        self.renderer = Some(renderer);
        // The renderer is born holding generic battlefield defaults; the app is born in battle,
        // so dress it in the actual match's weather right away.
        self.apply_match_weather();
        Ok(())
    }

    /// The HUD's worst-frame readout (F9): the 95th percentile of the raw frame intervals.
    ///
    /// It runs on every presented frame, so it does not get to allocate a `Vec` and fully sort
    /// it there — a frame-drop meter that costs a slice of the frame is its own subject. The
    /// scratch buffer is reused, and `select_nth_unstable` finds the percentile in one linear
    /// pass instead of ordering the other ninety-five samples nobody reads.
    fn frame_p95_ms(&mut self) -> f32 {
        if self.frame_dt_history.is_empty() {
            return 0.0;
        }
        let scratch = &mut self.frame_p95_scratch;
        scratch.clear();
        scratch.extend(self.frame_dt_history.iter().copied());
        let index = (scratch.len() * 95) / 100;
        let (_, p95, _) = scratch.select_nth_unstable_by(index, f32::total_cmp);
        *p95 * 1000.0
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
        // Where every live shell is this frame, remembered for the path it draws (A8).
        let shells =
            self.render_state.interpolated_shells(super::frame_scene::SNAPSHOT_INTERVAL_SECONDS);
        self.fx.record_shells(&shells, frame_dt);
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
        // Project the interpolated (+ locally predicted) tanks into the persistent presentation
        // world BEFORE the camera reads it: the presented rig's sprung-dive residual is the
        // presentation spring minus THIS frame's authoritative pitch, and stepping the spring
        // after the camera made the residual a one-frame-stale difference — a term that spiked
        // exactly when the hull pitch changed fastest (a bump), which is a camera nod nobody
        // authored. The scene and HUD read the same list below.
        let presentation_tanks = self.project_render_tanks(alpha);
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
            Some(camera.eye),
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
        self.reload_ready_age_s = tick_ready_ring(
            self.reload_ready_age_s,
            self.prev_reload_remaining_s,
            reload_remaining,
            frame_dt,
        );
        // The loaded ring's audible twin: the breech clacks the frame the reload completes.
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
        let frame_p95_ms = self.frame_p95_ms();
        let vitals = HudVitals {
            hit_points: self.player_hud_hit_points(),
            max_hit_points: self.player_max_hit_points(),
            reload_remaining_s: reload_remaining,
            reload_seconds: reload_max,
        };
        // The marker's colour is EASED toward the matrix's answer: a verdict flipping across a
        // plate edge as the mouse twitches must settle, not strobe.
        let mut reticle = self.hud_reticle(&camera, view_proj, alpha);
        if let Some(reticle) = reticle.as_mut() {
            self.reticle_marker_color = crate::hud::reticle_overlay::ease_marker_color(
                self.reticle_marker_color,
                reticle.marker_color,
                frame_dt,
            );
            reticle.marker_color = self.reticle_marker_color;
        }
        let hud_model = crate::hud::BattleHudModel {
            vitals,
            reticle,
            fps: self.fps_estimate,
            frame_p95_ms,
            speed_kmh: self.player_speed_kmh(),
            zoom_factor: self.camera_controller.zoom_factor(),
            damage_log: self.damage_log.visible(),
            track_feedback: self.track_feedback.model(),
            rack_fire_remaining_s: self
                .player_snapshot()
                .and_then(|tank| tank.rack_fire_remaining_s),
            incoming_hits: self.incoming_hits.screen_hits(camera_forward_xz),
            ammo: Some(self.player_ammo_hud()),
            modules: self.player_module_hud(),
            crew: self.player_snapshot().map(|tank| {
                crate::hud::crew_panel::CrewPanelModel::new(
                    tank.crew_unconscious_mask,
                    tank.crew_weakened_mask,
                    tank.crew_down_remaining_s,
                )
            }),
            minimap,
            battle_outcome: self.battle_outcome,
            battle_clock_remaining_s: self.session.battle_time_remaining_s(),
            kill_confirm_age_s: self.kill_confirm_age_s,
            reload_ready_age_s: self.reload_ready_age_s,
            fire_denied_age_s: self.fire_denied_age_s,
            scope_fade: self.camera_controller.scope_dressing(),
            pause_menu: self
                .pause_menu
                .as_ref()
                .map(|menu| crate::hud::pause_menu::PauseMenuModel { hovered: menu.hovered() }),
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
        // Reused scratch (recovered after `set_fx` below), so the ~1 MiB FX batch is not
        // reallocated every presented frame — the same pattern as the grass frame.
        let mut fx_live = std::mem::take(&mut self.fx_live_scratch);
        let mut fx_vertices = std::mem::take(&mut self.fx_composite_scratch);
        self.fx_frame_vertices_into(camera.eye, camera.target, &mut fx_live, &mut fx_vertices);
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
        // Near-field grass (Materia Świata 1b): a fixed world population cached through the
        // scene instancing path. The shader fades it continuously at 34–48 m; CPU population
        // reaches 54 m, so a four-metre planar cache step only streams invisible margin.
        let eye = glam::Vec3::from_array(camera.eye);
        let crater_fingerprint =
            crater_ledger_fingerprint(self.battlefield.heightmap.crater_records());
        let moved = self.grass_cache_eye.is_none_or(|cached| {
            let dx = cached.x - eye.x;
            let dz = cached.z - eye.z;
            dx * dx + dz * dz > GRASS_CACHE_REBUILD_M * GRASS_CACHE_REBUILD_M
        });
        if moved || crater_fingerprint != self.grass_cache_crater_fingerprint {
            self.grass_cache = scene_build::grass::grass_frame_objects(
                &self.battlefield.heightmap,
                self.battlefield.water_view(),
                &self.battlefield.static_cover,
                &self.battle_scene_meshes.as_ref().expect("ensured above").ground_maps,
                &scene_build::terrain_maps::terrain_material_set_for(self.session.map_id()),
                eye,
            );
            self.grass_cache_eye = Some(eye);
            self.grass_cache_crater_fingerprint = crater_fingerprint;
        }
        // Battlefield oaks ride the same instancing path, but they are rebuilt EVERY frame:
        // there are ten of them and the rung a tree draws depends on where the camera is right
        // now. They are appended to the grass allocation and trimmed off again, so the
        // per-frame scene submission still costs no allocation.
        let grass_len = self.grass_cache.len();
        self.grass_cache.extend(scene_build::tree_lod::tree_frame_objects(
            &self.battlefield.scenery,
            &self.battlefield.static_cover,
            self.live_cover.phase_bytes(),
            eye,
            &mut self.tree_lod_state,
        ));
        // `set_render_frame` reads the objects only synchronously. Move the allocation into
        // the temporary frame and recover it afterwards instead of cloning hundreds of KiB on
        // every presented frame.
        let mut grass_frame = RenderFrame {
            objects: std::mem::take(&mut self.grass_cache),
            ..RenderFrame::default()
        };
        renderer.set_render_frame(&grass_frame);
        self.grass_cache = std::mem::take(&mut grass_frame.objects);
        self.grass_cache.truncate(grass_len);
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
        // Recover the FX scratch buffers (drained/consumed above) so next frame reuses their
        // capacity instead of allocating fresh.
        self.fx_live_scratch = std::mem::take(&mut fx_live);
        self.fx_composite_scratch = std::mem::take(&mut fx_vertices);
    }
}

/// Order-sensitive fingerprint of the replicated crater ledger. Length alone is insufficient:
/// re-shelling can widen/deepen an existing record, and the capped ledger replaces its oldest
/// record without changing length.
fn crater_ledger_fingerprint(records: &[terrain::CraterRecord]) -> u64 {
    records.iter().enumerate().fold(0, |state, (index, record)| {
        let packed = u64::from(record.x_q)
            | (u64::from(record.z_q) << 16)
            | (u64::from(record.radius_q) << 32)
            | (u64::from(record.depth_q) << 40)
            | (u64::from(record.kind) << 48);
        game_core::math::splitmix64(state ^ packed ^ (index as u64).wrapping_mul(0x9E37_79B9))
    })
}

/// Advance the loaded-ring clock: the beat starts the frame the reload crosses to ready, ages
/// per presented frame, and expires after its TTL. Battle start (reload already at zero) never
/// fires it — only a real reload finishing does.
fn tick_ready_ring(
    age: Option<f32>,
    prev_remaining_s: f32,
    remaining_s: f32,
    dt: f32,
) -> Option<f32> {
    if prev_remaining_s > 0.0 && remaining_s <= 0.0 {
        return Some(0.0);
    }
    age.map(|a| a + dt).filter(|a| *a < crate::hud::reticle_marks::READY_RING_TTL_S)
}

#[cfg(test)]
mod ready_ring_tests {
    use super::{crater_ledger_fingerprint, tick_ready_ring};

    #[test]
    fn the_loaded_ring_fires_on_the_ready_crossing_ages_and_expires() {
        // Battle start: reload has always been ready — no beat.
        assert_eq!(tick_ready_ring(None, 0.0, 0.0, 0.016), None);
        // Mid-reload: still none.
        assert_eq!(tick_ready_ring(None, 3.2, 3.0, 0.016), None);
        // The crossing frame starts the beat.
        assert_eq!(tick_ready_ring(None, 0.1, 0.0, 0.016), Some(0.0));
        // Frames age it...
        assert_eq!(tick_ready_ring(Some(0.0), 0.0, 0.0, 0.1), Some(0.1));
        // ...and past the TTL it expires.
        let expired =
            tick_ready_ring(Some(crate::hud::reticle_marks::READY_RING_TTL_S), 0.0, 0.0, 0.016);
        assert_eq!(expired, None);
    }

    #[test]
    fn crater_fingerprint_changes_when_same_length_ledger_changes() {
        let first = terrain::CraterRecord::from_world(
            10.0,
            20.0,
            2.0,
            0.5,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        let deeper = terrain::CraterRecord::from_world(
            10.0,
            20.0,
            2.0,
            0.8,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        assert_ne!(crater_ledger_fingerprint(&[first]), crater_ledger_fingerprint(&[deeper]));
        assert_ne!(
            crater_ledger_fingerprint(&[first, deeper]),
            crater_ledger_fingerprint(&[deeper, first]),
            "ledger eviction/reordering must invalidate the cache"
        );
    }
}
