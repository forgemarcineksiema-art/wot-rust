//! The one path that renders a canonical review view (`docs/art-direction-policy.md`).
//!
//! The policy promises that "the frame a human reviews is exactly the frame the harness locks".
//! That used to be a convention: the `prokhorovka_views` example and the `look_goldens` test each
//! hand-rolled the same ~50 lines of scene setup. They drifted, and **both** stopped binding the
//! foliage atlas — so every imported tree in the committed goldens rendered as untextured white
//! against a 1x1 default, and nobody could see it because the reviewed frame carried the same
//! bug. Conventions rot; a shared function does not.
//!
//! Everything a review frame needs is therefore assembled here, once. A caller supplies the map,
//! the views and the frame size — nothing else, so nothing else can drift.

use game_core::{TankId, TeamId};
use net::TankSnapshot;
use renderer_api::{Camera, CameraProjectionPolicy, RenderFrame, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use scene_build::review_views::{HangarReviewView, ReviewVehicle, ReviewView};
use terrain::MapId;

/// Anything that can go wrong while standing a review frame up.
pub type LookHarnessError = Box<dyn std::error::Error>;

/// The scene clock every review frame is rendered on. Fixed, because the cloud sheet and the
/// grass sway both crawl on it: a wall-clock here would make the goldens unlockable.
const REVIEW_SCENE_TIME_S: f32 = 12.0;

/// The vertical FOV the review camera reads the world through.
const REVIEW_FOV_DEGREES: f32 = 55.0;

/// Render `views` on `map` at `width` x `height`, returning one RGBA8 buffer per view in order.
///
/// The whole world is built for the map first (ground, statics, water, the card meadow, the
/// foliage atlas), then each view swaps in its own lighting, sky, shadow focus and grass scatter.
/// The render is a pure function of scene + profile + the fixed clock, which is what lets the
/// goldens compare byte-exactly on one machine.
pub fn render_review_views(
    map: MapId,
    views: &[ReviewView],
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>, LookHarnessError> {
    let battlefield = map_forge::battlefield(map);
    let materials = crate::terrain_material_set_for(map);
    let ((ground_vertices, ground_indices), (statics_vertices, statics_indices)) =
        crate::battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = crate::bake_terrain_ground_maps(&battlefield);
    let (water_vertices, water_indices) = crate::battlefield_water_mesh(&battlefield);
    let (dressing_vertices, dressing_indices) =
        crate::grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_vertices, &statics_indices)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_vertices,
        &ground_indices,
        &ground_maps,
        &materials,
    );
    renderer.set_water(&ctx, &water_vertices, &water_indices);
    renderer.set_dressing(&ctx, &dressing_vertices, &dressing_indices);
    // The lane the drift cost us: imported flora samples this atlas, and without it every leaf
    // and trunk reads as the 1x1 white default. The live client has always bound it
    // (`app::render`); the review path had not.
    renderer.set_foliage_atlas(&ctx, &scene_build::flora_pack::flora_catalog().atlas_mips);
    renderer.scene_time_s = REVIEW_SCENE_TIME_S;
    renderer.register_mesh(&ctx, crate::GRASS_MESH_HANDLE, &crate::grass_tuft_mesh());

    let mut catalog = crate::VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!(
            "note: no Forge artifacts loaded ({error}); review vehicles use the neutral material"
        );
    }

    let projection = CameraProjectionPolicy::webgpu_default();
    let mut frames = Vec::with_capacity(views.len());
    for view in views {
        // The vehicle goes through the SAME entry battle and the garage use, so a review frame
        // cannot flatter the hero with a path the game does not ship.
        let vehicle_objects = view.vehicle.map(|vehicle| {
            crate::tank_vehicle_render_objects(
                &mut catalog,
                &review_snapshot(&vehicle),
                vehicle.hull_color,
            )
        });
        for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(&ctx, handle, &mesh);
        }
        for (handle, maps) in catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(&ctx, handle, &maps);
        }
        renderer.set_vehicle_render_frame(
            &ctx,
            &crate::render_frame_from_objects(vehicle_objects.unwrap_or_default()),
        );

        let grass = crate::grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &ground_maps,
            &materials,
            glam::Vec3::from_array(view.eye),
        );
        renderer.set_render_frame(&ctx, &RenderFrame { objects: grass, ..RenderFrame::default() });
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
        // A view with a subject focuses the near shadow cascade on the SUBJECT, not on the
        // camera's aim point: the contact shadow under the hull is the whole reason the frame
        // exists, and off-centre it falls outside the crisp box.
        renderer.shadow_focus = Some(view.vehicle.map_or(view.target, |v| v.position));

        let camera =
            Camera { eye: view.eye, target: view.target, vertical_fov_degrees: REVIEW_FOV_DEGREES };
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        frames.push(target.read_rgba8(&ctx)?);
    }
    Ok(frames)
}

/// Render the garage review views. Separate from the battlefield path on purpose: the hangar has
/// no terrain, no water, no grass, no sky dome and no fog — it is a lit interior with one
/// subject, and pretending otherwise would mean a review that quietly exercises passes the
/// garage never runs.
pub fn render_hangar_review_views(
    views: &[HangarReviewView],
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>, LookHarnessError> {
    let (hangar_vertices, hangar_indices) = scene_build::hangar::hangar_scene_mesh();

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    // The hangar shell rides the statics slot, exactly as `garage_render::ensure_scene` uploads
    // it — same buffer, same shader, same lighting path as the live garage.
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &hangar_vertices, &hangar_indices)?;
    renderer.scene_time_s = REVIEW_SCENE_TIME_S;
    // The orbit camera sweeps a full circle, so the battle path's forward-offset shadow heuristic
    // would walk the boxes off the subject. Pin them to the turntable, as the garage does.
    renderer.shadow_focus = Some(scene_build::hangar::hangar_shadow_focus());

    let mut catalog = crate::VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!(
            "note: no Forge artifacts loaded ({error}); review vehicles use the neutral material"
        );
    }

    // The HUD atlas is uploaded once, unconditionally: an overlay view that silently rendered
    // with no font bound would lock a screen full of blank quads and call it a review.
    let (font_w, font_h, font_coverage) = crate::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);

    let projection = CameraProjectionPolicy::webgpu_default();
    let mut frames = Vec::with_capacity(views.len());
    for view in views {
        // The overlay is the REAL garage overlay — `garage_overlay` builds it from a default
        // `GarageState`, the same call the live client makes — not a review-only reconstruction.
        let hud = if view.overlay {
            crate::garage_overlay(false, width as f32 / height as f32)
        } else {
            Vec::new()
        };
        renderer.set_hud(&ctx, &hud);

        let objects = crate::tank_vehicle_render_objects(
            &mut catalog,
            &review_snapshot(&view.vehicle),
            view.vehicle.hull_color,
        );
        for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(&ctx, handle, &mesh);
        }
        for (handle, maps) in catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(&ctx, handle, &maps);
        }
        renderer.set_vehicle_render_frame(&ctx, &crate::render_frame_from_objects(objects));
        renderer.scene_lighting = view.lighting;
        // Interior: the gradient-sky pass is off and a flat clear colour stands behind the room.
        renderer.set_interior_background(view.background.0, view.background.1, view.background.2);

        let camera = Camera {
            eye: view.eye,
            target: view.target,
            vertical_fov_degrees: scene_build::hangar::HERO_FOV_DEGREES,
        };
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        frames.push(target.read_rgba8(&ctx)?);
    }
    Ok(frames)
}

/// Turn a review view's picture-level vehicle description into the snapshot the render path
/// expects. Only the fields the mesh kernels read carry meaning: the tank is undamaged, fully
/// loaded and standing still, because a review frame judges the LOOK, not a battle state.
fn review_snapshot(vehicle: &ReviewVehicle) -> TankSnapshot {
    let spec = vehicle.kind.spec();
    TankSnapshot {
        tank_id: TankId(0),
        team: TeamId(1),
        vehicle: vehicle.kind,
        position: vehicle.position,
        yaw_rad: vehicle.yaw_rad,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: vehicle.turret_yaw_rad,
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
    }
}
