//! Render the blueprint-driven lofted T-54 turret (`vehicle_build::t54_turret_loft`, fed by
//! `TurretLoftVisual`) from front/top/3-4/side, so the cast shell is judged from every angle that
//! exposed the old metaball turret's lumps. Turret shell only (cupola/mantlet bed in at M3).
//! `cargo run -p client --example probe -- t54_loft_spike -- target/loft`

use std::fs::File;
use std::io::BufWriter;

use game_core::{VehicleBlueprint, VehicleKind};
use renderer_api::{
    Camera, CameraProjectionPolicy, SceneLighting, SceneVertex, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

const COLOR: [f32; 3] = [0.60, 0.64, 0.54];

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let prefix = crate::sub_arg(1).unwrap_or_else(|| "target/loft".to_string());
    let (w, h) = (1100u32, 680u32);
    let aspect = w as f32 / h as f32;

    let turret_loft = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 blueprint")
        .hybrid()
        .expect("T-54 hybrid")
        .turret_loft;
    let mesh = vehicle_build::t54_turret_loft(&turret_loft);
    let verts: Vec<SceneVertex> = mesh
        .vertices()
        .iter()
        .map(|v| SceneVertex::new(v.position.to_array(), v.normal.to_array(), COLOR))
        .collect();
    let indices = mesh.indices().to_vec();

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, w, h)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &verts, &indices)?;
    renderer.scene_lighting = SceneLighting::garage_studio();

    let look = [0.0, 1.58, 0.0];
    let views = [
        ("front", [0.0, 1.72, 3.5]),
        ("top", [0.03, 5.1, 0.06]),
        ("threequarter", [2.6, 2.9, 3.0]),
        ("side", [3.6, 1.85, 0.0]),
    ];
    let projection = CameraProjectionPolicy::webgpu_default();
    for (name, eye) in views {
        let camera = Camera { eye, target: look, vertical_fov_degrees: 32.0 };
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("{prefix}_{name}.png");
        let mut enc = png::Encoder::new(BufWriter::new(File::create(&path)?), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
