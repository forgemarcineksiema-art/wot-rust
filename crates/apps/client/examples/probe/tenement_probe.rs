//! Close-up review render of the Tenement style (urban-map program PR-08) — the model-logic
//! gate's artifact: floors, string courses, the window grid, the paired entrance, and the
//! shallow roof, plus the honest rubble form. Two PNGs:
//! `cargo run -p client --example probe -- tenement_probe`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{StaticCoverKind, StaticCoverObject};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    // The steppe substrate with its cover swapped for one tenement pair: the probe looks at
    // the building, not the map.
    let mut battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let ground_y = battlefield.heightmap.sample_height(500.0, 500.0).unwrap_or(0.0);
    battlefield.static_cover = vec![
        StaticCoverObject {
            id: "tenement_probe_a".into(),
            name: "tenement (probe)".into(),
            kind: StaticCoverKind::FarmBuilding,
            center: [500.0, ground_y + 6.0, 500.0],
            half_extents_m: [4.6, 6.0, 6.0],
        },
        StaticCoverObject {
            id: "tenement_probe_b".into(),
            name: "tenement (probe, street pair)".into(),
            kind: StaticCoverKind::FarmBuilding,
            center: [500.0, ground_y + 6.0, 516.0],
            half_extents_m: [4.6, 6.0, 6.0],
        },
    ];
    battlefield.scenery.clear();

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let ground_maps = bake_terrain_ground_maps(&battlefield);

    for (states, label, eye_lift) in
        [(vec![0u8, 0u8], "intact", 4.0f32), (vec![1u8, 0u8], "rubble", 3.0f32)]
    {
        let ((ground_v, ground_i), (statics_v, statics_i)) =
            battlefield_ground_and_statics_meshes(&battlefield, &states);
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
        // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
        crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
        renderer.set_battlefield_ground(
            &ctx,
            &ground_v,
            &ground_i,
            &ground_maps,
            &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
        );
        renderer.scene_lighting = SceneLighting::battlefield_default();
        renderer.scene_time_s = 12.0;
        let eye = [482.0, ground_y + eye_lift, 486.0];
        let look = [500.0, ground_y + 5.0, 505.0];
        renderer.shadow_focus = Some(look);
        let camera = Camera { eye, target: look, vertical_fov_degrees: 55.0 };
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/tenement_{label}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
