//! Review render of a Prokhorovka farmyard (zagroda): barn, cottage, shed — and the wooden
//! yard fences (Fizyczny Świat P10) a hull can crush and a shell can sweep. One PNG:
//! `cargo run -p client --example farmyard_views`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, battlefield_water_mesh,
    terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::prokhorovka_hill_252_2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    let battlefield = prokhorovka_hill_252_2();
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let (water_v, water_i) = battlefield_water_mesh(&battlefield);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_v,
        &ground_i,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
    );
    renderer.set_water(&ctx, &water_v, &water_i);
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;

    // The south Oktyabrskiy yard: cottage [470,458], shed [503,455], barn [488,470], and the
    // two fence runs closing it ([479, 447.5] along x, [461.5, 466] along z).
    let ground_y = battlefield.heightmap.sample_height(480.0, 455.0).unwrap_or(0.0);
    let eye = [455.0, ground_y + 7.0, 436.0];
    let look = [484.0, ground_y + 1.0, 460.0];
    renderer.shadow_focus = Some(look);

    let camera = Camera { eye, target: look, vertical_fov_degrees: 50.0 };
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        width as f32 / height as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
    let pixels = target.read_rgba8(&ctx)?;
    let path = "target/farmyard_south.png";
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path}");
    Ok(())
}
