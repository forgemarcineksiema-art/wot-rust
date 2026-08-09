//! The garage frame: the selected vehicle parked in the hangar under the orbit camera, plus the
//! scene-geometry swap between the garage hangar and the battlefield, and the player's barrel
//! silhouette scale shared by the rendered gun and the reticle muzzle.

use std::time::Instant;

use game_core::{TankId, TeamId, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use renderer_api::{CameraProjectionPolicy, RenderFrame, SceneLighting, view_projection_matrix};
use renderer_wgpu::WindowRenderer;
use tracing::error;

use super::{ClientApp, SceneKind};
use crate::render_frame_from_objects;
use crate::vehicle::asset_render::tank_vehicle_render_objects_with_tracks;
use crate::vehicle::variation::VehicleVariation;

impl ClientApp {
    /// Render the static garage hangar: the selected vehicle parked on the turntable under an
    /// orbit camera, with the garage UI overlay. Replaces the battle scene while the garage is open.
    pub(super) fn render_garage(&mut self) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_render_time).as_secs_f32().min(0.1);
        self.last_render_time = now;
        // Apply orbit drag (and clear the cursor delta) — the battle path does this per frame, and
        // the garage needs it too or the inspection camera never moves. The camera tick then runs
        // the focus/return spring and the idle auto-orbit.
        self.apply_mouse_look();
        self.garage.tick_camera(dt);
        self.garage.tick_drive_in(dt);
        // The hangar has ears too: UI clicks flush here, the engine bed idles down, the wind
        // drops to a sheltered breath. No listener update — the orbit camera is not a battle ear.
        self.flush_audio(None, None);
        // Keep the speculative map bake tracking the pick while the player is still in here —
        // this frame's spare cores are the ones the Battle press will not have to spend.
        self.poll_map_prebake();
        self.ensure_scene(SceneKind::Garage);
        let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, WindowRenderer::aspect_ratio);
        let camera = self.garage.orbit_camera();
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );

        // While a vehicle switch rolls the new tank in from the doorway, animate its pose down
        // the entry lane — the hull FACES the lane while it travels and pivot-turns to the hero
        // pose on the mark (see `drive_in.rs`) — and scroll each track with its own travelled
        // distance (the pivot runs them in opposite directions). Once parked: hero pose, still.
        let pose = self.garage.drive_in_pose();
        let mut snapshot = garage_preview_snapshot(self.garage.selected_vehicle());
        snapshot.position[2] = pose.z;
        snapshot.yaw_rad = pose.yaw_rad;
        let variation = VehicleVariation::from_snapshot(&snapshot);
        let mut objects = tank_vehicle_render_objects_with_tracks(
            &mut self.vehicle_asset_catalog,
            &snapshot,
            [0.72, 0.76, 0.62],
            &variation,
            pose.track_left_m,
            pose.track_right_m,
        );
        // Stretch the gun submesh (objects: [hull, turret, gun]) along its barrel axis so swapping
        // to a longer/shorter gun visibly changes the silhouette. Local +Z is the barrel direction.
        let barrel_scale = self.garage.gun_silhouette_scale();
        if (barrel_scale - 1.0).abs() > 1.0e-3
            && let Some(gun) = objects.get_mut(crate::VEHICLE_GUN_OBJECT)
        {
            let scaled = glam::Mat4::from_cols_array_2d(&gun.transform)
                * glam::Mat4::from_scale(glam::Vec3::new(1.0, 1.0, barrel_scale));
            gun.transform = scaled.to_cols_array_2d();
        }
        let render_frame = render_frame_from_objects(objects);
        let hud = self.garage.overlay_vertices(aspect);

        // Dust streams off the tracks while the tank rolls in; the pool ages out once it parks.
        if self.garage.poll_drive_dust() {
            self.fx.track_dust(Vec3::new(0.0, 0.0, pose.z - 3.0));
        }
        // The air of the hall (E1): a slow trickle of dust motes inside the sun shafts, and
        // the shaft blades themselves appended as static soft-additive quads. Both read from
        // the hangar's own blade geometry, so the light, its dust and its beams agree.
        let blades = scene_build::hangar::sun_shaft_quads();
        self.fx.hangar_motes(&blades, dt);
        // ...and the hall breathes (E2): steam off the heating duct by the stores, drifting
        // toward the gate with the draft the exhaust fan maintains.
        self.fx.vent_steam(Vec3::from_array(scene_build::hangar::STEAM_DUCT_OUTLET), dt);
        self.fx.tick(dt);
        let mut fx_vertices =
            self.fx.vertices(Vec3::from_array(camera.eye), Vec3::from_array(camera.target));
        fx_vertices.extend(crate::fx::FxSystem::hangar_shaft_vertices(&blades));

        let scene_time_s = self.presented_time_s();
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
        renderer.set_vehicle_render_frame(&render_frame);
        // The exhaust fan turns (E2): its blades are the dynamic mesh the garage used to
        // clear — rebuilt per frame at the tick-domain angle, ~180 vertices of motion.
        let (fan_v, fan_i) = crate::look_harness::hangar_fan_mesh_at(scene_time_s);
        renderer.set_dynamic_mesh(&fan_v, &fan_i);
        renderer.set_fx(&fx_vertices);
        renderer.set_hud(&hud);
        renderer.set_scene_time_s(scene_time_s);
        // The bench tube flickers (E2): the rig re-evaluated per presented frame on the same
        // clock everything else animates on. Deterministic — at the golden harness's frozen
        // second the factor is 1.0 and this line changes nothing.
        renderer.set_scene_lighting(SceneLighting::garage_hero_at(scene_time_s));
        if let Err(error) = renderer.render(view_proj, camera.eye) {
            error!(%error, "garage frame render failed");
        }
    }

    /// Swap the renderer's static geometry to the requested scene if it differs. Only fires on
    /// a garage <-> battle transition, not per frame — and the battle meshes come from the
    /// app-lifetime cache, so the swap costs the GPU upload alone (rebaking the full 1000 m
    /// battlefield inside the transition frame froze it for hundreds of ms on iGPU laptops).
    pub(super) fn ensure_scene(&mut self, want: SceneKind) {
        // `scene_upload_dirty` is the second reason to upload: the scene KIND did not change but
        // the world behind it did (a garage map pick rebuilt the battlefield), so the slots hold
        // the previous map and the identity check alone would keep it on screen forever.
        if self.current_scene == want && !self.scene_upload_dirty {
            return;
        }
        if want == SceneKind::Battle {
            self.ensure_battle_scene_meshes();
        }
        // The hangar mesh stays cheap enough to bake on entry; it must outlive the borrow below.
        let hangar_meshes;
        // The river surface swaps with the scene: the battlefield's water (empty on dry maps),
        // nothing in the hangar.
        let (vertices, indices, water_vertices, water_indices): (&[_], &[u32], &[_], &[u32]) =
            match want {
                SceneKind::Garage => {
                    hangar_meshes = scene_build::hangar::hangar_scene_mesh();
                    (&hangar_meshes.0, &hangar_meshes.1, &[], &[])
                }
                SceneKind::Battle => {
                    let meshes = self.battle_scene_meshes.as_ref().expect("ensured above");
                    (
                        &meshes.statics_vertices,
                        &meshes.statics_indices,
                        &meshes.water_vertices,
                        &meshes.water_indices,
                    )
                }
            };
        let (sky, lighting, rain_intensity, wetness) = match want {
            // The workshop rig rakes a warm sun down through the skylights (real contact shadow
            // on the turntable) over a dim, near-neutral shop interior. What shows through a
            // roof opening is the day outside — single-sourced, so the review goldens and the
            // probes cannot drift onto a different sky than the one the player sees.
            SceneKind::Garage => {
                (scene_build::hangar::INTERIOR_BACKGROUND, SceneLighting::garage_hero(), 0.0, 0.0)
            }
            SceneKind::Battle => {
                let frame = self.weather_frame;
                (frame.sky, frame.lighting, frame.rain_intensity, frame.surface_wetness)
            }
        };
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_terrain(vertices, indices);
            // The splat ground rides only the battle scene; the hangar floor is its own mesh.
            match want {
                SceneKind::Garage => {
                    renderer.clear_battlefield_ground();
                    // The hangar has no meadow: clear the dressing slot with the ground.
                    renderer.set_dressing(&[], &[]);
                    self.dressing_uploaded_fingerprint =
                        renderer_api::scene_mesh_fingerprint(&[], &[]);
                }
                SceneKind::Battle => {
                    let meshes = self.battle_scene_meshes.as_ref().expect("ensured above");
                    renderer.set_battlefield_ground(
                        &meshes.ground_vertices,
                        &meshes.ground_indices,
                        &meshes.ground_maps,
                        &scene_build::terrain_maps::terrain_material_set_for(self.session.map_id()),
                    );
                    renderer.set_dressing(&meshes.dressing_vertices, &meshes.dressing_indices);
                    self.dressing_uploaded_fingerprint = renderer_api::scene_mesh_fingerprint(
                        &meshes.dressing_vertices,
                        &meshes.dressing_indices,
                    );
                }
            }
            renderer.set_water(water_vertices, water_indices);
            // Interior scenes show a flat backdrop; the battlefield shows the gradient sky dome.
            match want {
                SceneKind::Garage => renderer.set_interior_background(sky.0, sky.1, sky.2),
                SceneKind::Battle => renderer.set_outdoor_sky(sky.0, sky.1, sky.2),
            }
            renderer.set_scene_lighting(lighting);
            renderer.set_rain_intensity(rain_intensity);
            renderer.set_wetness(wetness);
            if want == SceneKind::Battle {
                let frame = self.weather_frame;
                renderer.set_weather_dynamics(
                    frame.puddle_fill,
                    frame.cloud_offset,
                    frame.rain_phase_s,
                );
            } else {
                renderer.set_weather_dynamics(0.0, [0.0; 2], 0.0);
            }
            // The garage pins the shadow boxes to the turntable (the orbit camera's "forward"
            // swings a full circle); battle restores the forward chase-camera heuristic. Both
            // arms live here so neither scene inherits the other's focus.
            match want {
                SceneKind::Garage => {
                    renderer.set_shadow_focus(Some(scene_build::hangar::hangar_shadow_focus()));
                    // ...and SIZE them to the room. Pinning only the centre left a 36 m hall
                    // inside the battlefield's 128 m box, staircasing every skylight shaft.
                    renderer.set_shadow_focus_radius_m(Some(
                        scene_build::hangar::hangar_shadow_radius_m(),
                    ));
                    // ...and then DROP the far cascade, because the box above already holds the
                    // whole hall — that is what `the_near_shadow_box_contains_the_whole_hall`
                    // asserts, corner by corner, under every garage rig. The far cascade the
                    // garage was still encoding spanned 270 m at 264 mm per texel around a 36 m
                    // room: the hall's 13 374 triangles redrawn into a second depth map every
                    // frame, for a map no fragment in this room can sample.
                    renderer.set_shadow_cascades(Some(1));
                    // The garage renders "a bit richer" (Hala 2.0, user decision 2026-08-04):
                    // a modest bloom chain so the frosted panes and lamp faces genuinely glow.
                    // The battlefield keeps its tier default — no fairness surface is touched.
                    renderer.set_bloom_mips(scene_build::hangar::hangar_bloom_mips());
                    // The hero probe (Hala 3.0 B2): the hall's baked irradiance cube, so the
                    // parked vehicle receives the room's bounce the way the room itself does.
                    renderer.set_hero_probe(Some(scene_build::hangar::hangar_hero_probe()));
                    // ...and the hall's material grain catches the worklight (C1).
                    renderer.set_interior_detail_normal(true);
                    // ...and every polished surface mirrors the ROOM (D1): the baked,
                    // prefiltered reflection cube from the hero station.
                    renderer.set_environment_cube(Some(
                        &scene_build::hangar::hangar_reflection_cube().mips,
                    ));
                }
                SceneKind::Battle => {
                    renderer.set_shadow_focus(None);
                    renderer.set_shadow_focus_radius_m(None);
                    // The battlefield is 1000 m and the near box is 64: it needs both cascades.
                    renderer.set_shadow_cascades(None);
                    let tier = renderer.default_bloom_mips();
                    renderer.set_bloom_mips(tier);
                    // No hall, no probe: zero lanes, a bit-exact no-op in the vehicle shader.
                    renderer.set_hero_probe(None);
                    renderer.set_interior_detail_normal(false);
                    renderer.set_environment_cube(None);
                }
            }
            self.current_scene = want;
            self.scene_upload_dirty = false;
        }
    }

    /// Dress the freshly created renderer in the CURRENT battle's weather (the renderer is
    /// born holding the generic battlefield defaults; the app is born in battle).
    pub(super) fn apply_match_weather(&mut self) {
        let look = self.weather_frame;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_outdoor_sky(look.sky.0, look.sky.1, look.sky.2);
            renderer.set_scene_lighting(look.lighting);
            renderer.set_rain_intensity(look.rain_intensity);
            renderer.set_wetness(look.surface_wetness);
            renderer.set_weather_dynamics(look.puddle_fill, look.cloud_offset, look.rain_phase_s);
        }
    }

    /// Barrel-length scale of the player's installed gun vs the vehicle's stock gun. The predictor
    /// holds the real (custom) loadout — the snapshot-derived `player_spec` is stock by kind — so
    /// the rendered gun and the reticle muzzle both track the gun the player actually fitted.
    pub(super) fn player_barrel_scale(&self) -> f32 {
        let spec = self.predictor.spec();
        let stock = spec.kind.stock_barrel_length_m();
        if stock <= 0.0 { 1.0 } else { (spec.gun.barrel_length_m / stock).clamp(0.6, 1.6) }
    }
}

/// A pose-only snapshot of the selected vehicle parked on the garage turntable, angled three-
/// quarters to the camera. Only the fields the mesh kernels read are meaningful.
fn garage_preview_snapshot(kind: VehicleKind) -> TankSnapshot {
    let spec = kind.spec_ref();
    TankSnapshot {
        tank_id: TankId(0),
        team: TeamId(1),
        vehicle: kind,
        position: [0.0, scene_build::hangar::TURNTABLE_TOP_M, 0.0],
        yaw_rad: scene_build::hangar::HERO_PARK_YAW,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
    }
}
