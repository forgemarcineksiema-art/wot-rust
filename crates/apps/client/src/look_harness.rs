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

/// The hangar's sun-shaft blades as FX vertices — the ONE public wrapper the review probes
/// and this harness share, so a probe frame and a golden frame hang the same beams
/// (`fx::FxSystem::hangar_shaft_vertices` is crate-private).
pub fn hangar_shaft_fx_vertices() -> Vec<renderer_api::FxVertex> {
    hangar_shaft_fx_vertices_for(scene_build::hangar::HangarLight::Day)
}

/// [`hangar_shaft_fx_vertices`] under a chosen daylight (H1) — morning renders none.
pub fn hangar_shaft_fx_vertices_for(
    light: scene_build::hangar::HangarLight,
) -> Vec<renderer_api::FxVertex> {
    crate::fx::FxSystem::hangar_shaft_vertices(&scene_build::hangar::sun_shaft_quads_for(light))
}

/// How fast the exhaust fan turns (E2), radians per presented second — an unhurried
/// extraction fan, not a propeller.
const FAN_SPEED_RAD_S: f32 = 2.4;

/// The hangar's MOVING geometry at a moment on the presentation clock: the exhaust fan's
/// blades plus the bay gate's slat curtain at `gate_open_m` (E3) — the one builder the live
/// garage, this harness and the probes share, so the locked frame holds the exact blade
/// angle and curtain position the game would show at the frozen review second.
pub fn hangar_dynamic_mesh_at(
    seconds: f32,
    gate_open_m: f32,
) -> (Vec<renderer_api::SceneVertex>, Vec<u32>) {
    hangar_dynamic_mesh_worked(seconds, gate_open_m, seconds, None)
}

/// [`hangar_dynamic_mesh_at`] with the mechanic's OWN clock and an optional repair-work cue
/// (Hala v4 R3). The garage pauses the round clock for the length of each repair beat and
/// hands the cue over, so the mechanic answers the wrench; the goldens and the probes pass
/// the plain clock and no cue — bit-for-bit the old mesh.
pub fn hangar_dynamic_mesh_worked(
    seconds: f32,
    gate_open_m: f32,
    mechanic_seconds: f32,
    work: Option<&scene_build::hangar_mechanic::WorkCue>,
) -> (Vec<renderer_api::SceneVertex>, Vec<u32>) {
    let (mut v, mut i) = scene_build::hangar::wall_fan_blades(seconds * FAN_SPEED_RAD_S);
    for (part_v, part_i) in [
        scene_build::hangar::bay_gate_slats(gate_open_m),
        // The crane trolley rides its girder (K1) — somebody works this hall.
        scene_build::hangar::crane_trolley_at(seconds),
        // ...and here he is (K2, behind its kill-switch): the mechanic on his round
        // between the welding bay and the workbench, never inside the hero's ring —
        // stepping toward its EDGE while the repair beat runs (R3).
        scene_build::hangar_mechanic::mechanic_working_at(mechanic_seconds, work),
    ] {
        let base = v.len() as u32;
        v.extend(part_v);
        i.extend(part_i.iter().map(|idx| idx + base));
    }
    (v, i)
}

/// The welding arc's glow behind the second bay's screen (K1): a couple of cool additive
/// quads flickering on the arc's own deterministic duty cycle — EMPTY in the quiet half,
/// which includes the goldens' frozen review second. The spark fountain itself is a live
/// random emitter (`FxSystem::welding_sparks`), same exemption as the motes.
pub fn welding_glow_vertices(seconds: f32) -> Vec<renderer_api::FxVertex> {
    if !scene_build::hangar::welding_burn_at(seconds) {
        return Vec::new();
    }
    let [wx, _, wz] = scene_build::hangar::WELDING_CORNER;
    // Arc flicker: fast, jagged, deterministic on the clock. Peak amplitude sits at the
    // sun-shaft GLOW scale (~0.13) — a working light in the room's key, never a floodlight.
    let flicker =
        0.55 + 0.30 * (seconds * 57.0).sin().abs() + 0.15 * (seconds * 173.0 + 1.7).sin().abs();
    let glow = |c: f32| c * flicker;
    let color = [glow(0.085), glow(0.10), glow(0.145), 0.0];
    let mut vertices = Vec::with_capacity(12);
    let mut quad = |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| {
        for corner in [a, b, c, a, c, d] {
            vertices.push(renderer_api::FxVertex::sharp(corner, [0.0, 0.0], 1.0, color));
        }
    };
    // A modest spill over the screen's top edge and a faint pool at the arc's feet — the
    // screen (at wx+0.9) keeps the arc itself hidden from the turntable side.
    quad(
        [wx + 0.7, 1.75, wz - 1.2],
        [wx + 0.7, 1.75, wz + 0.6],
        [wx + 0.3, 2.45, wz + 0.6],
        [wx + 0.3, 2.45, wz - 1.2],
    );
    quad(
        [wx - 0.9, 0.05, wz - 1.0],
        [wx + 0.5, 0.05, wz - 1.0],
        [wx + 0.5, 0.05, wz + 0.6],
        [wx - 0.9, 0.05, wz + 0.6],
    );
    vertices
}

/// The vertical FOV the review camera reads the world through. Kept at 55° on purpose: the
/// look goldens predate the Świat 2.0 battle lens (48°) and stay a stable instrument — the
/// battle FOV verdict renders through the `fov_probe` before/after pair instead.
const REVIEW_FOV_DEGREES: f32 = scene_build::review_views::CHASE_REVIEW_FOV_DEGREES;

/// Render `views` on `map` at `width` x `height`, returning one RGBA8 buffer per view in order.
///
/// The whole world is built for the map first (ground, statics, water, the card meadow, the
/// foliage atlas), then each view swaps in its own lighting, sky, shadow focus and grass scatter.
/// Register every instanced mesh the battlefield's per-frame dressing can ask for — the grass
/// species and the tree ladder's rungs — on a renderer, exactly as the battle registers them at
/// deployment. One function for the goldens, the frame instrument and the view probes.
pub fn register_battlefield_dressing_meshes(ctx: &GpuContext, renderer: &mut SceneRenderer) {
    for (handle, mesh) in crate::grass_species_meshes() {
        renderer.register_mesh(ctx, handle, &mesh);
    }
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        renderer.register_mesh(ctx, handle, &mesh);
    }
}

/// The instanced dressing the battle submits around an eye, in ONE call: the near grass ring
/// AND the tree ladder's rung per tree for this eye. The battle path appends the trees to its
/// grass cache every frame (`app::render`); a review or a measurement that submitted the grass
/// alone drew a map with no oaks on it — which is exactly what every battlefield golden and
/// every `perf_capture` "full scene" number did until this function existed (the fifth
/// instrument-fidelity defect, after MSAA, the missing adapter, the fleet on the scenery path
/// and the garage block measuring the old room). `cover_states` are the sim's phase bytes;
/// an empty slice is "everything intact", the same reading the statics bake gives it.
pub fn battlefield_dressing_objects(
    battlefield: &terrain::BattlefieldMap,
    ground_maps: &renderer_api::TerrainGroundMaps,
    materials: &renderer_api::TerrainMaterialSet,
    cover_states: &[u8],
    eye: scene_build::tree_lod::TreeEye,
    tree_lod_state: &mut scene_build::tree_lod::TreeLodState,
) -> Vec<renderer_api::RenderObject> {
    let mut objects = crate::grass_frame_objects(
        &battlefield.heightmap,
        battlefield.water_view(),
        &battlefield.static_cover,
        ground_maps,
        materials,
        eye.position,
    );
    objects.extend(scene_build::tree_lod::tree_frame_objects_with_backdrop(
        battlefield,
        cover_states,
        eye,
        tree_lod_state,
    ));
    objects
}

/// The render is a pure function of scene + profile + the fixed clock, which is what lets the
/// goldens compare byte-exactly on one machine.
pub fn render_review_views(
    map: MapId,
    views: &[ReviewView],
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>, LookHarnessError> {
    render_review_views_with_fov(map, views, width, height, REVIEW_FOV_DEGREES)
}

/// The same render with an explicit lens. The goldens never call this — it exists for the
/// Świat 2.0 FOV before/after probes, where the whole point is a frame the goldens do NOT lock.
pub fn render_review_views_with_fov(
    map: MapId,
    views: &[ReviewView],
    width: u32,
    height: u32,
    vertical_fov_degrees: f32,
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
    renderer.scene_time_s = REVIEW_SCENE_TIME_S;
    register_battlefield_dressing_meshes(&ctx, &mut renderer);
    // The procedural leaf atlas rides the SAME entry the battle uses (Drzewa 3.0 PR5) — the
    // harness header documents how the review path once lost the atlas bind and locked white
    // trees; wiring it here keeps the goldens honest about what the player sees.
    let (foliage_color, foliage_normal) = scene_build::foliage_atlas_paint::foliage_atlas_chains();
    renderer.set_foliage_atlas(&ctx, &foliage_color, Some(&foliage_normal));
    renderer.set_bark_textures(&ctx, &scene_build::foliage_atlas_paint::bark_texture_layers());

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

        // The grass ring AND the tree ladder, as the battle submits them. A fresh LOD state
        // per view: a review frame has no previous frame, so every tree takes the plain band
        // for its distance — deterministic, which is what the goldens need.
        let mut tree_lod_state = scene_build::tree_lod::TreeLodState::default();
        let dressing = battlefield_dressing_objects(
            &battlefield,
            &ground_maps,
            &materials,
            &[],
            scene_build::tree_lod::TreeEye::at(glam::Vec3::from_array(view.eye)),
            &mut tree_lod_state,
        );
        renderer
            .set_render_frame(&ctx, &RenderFrame { objects: dressing, ..RenderFrame::default() });
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
        // A view with a subject focuses the near shadow cascade on the SUBJECT, not on the
        // camera's aim point: the contact shadow under the hull is the whole reason the frame
        // exists, and off-centre it falls outside the crisp box.
        renderer.shadow_focus = Some(view.vehicle.map_or(view.target, |v| v.position));

        let camera = Camera {
            eye: view.eye,
            target: view.target,
            // A view may bring its own lens (the sniper review frame, A7).
            vertical_fov_degrees: view.vertical_fov_degrees.unwrap_or(vertical_fov_degrees),
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

/// Everything scene-level the SHIPPED garage sets, applied to an offscreen renderer in one
/// place — the golden path and the perf probe both call THIS, so the locked picture, the
/// measured frame and the played frame are one room.
///
/// It exists because the instrument drifted: #544 (reflection cube), #545 (sun shafts) and
/// #554 (penumbra + caster cut) each changed the shipped garage, the golden path followed,
/// and `perf_capture`'s garage block kept measuring the old room — the fourth instrument-
/// fidelity defect in this project's history (after MSAA, the missing adapter and the fleet
/// taking the scenery path). A shared function does not rot; parallel setup blocks do.
///
/// Scene-LEVEL only: the caller still parks its own vehicle, HUD and camera. The lighting
/// and background applied here are the shipped Day baseline; the golden path overrides them
/// per view (that override is the views' own single-source, untouched).
pub fn apply_shipped_garage_scene(ctx: &GpuContext, renderer: &mut SceneRenderer) {
    renderer.scene_time_s = REVIEW_SCENE_TIME_S;
    // The orbit camera sweeps a full circle, so the battle path's forward-offset shadow heuristic
    // would walk the boxes off the subject. Pin them to the turntable AND size them to the room,
    // as the garage does — a review artifact shows what the game shows. Same for the garage's
    // richer bloom chain (Hala 2.0 T1): the panes' glow is part of the locked picture.
    renderer.shadow_focus = Some(scene_build::hangar::hangar_shadow_focus());
    renderer.shadow_focus_radius_m = Some(scene_build::hangar::hangar_shadow_radius_m());
    // Światło służy czołgowi: the same penumbra and the same reduced caster set the live
    // garage runs — the locked picture is the played picture.
    renderer.set_shadow_softness(scene_build::hangar::HANGAR_SHADOW_SOFTNESS);
    renderer.set_terrain_shadow_indices(ctx, Some(&scene_build::hangar::hangar_shadow_indices()));
    // And the single cascade the live garage runs: the near box holds the whole hall, so the
    // far one draws a map nothing samples. A review artifact shows what the game shows.
    renderer.shadow_cascades = Some(1);
    renderer.set_bloom_mips(scene_build::hangar::hangar_bloom_mips());
    // The hero probe (Hala 3.0 B2), the interior detail normal (C1), the reflection cube
    // (D1) and the sun shafts (E1), exactly as the live garage sets them: the locked picture
    // is the played picture. The shafts are static geometry whose drift runs on the frozen
    // review clock, so they lock; the dust MOTES are a live random trickle and stay live-only
    // (the same standing exemption the drive-in dust has always had).
    renderer.set_hero_probe(Some(scene_build::hangar::hangar_hero_probe()));
    renderer.set_interior_detail_normal(true);
    // The hero draws before the room (Hala v4 P2) — the shipped garage order, so the
    // goldens and the perf probe measure the frame the player pays for.
    renderer.set_vehicles_first(true);
    renderer.set_environment_cube(ctx, Some(&scene_build::hangar::hangar_reflection_cube().mips));
    renderer.set_fx(ctx, &hangar_shaft_fx_vertices());
    // The fan holds one exact blade angle at the frozen review clock (E2), the gate curtain
    // parks at ajar (E3), and the flicker factor is exactly 1.0 there —
    // `garage_hero_at(REVIEW_SCENE_TIME_S)` is `garage_hero()` to the bit, so the views' own
    // lighting stays the single source it always was.
    let (dyn_v, dyn_i) =
        hangar_dynamic_mesh_at(REVIEW_SCENE_TIME_S, scene_build::hangar::GATE_AJAR_M);
    renderer.set_dynamic_mesh(ctx, &dyn_v, &dyn_i);
    // The shipped Day rig and its backdrop, so a caller that sets nothing else still renders
    // the room the player sits in. H1 locks Day == `garage_hero()` to the bit.
    renderer.scene_lighting =
        scene_build::hangar::hangar_lighting(scene_build::hangar::HangarLight::Day);
    let (bg_r, bg_g, bg_b) =
        scene_build::hangar::interior_background_for(scene_build::hangar::HangarLight::Day);
    renderer.set_interior_background(bg_r, bg_g, bg_b);
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
    let (hangar_vertices, hangar_indices) = scene_build::hangar::hangar_scene_mesh_without_gate();

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    // The hangar shell rides the statics slot, exactly as `garage_render::ensure_scene` uploads
    // it — same buffer, same shader, same lighting path as the live garage. WITHOUT the gate
    // curtain (E3): the slats are dynamic geometry now, parked at ajar below.
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &hangar_vertices, &hangar_indices)?;
    apply_shipped_garage_scene(&ctx, &mut renderer);

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
        // The overlay is the REAL garage overlay — these are the same builders the live client
        // calls, driven from a default `GarageState`, not a review-only reconstruction.
        let aspect = width as f32 / height as f32;
        let hud = match view.screen {
            // The inspector view carries the legend the live screen shows with the overlay
            // (R1) — the locked frame explains its own color ramp.
            scene_build::review_views::GarageScreen::Room if view.inspector => {
                crate::garage_inspector_legend(aspect)
            }
            scene_build::review_views::GarageScreen::Room => Vec::new(),
            scene_build::review_views::GarageScreen::Hangar => crate::garage_overlay(false, aspect),
            scene_build::review_views::GarageScreen::TechTree => {
                crate::garage_overlay(true, aspect)
            }
            // The T-54's gun slot: the one slot on the opening vehicle with a real choice, so
            // the locked list has rows to draw rather than an empty plate.
            scene_build::review_views::GarageScreen::OptionList => {
                crate::garage_overlay_option_list(0, 1, aspect)
            }
        };
        renderer.set_hud(&ctx, &hud);

        // At the parked settle its mass earns (J1) — the same pose the live garage renders.
        // The BATTLEFIELD review path above keeps the neutral pose its goldens froze.
        let snapshot = review_snapshot(&view.vehicle);
        let objects = crate::vehicle::asset_render::tank_vehicle_render_objects_at_rest(
            &mut catalog,
            &snapshot,
            view.vehicle.hull_color,
            &crate::vehicle::variation::VehicleVariation::from_snapshot(&snapshot),
            0.0,
            0.0,
        );
        for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(&ctx, handle, &mesh);
        }
        for (handle, maps) in catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(&ctx, handle, &maps);
        }
        renderer.set_vehicle_render_frame(&ctx, &crate::render_frame_from_objects(objects));
        // The armor inspector view (I1): the same overlay builder the live client renders,
        // appended to the frozen shafts; every other view keeps the shafts alone.
        if view.inspector {
            let mut fx = hangar_shaft_fx_vertices();
            fx.extend(crate::vehicle::armor_overlay::armor_inspector_fx_vertices(
                view.vehicle.kind,
                glam::Vec3::from_array(view.vehicle.position),
                view.vehicle.yaw_rad,
            ));
            renderer.set_fx(&ctx, &fx);
        } else {
            renderer.set_fx(&ctx, &hangar_shaft_fx_vertices());
        }
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
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The instrument's promise: the dressing a review frame (and the frame instrument)
    /// submits carries every ladder tree the battle would draw at that eye — not the grass
    /// alone. Every battlefield golden and every "full scene" frame time recorded before this
    /// function existed described a map without its oaks.
    #[test]
    fn the_review_dressing_draws_every_ladder_tree_the_battle_draws() {
        let map = MapId::ProkhorovkaHill252_2;
        let battlefield = map_forge::battlefield(map);
        let ground_maps = crate::bake_terrain_ground_maps(&battlefield);
        let materials = crate::terrain_material_set_for(map);
        let eye = glam::Vec3::new(500.0, 8.0, 470.0);
        let mut state = scene_build::tree_lod::TreeLodState::default();
        let dressing = battlefield_dressing_objects(
            &battlefield,
            &ground_maps,
            &materials,
            &[],
            scene_build::tree_lod::TreeEye::at(eye),
            &mut state,
        );

        let ladder: Vec<renderer_api::MeshHandle> = scene_build::tree_lod::tree_lod_meshes()
            .into_iter()
            .map(|(handle, _)| handle)
            .collect();
        // A tree may be two or three objects (a cross-fade band, the impostor's two quads);
        // their windows partition [0, 1), so exactly one per tree starts at 0 — that is the
        // count of trees drawn.
        let trees_drawn = dressing
            .iter()
            .filter(|object| ladder.contains(&object.mesh) && object.dither[0] == 0.0)
            .count();
        let trees_planted = battlefield
            .scenery
            .iter()
            .filter(|instance| scene_build::tree_lod::ladder_species(instance.kind).is_some())
            .count();
        assert!(trees_planted > 0, "prokhorovka plants battlefield trees");
        // Since F11 the horizon ring rides the same ladder: every ring tree draws too.
        let ring = scene_build::backdrop::backdrop_tree_instances(&battlefield).len();
        assert_eq!(
            trees_drawn,
            trees_planted + ring,
            "every planted ladder tree and every ring tree draws, intact cover"
        );
        assert!(dressing.len() > trees_drawn, "the grass ring is still in the frame");
    }

    /// Render the scene at `distance` m from the tree (the oak probe's eye rule) twice — with
    /// the tree and without it — and read the TREE's pixels as the difference: the crown
    /// (green-dominant) mean colour and count, and the wood (the rest) count. A colour mask
    /// alone read the grass under the tree as crown.
    fn tree_reading(
        ctx: &renderer_wgpu::GpuContext,
        renderer: &mut renderer_wgpu::SceneRenderer,
        target: &renderer_wgpu::OffscreenTarget,
        battlefield: &terrain::BattlefieldMap,
        distance: f32,
    ) -> ([f32; 3], usize, usize) {
        tree_reading_at(ctx, renderer, target, battlefield, distance, 1.0)
    }

    /// The same reading with the lens's magnification forced: `magnification` 2 at 132 m
    /// draws the Near rung where a Mid rung would stand — the two rungs at ONE distance.
    fn tree_reading_at(
        ctx: &renderer_wgpu::GpuContext,
        renderer: &mut renderer_wgpu::SceneRenderer,
        target: &renderer_wgpu::OffscreenTarget,
        battlefield: &terrain::BattlefieldMap,
        distance: f32,
        magnification: f32,
    ) -> ([f32; 3], usize, usize) {
        let (width, height) = (1600u32, 900u32);
        let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
        let (tx, tz) = (500.0_f32, 500.0_f32);
        let (ex, ez) = (tx - 0.35 * distance, tz - 0.94 * distance);
        let eye = [ex, ground(ex, ez) + 2.2, ez];
        let look = [tx, ground(tx, tz) + 8.0, tz];
        renderer.shadow_focus = Some(look);
        let camera = renderer_api::Camera { eye, target: look, vertical_fov_degrees: 45.0 };
        let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
        let view_proj = renderer_api::view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        let mut render = |objects: Vec<renderer_api::RenderObject>| {
            let frame =
                renderer_api::RenderFrame { objects, ..renderer_api::RenderFrame::default() };
            renderer.set_render_frame(ctx, &frame);
            renderer.render(ctx, target.render_target(), view_proj, camera.eye).expect("render");
            target.read_rgba8(ctx).expect("readback")
        };
        let empty = render(Vec::new());
        let mut state = scene_build::tree_lod::TreeLodState::default();
        let objects = scene_build::tree_lod::tree_frame_objects(
            &battlefield.scenery,
            &battlefield.static_cover,
            &[],
            scene_build::tree_lod::TreeEye {
                position: glam::Vec3::from_array(eye),
                cone: None,
                magnification,
            },
            &mut state,
        );
        // Outside the bands: one rung, or the impostor's two quads sharing the window.
        assert!(!objects.is_empty() && objects.len() <= 2, "a solid rung at {distance} m");
        assert!(
            objects.iter().any(|o| o.dither[0] == 0.0)
                && objects.iter().any(|o| o.dither[1] == 1.0)
        );
        let with_tree = render(objects);
        let (mut sum, mut crown, mut wood) = ([0.0_f32; 3], 0usize, 0usize);
        for i in (0..(width * height * 4) as usize).step_by(4) {
            let (r, g, b) = (with_tree[i] as f32, with_tree[i + 1] as f32, with_tree[i + 2] as f32);
            let delta = (r - empty[i] as f32).abs()
                + (g - empty[i + 1] as f32).abs()
                + (b - empty[i + 2] as f32).abs();
            if delta < 30.0 {
                continue;
            }
            if g > r + 6.0 && g > b + 6.0 {
                sum = [sum[0] + r, sum[1] + g, sum[2] + b];
                crown += 1;
            } else {
                wood += 1;
            }
        }
        let n = crown.max(1) as f32;
        ([sum[0] / n, sum[1] / n, sum[2] / n], crown, wood)
    }

    /// LOD continuity, measured (the owner, 2026-09-03: "eliminate any place where I see a tree
    /// change its graphics"): the impostor just past the 300 m band and the Mid rung just
    /// before it are the SAME tree to the eye — mean crown colour within 15 % a channel,
    /// crown coverage within 25 % of the perspective expectation, and the wood still there at
    /// both distances (the far wood used to vanish under the alpha test).
    #[test]
    fn the_impostor_and_the_mid_rung_read_as_one_tree_across_the_band() {
        let Ok(ctx) = renderer_wgpu::GpuContext::headless() else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        let map = MapId::ProkhorovkaHill252_2;
        let mut battlefield = map_forge::battlefield(map);
        battlefield.static_cover.clear();
        // FLAT ground: on the map's own relief a crest between the eye and the tree hid the
        // trunk and the lower crown at some distances and the instrument read that as the
        // rung thinning (2026-09-03: 26 % "lost" at 127 m was a hill).
        battlefield.heightmap = terrain::HeightMap::flat(
            battlefield.heightmap.width(),
            battlefield.heightmap.height(),
            battlefield.heightmap.cell_size_m(),
            5.0,
        )
        .expect("a flat heightmap");
        let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
        battlefield.scenery = vec![terrain::SceneryInstance {
            kind: terrain::SceneryKind::Oak,
            position: [500.0, ground(500.0, 500.0), 500.0],
            yaw_rad: 0.6,
            scale: 1.0,
        }];
        let ((ground_v, ground_i), (statics_v, statics_i)) =
            crate::battlefield_ground_and_statics_meshes(&battlefield, &[]);
        let ground_maps = crate::bake_terrain_ground_maps(&battlefield);
        let target = renderer_wgpu::OffscreenTarget::new(&ctx, 1600, 900).expect("target");
        let mut renderer =
            renderer_wgpu::SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)
                .expect("renderer");
        renderer.set_battlefield_ground(
            &ctx,
            &ground_v,
            &ground_i,
            &ground_maps,
            &crate::terrain_material_set_for(map),
        );
        renderer.scene_lighting = renderer_api::SceneLighting::battlefield_default();
        renderer.scene_time_s = 12.0;
        for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
            renderer.register_mesh(&ctx, handle, &mesh);
        }
        let (color, normal) = scene_build::foliage_atlas_paint::foliage_atlas_chains();
        renderer.set_foliage_atlas(&ctx, &color, Some(&normal));
        renderer.set_bark_textures(&ctx, &scene_build::foliage_atlas_paint::bark_texture_layers());

        let before =
            scene_build::tree_lod::MID_MAX_M - scene_build::tree_lod::IMPOSTOR_FADE_HALF_M - 2.0;
        let after =
            scene_build::tree_lod::MID_MAX_M + scene_build::tree_lod::IMPOSTOR_FADE_HALF_M + 2.0;
        // The Near rung at 100 m is the truth every other reading is held against: a crown
        // shrinks with perspective and nothing else.
        let near_at = 100.0_f32;
        let (near_rgb, near_px, near_wood) =
            tree_reading(&ctx, &mut renderer, &target, &battlefield, near_at);
        let (mid_rgb, mid_px, mid_wood) =
            tree_reading(&ctx, &mut renderer, &target, &battlefield, before);
        let (imp_rgb, imp_px, imp_wood) =
            tree_reading(&ctx, &mut renderer, &target, &battlefield, after);
        let at_100 = |px: usize, d: f32| px as f32 * (d / near_at) * (d / near_at);
        eprintln!(
            "near {near_rgb:?} crown {near_px} wood {near_wood} | mid {mid_rgb:?} crown {mid_px} (={:.0} at 100 m) wood {mid_wood} (={:.0}) | impostor {imp_rgb:?} crown {imp_px} (={:.0}) wood {imp_wood} (={:.0})",
            at_100(mid_px, before),
            at_100(mid_wood, before),
            at_100(imp_px, after),
            at_100(imp_wood, after)
        );
        assert!(
            near_px > 2000 && mid_px > 200 && imp_px > 200,
            "every rung puts a crown on screen"
        );
        // Colour: the two far rungs sit in the same air, so they must agree with each other
        // (the Near reading is 190 m closer and out of most of the haze).
        for c in 0..3 {
            let ratio = imp_rgb[c] / mid_rgb[c].max(1.0);
            assert!((0.85..=1.15).contains(&ratio), "channel {c}: impostor/mid = {ratio:.3}");
        }
        // Coverage: each far rung's crown, brought to 100 m by perspective, is the Near
        // rung's crown within a quarter — the impostor used to draw 1.8x of it (a solid lid
        // from too few transparent bounces).
        for (name, px, d) in [("mid", mid_px, before), ("impostor", imp_px, after)] {
            let ratio = at_100(px, d) / near_px as f32;
            assert!((0.75..=1.25).contains(&ratio), "{name} crown coverage vs near = {ratio:.3}");
        }
        // Wood: present at both far distances (it used to vanish under the alpha test) and
        // not a forest of it (the thickened impostor used to draw 7x the Near wood).
        for (name, px, d) in [("mid", mid_wood, before), ("impostor", imp_wood, after)] {
            let ratio = at_100(px, d) / near_wood.max(1) as f32;
            assert!(
                px > 40 && (0.4..=3.0).contains(&ratio),
                "{name} wood vs near = {ratio:.3} ({px} px)"
            );
        }

        // The 120 m band, the same law: the Near rung just before it and the Mid rung just
        // past it are one tree — colour within 15 %, coverage within 25 % of the 100 m
        // reading by perspective, wood at both.
        let n_before =
            scene_build::tree_lod::NEAR_MAX_M - scene_build::tree_lod::NEAR_FADE_HALF_M - 2.0;
        let n_after =
            scene_build::tree_lod::NEAR_MAX_M + scene_build::tree_lod::NEAR_FADE_HALF_M + 2.0;
        let (a_rgb, a_px, a_wood) =
            tree_reading(&ctx, &mut renderer, &target, &battlefield, n_before);
        let (b_rgb, b_px, b_wood) =
            tree_reading(&ctx, &mut renderer, &target, &battlefield, n_after);
        let (f_rgb, f_px, f_wood) =
            tree_reading_at(&ctx, &mut renderer, &target, &battlefield, n_after, 2.0);
        eprintln!("at {n_after} m forced Near: {f_rgb:?} crown {f_px} wood {f_wood}");
        // The two rungs at ONE distance: the Near deck and the Mid deck are the same crown.
        let same_spot = f_px as f32 / b_px.max(1) as f32;
        assert!((0.9..=1.1).contains(&same_spot), "Near vs Mid at {n_after} m: {same_spot:.3}");
        let _ = (f_rgb, f_wood);
        eprintln!(
            "120 m band: near {a_rgb:?} crown {a_px} (={:.0}) wood {a_wood} | mid {b_rgb:?} crown {b_px} (={:.0}) wood {b_wood}",
            at_100(a_px, n_before),
            at_100(b_px, n_after)
        );
        for c in 0..3 {
            let ratio = b_rgb[c] / a_rgb[c].max(1.0);
            assert!((0.85..=1.15).contains(&ratio), "120 m channel {c}: mid/near = {ratio:.3}");
        }
        for (name, px, d) in [("near@108", a_px, n_before), ("mid@132", b_px, n_after)] {
            let ratio = at_100(px, d) / near_px as f32;
            assert!((0.75..=1.25).contains(&ratio), "{name} crown coverage vs 100 m = {ratio:.3}");
        }
        assert!(a_wood > 40 && b_wood > 40, "wood at both sides of 120 m: {a_wood} / {b_wood}");
    }
}
