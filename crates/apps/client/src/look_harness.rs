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
    for (handle, mesh) in crate::grass_species_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    // The procedural leaf atlas rides the SAME entry the battle uses (Drzewa 3.0 PR5) — the
    // harness header documents how the review path once lost the atlas bind and locked white
    // trees; wiring it here keeps the goldens honest about what the player sees.
    let (foliage_color, foliage_normal) = scene_build::foliage_atlas_paint::foliage_atlas_chains();
    renderer.set_foliage_atlas(&ctx, &foliage_color, Some(&foliage_normal));

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
            battlefield.water_view(),
            &battlefield.static_cover,
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

        let camera = Camera { eye: view.eye, target: view.target, vertical_fov_degrees };
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
    }
}
