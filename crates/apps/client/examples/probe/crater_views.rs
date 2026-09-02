//! Close-up review renders of TRUE terrain deformation (Fizyczny Świat P4c): the same crater
//! ledger the sim replicates, folded through `sample_height`, re-meshed by the P4b ground path
//! and dressed with the P3 scorch/spoil stamps — exactly the picture the game shows after a
//! high-explosive barrage. The physical reference to judge against (from war photography of HE
//! ground bursts): a bowl sunk below grade, a RAISED rim of displaced subsoil — LIGHTER than
//! the field — just past the lip, a scorched dark bowl floor (earth-dark, never a void), and
//! clods of soil thrown downrange. Writes under `target/`:
//!   `crater_before.png`  — the virgin field from the review vantage.
//!   `crater_close.png`   — a fresh 122 mm crater, close.
//!   `crater_grazing.png` — the same crater at a grazing angle (rim silhouette).
//!   `crater_field.png`   — the shelled field: 122 mm, 100 mm, and a re-shelled (deepened) hole.
//!
//! `cargo run -p client --example probe -- crater_views`

use std::fs::File;
use std::io::BufWriter;

use client::{
    TerrainScars, bake_terrain_ground_maps, battlefield_dressing_objects,
    battlefield_ground_and_statics_meshes, battlefield_water_mesh, grass_card_dressing_mesh,
    register_battlefield_dressing_meshes, terrain_material_set_for,
};
use game_core::{ShellImpact, ShellType, TankId};
use glam::Vec3;
use renderer_api::{
    Camera, CameraProjectionPolicy, FxVertex, SceneLighting, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// The open mid-field spot the destruction showcase already uses — flat enough to read shape.
const SPOT: [f32; 2] = [340.0, 300.0];

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    let mut battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let ground_y =
        |bf: &terrain::BattlefieldMap, x: f32, z: f32| bf.heightmap.sample_height(x, z).unwrap();

    // The barrage, derived exactly the way the sim derives it: a 122 mm burst at the focus, a
    // 100 mm burst beside it, and one spot shelled twice (the ledger merges and deepens).
    let bursts: [(f32, f32, f32); 4] = [
        (SPOT[0], SPOT[1], 122.0),
        (SPOT[0] + 7.5, SPOT[1] + 4.0, 100.0),
        (SPOT[0] - 6.5, SPOT[1] + 6.0, 122.0),
        (SPOT[0] - 6.5, SPOT[1] + 6.0, 122.0),
    ];
    let mut ledger = Vec::new();
    for &(x, z, caliber) in &bursts {
        let y = ground_y(&battlefield, x, z);
        sim::record_high_explosive_burst(&mut ledger, Vec3::new(x, y, z), caliber, &[]);
    }

    // Bake the splat/macro maps once — the ground's dress never depends on the ledger.
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let materials = terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
    let (water_vertices, water_indices) = battlefield_water_mesh(&battlefield);

    let focus_y = ground_y(&battlefield, SPOT[0], SPOT[1]);
    let close_eye = [SPOT[0] + 7.0, focus_y + 3.4, SPOT[1] + 5.5];
    let close_look = [SPOT[0], focus_y - 0.4, SPOT[1]];
    let grazing_eye = [SPOT[0] + 12.0, focus_y + 1.6, SPOT[1] - 1.0];
    let field_eye = [SPOT[0] + 16.0, focus_y + 8.0, SPOT[1] + 13.0];
    let field_look = [SPOT[0] - 2.0, focus_y, SPOT[1] + 3.0];

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;

    // ---- BEFORE: the virgin field ------------------------------------------------------------
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    renderer.set_battlefield_ground(&ctx, &ground_v, &ground_i, &ground_maps, &materials);
    renderer.set_water(&ctx, &water_vertices, &water_indices);
    let (dressing_v, dressing_i) = grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    register_battlefield_dressing_meshes(&ctx, &mut renderer);
    set_crater_view_grass(&ctx, &mut renderer, &battlefield, &ground_maps, &materials, close_eye);
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;
    renderer.shadow_focus = Some([SPOT[0], focus_y, SPOT[1]]);
    shoot(&ctx, &target, &mut renderer, close_eye, close_look, width, height, "crater_before")?;

    // ---- AFTER: fold the ledger in, re-mesh (the P4b path), stamp the P3 scars ----------------
    battlefield.heightmap.set_craters(&ledger);
    let ((ground_v, ground_i), _) = battlefield_ground_and_statics_meshes(&battlefield, &[]);
    renderer.update_battlefield_ground_geometry(&ctx, &ground_v, &ground_i);
    let (dressing_v, dressing_i) = grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);

    // The presentation stamps, recorded onto the DEFORMED ground (the ingest order): a shallow
    // incoming track from the south-west, like tank fire arrives.
    let mut scars = TerrainScars::default();
    for &(x, z, caliber) in &bursts {
        let y = ground_y(&battlefield, x, z);
        scars.record(
            &ShellImpact {
                owner: Some(TankId(1)),
                position: Vec3::new(x, y, z),
                surface: game_core::ImpactSurface::Terrain,
                shell_type: ShellType::HighExplosive,
                direction: Vec3::new(0.42, -0.30, 0.55).normalize(),
                caliber_mm: caliber,
                ..Default::default()
            },
            &battlefield.heightmap,
        );
    }
    let mut fx: Vec<FxVertex> = Vec::new();
    scars.append_quads(&mut fx);
    renderer.set_fx(&ctx, &fx);

    // The grass field follows each review camera exactly like the live client's cached ring;
    // the crater ledger makes the shell's kill zone read as burned-bare ground.
    set_crater_view_grass(&ctx, &mut renderer, &battlefield, &ground_maps, &materials, close_eye);
    shoot(&ctx, &target, &mut renderer, close_eye, close_look, width, height, "crater_close")?;
    set_crater_view_grass(&ctx, &mut renderer, &battlefield, &ground_maps, &materials, grazing_eye);
    shoot(&ctx, &target, &mut renderer, grazing_eye, close_look, width, height, "crater_grazing")?;
    set_crater_view_grass(&ctx, &mut renderer, &battlefield, &ground_maps, &materials, field_eye);
    shoot(&ctx, &target, &mut renderer, field_eye, field_look, width, height, "crater_field")?;

    // The AP furrows, exactly as the battle shows them: kinetic rounds ploughed into grass,
    // seen from a hull-height camera (the view the player actually judges them from).
    let mut ap_scars = TerrainScars::default();
    for index in 0..4 {
        let x = SPOT[0] - 14.0 + index as f32 * 6.5;
        let z = SPOT[1] - 18.0 - index as f32 * 2.0;
        let y = ground_y(&battlefield, x, z);
        ap_scars.record(
            &ShellImpact {
                owner: Some(TankId(1)),
                position: Vec3::new(x, y, z),
                surface: game_core::ImpactSurface::Terrain,
                shell_type: ShellType::ArmorPiercing,
                direction: Vec3::new(-0.35, -0.25, -0.9).normalize(),
                caliber_mm: 100.0,
                ..Default::default()
            },
            &battlefield.heightmap,
        );
    }
    let mut ap_fx: Vec<FxVertex> = Vec::new();
    ap_scars.append_quads(&mut ap_fx);
    renderer.set_fx(&ctx, &ap_fx);
    let f_y = ground_y(&battlefield, SPOT[0] - 8.0, SPOT[1] - 20.0);
    let furrow_eye = [SPOT[0] + 2.0, f_y + 2.6, SPOT[1] - 6.0];
    let furrow_look = [SPOT[0] - 8.0, f_y, SPOT[1] - 22.0];
    set_crater_view_grass(&ctx, &mut renderer, &battlefield, &ground_maps, &materials, furrow_eye);
    shoot(&ctx, &target, &mut renderer, furrow_eye, furrow_look, width, height, "furrow_field")?;
    // The same furrows under the warm evening grade — the light that exposed the pale-border
    // sticker on a live screenshot. Turned earth must read dark under EVERY profile.
    if let Some(view) = client::review_views_for(terrain::MapId::ProkhorovkaHill252_2, &battlefield)
        .into_iter()
        .find(|view| view.name.contains("evening") || view.name.contains("golden"))
    {
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
    }
    shoot(&ctx, &target, &mut renderer, furrow_eye, furrow_look, width, height, "furrow_evening")?;
    Ok(())
}

fn set_crater_view_grass(
    ctx: &GpuContext,
    renderer: &mut SceneRenderer,
    battlefield: &terrain::BattlefieldMap,
    maps: &renderer_api::TerrainGroundMaps,
    materials: &renderer_api::TerrainMaterialSet,
    eye: [f32; 3],
) {
    // The grass ring AND the tree ladder, exactly as the battle submits them.
    let mut tree_lod_state = scene_build::tree_lod::TreeLodState::default();
    let dressing = battlefield_dressing_objects(
        battlefield,
        maps,
        materials,
        &[],
        glam::Vec3::from_array(eye),
        &mut tree_lod_state,
    );
    renderer.set_render_frame(
        ctx,
        &renderer_api::RenderFrame { objects: dressing, ..Default::default() },
    );
}

#[expect(clippy::too_many_arguments)]
fn shoot(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    renderer: &mut SceneRenderer,
    eye: [f32; 3],
    look: [f32; 3],
    width: u32,
    height: u32,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let camera = Camera { eye, target: look, vertical_fov_degrees: 42.0 };
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        width as f32 / height as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );
    renderer.render(ctx, target.render_target(), view_proj, camera.eye)?;
    let pixels = target.read_rgba8(ctx)?;
    let path = format!("target/{name}.png");
    let file = File::create(&path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path}");
    Ok(())
}
