use std::fs::File;
use std::io::BufWriter;

use renderer_api::{
    Camera, CameraProjectionPolicy, SceneLighting, SceneVertex, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// Render the sky dome alone — each outdoor look at three camera elevations (horizonward,
/// mid-sky, near-zenith) over a bare ground plane. The review tool for the cloud pattern:
/// the zenith shots are where a lattice artefact in the value noise (square interpolation
/// cells blown up by the dome projection) is most visible. One PNG per look x elevation
/// under `target/`: `cargo run -p client --example probe -- sky_probe`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1280u32;
    let height = 720u32;

    // A bare ground plane: enough world for the horizon line to read, nothing to distract
    // from the dome.
    let n = [0.0, 1.0, 0.0];
    let c = [0.32, 0.40, 0.26];
    let vertices = vec![
        SceneVertex::new([-400.0, 0.0, -400.0], n, c),
        SceneVertex::new([400.0, 0.0, -400.0], n, c),
        SceneVertex::new([400.0, 0.0, 400.0], n, c),
        SceneVertex::new([-400.0, 0.0, 400.0], n, c),
    ];
    let indices = vec![0u32, 2, 1, 0, 3, 2];

    let looks: [(&str, SceneLighting); 4] = [
        ("noon", SceneLighting::battlefield_default()),
        ("overcast", SceneLighting::prokhorovka_overcast()),
        ("dawn", SceneLighting::bystra_dawn_fog()),
        ("rain", SceneLighting::bystra_rain()),
    ];
    // Elevation of the view centre above the horizon: the horizon band, the cloud belt, and
    // the near-zenith cap where one noise cell used to span tens of degrees.
    let elevations: [(&str, f32); 3] = [("low", 12.0), ("mid", 40.0), ("high", 68.0)];

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices)?;
    // A live clock so the drift phase is a mid-battle one, not the t=0 special case.
    renderer.scene_time_s = 240.0;

    for (look_name, lighting) in looks {
        renderer.scene_lighting = lighting;
        for (elevation_name, degrees) in elevations {
            let eye = [0.0, 3.0, 0.0];
            let e = degrees.to_radians();
            let camera = Camera {
                eye,
                target: [0.0, 3.0 + 50.0 * e.sin(), -50.0 * e.cos()],
                vertical_fov_degrees: 55.0,
            };
            let projection = CameraProjectionPolicy::webgpu_default();
            let view_proj = view_projection_matrix(
                &camera,
                width as f32 / height as f32,
                projection.near_plane_m(),
                projection.far_plane_m(),
            );
            renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

            let pixels = target.read_rgba8(&ctx)?;
            let path = format!("target/sky_{look_name}_{elevation_name}.png");
            let file = File::create(&path)?;
            let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(&pixels)?;
            println!("wrote {path}");
        }
    }
    Ok(())
}
