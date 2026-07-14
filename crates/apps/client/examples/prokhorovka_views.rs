use std::fs::File;
use std::io::BufWriter;

use client::{
    GRASS_MESH_HANDLE, bake_terrain_ground_maps, battlefield_ground_and_statics_meshes,
    battlefield_water_mesh, grass_frame_objects, grass_tuft_mesh, prokhorovka_review_views,
    terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, RenderFrame, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::prokhorovka_hill_252_2;

/// Render the Prokhorovka steppe offscreen in its three times of day — the hazy noon, the
/// golden evening whose low western sun the shadow cascades rake long, and the dry lead
/// overcast — from the hill panorama and a mid-field vantage. The views are the canonical
/// review set (`client::prokhorovka_review_views`), shared with the `look_goldens` harness so
/// what a human reviews here is exactly what the goldens lock. One PNG per view under
/// `target/`: `cargo run -p client --example prokhorovka_views`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1280u32;
    let height = 720u32;

    let battlefield = prokhorovka_hill_252_2();
    let ((ground_vertices, ground_indices), (statics_vertices, statics_indices)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let (water_vertices, water_indices) = battlefield_water_mesh(&battlefield);
    let views = prokhorovka_review_views(&battlefield);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_vertices, &statics_indices)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_vertices,
        &ground_indices,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
    );
    renderer.set_water(&ctx, &water_vertices, &water_indices);
    renderer.scene_time_s = 12.0;
    renderer.register_mesh(&ctx, GRASS_MESH_HANDLE, &grass_tuft_mesh());

    for view in &views {
        let grass = grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &ground_maps,
            &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
            glam::Vec3::from_array(view.eye),
        );
        renderer.set_render_frame(&ctx, &RenderFrame { objects: grass, ..RenderFrame::default() });
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
        renderer.shadow_focus = Some(view.target);

        let camera = Camera { eye: view.eye, target: view.target, vertical_fov_degrees: 55.0 };
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/{}.png", view.name);
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
