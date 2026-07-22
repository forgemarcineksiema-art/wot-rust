//! Close-up review render of the FactoryHall style (urban-map program PR-09) — the
//! model-logic gate's artifact: the wagon door under its lintel, high machine-wall windows,
//! and the glazed clerestory band under the flat cap, plus the honest rubble form. Two PNGs:
//! `cargo run -p client --example factory_probe`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{StaticCoverKind, StaticCoverObject};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    let mut battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let ground_y = battlefield.heightmap.sample_height(500.0, 500.0).unwrap_or(0.0);
    battlefield.static_cover = vec![StaticCoverObject {
        id: "mill_factory_probe".into(),
        name: "factory hall (probe)".into(),
        kind: StaticCoverKind::FarmBuilding,
        center: [500.0, ground_y + 4.5, 500.0],
        half_extents_m: [6.5, 4.5, 11.0],
    }];
    battlefield.scenery.clear();

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let ground_maps = bake_terrain_ground_maps(&battlefield);

    for (states, label) in [(vec![0u8], "intact"), (vec![1u8], "rubble")] {
        let ((ground_v, ground_i), (statics_v, statics_i)) =
            battlefield_ground_and_statics_meshes(&battlefield, &states);
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
        renderer.set_battlefield_ground(
            &ctx,
            &ground_v,
            &ground_i,
            &ground_maps,
            &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
        );
        renderer.scene_lighting = SceneLighting::battlefield_default();
        renderer.scene_time_s = 12.0;
        // Look at the +Z gable end where the wagon door sits, with the long wall receding.
        let eye = [488.0, ground_y + 4.0, 522.0];
        let look = [500.0, ground_y + 4.0, 502.0];
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
        let path = format!("target/factory_{label}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
