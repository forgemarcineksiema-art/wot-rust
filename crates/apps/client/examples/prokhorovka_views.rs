use std::fs::File;
use std::io::BufWriter;

use client::{battlefield_scene_mesh, battlefield_water_mesh};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::prokhorovka_hill_252_2;

/// Render the Prokhorovka steppe offscreen in its three times of day — the hazy noon, the
/// golden evening whose low western sun the shadow cascades rake long, and the dry lead
/// overcast — from the hill panorama and a mid-field vantage. One PNG per view under
/// `target/`: `cargo run -p client --example prokhorovka_views`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1280u32;
    let height = 720u32;

    let battlefield = prokhorovka_hill_252_2();
    let (vertices, indices) = battlefield_scene_mesh(&battlefield);
    let (water_vertices, water_indices) = battlefield_water_mesh(&battlefield);
    let ground =
        |x: f32, z: f32| -> f32 { battlefield.heightmap.sample_height(x, z).unwrap_or(5.0) };

    struct View {
        name: &'static str,
        eye: [f32; 3],
        target: [f32; 3],
        lighting: SceneLighting,
        sky: (f64, f64, f64),
    }
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];
    let views = [
        View {
            name: "prokhorovka_noon",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::battlefield_default(),
            sky: (0.55, 0.69, 0.87),
        },
        View {
            name: "prokhorovka_golden_evening",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
        },
        View {
            name: "prokhorovka_overcast",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::prokhorovka_overcast(),
            sky: (0.48, 0.51, 0.55),
        },
        View {
            name: "prokhorovka_evening_midfield",
            eye: at(500.0, 6.0, 460.0),
            target: at(620.0, 2.0, 520.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
        },
    ];

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices)?;
    renderer.set_water(&ctx, &water_vertices, &water_indices);
    renderer.scene_time_s = 12.0;

    for view in &views {
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
