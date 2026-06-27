use std::fs::File;
use std::io::BufWriter;

use client::{garage_overlay, hangar_camera_pivot, hangar_scene_mesh};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// Render the garage in the browse-only tech tree view and save a PNG. This exercises the
/// tech-tree overlay (nation columns + vehicle nodes) headlessly for visual review.
/// `cargo run -p client --example garage_techview -- out.png`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "target/garage_techview.png".to_string());
    let width = 1280u32;
    let height = 720u32;
    let aspect = width as f32 / height as f32;

    // The hangar scene (the tech tree overlay is drawn over it, dimmed by the panel background).
    let (terrain_vertices, terrain_indices) = hangar_scene_mesh();

    // A parked T-54 on the turntable, the same pose the garage uses for the preview.
    let spec = VehicleKind::T54_1951.spec();
    let snapshot = TankSnapshot {
        tank_id: TankId(0),
        team: TeamId(1),
        vehicle: VehicleKind::T54_1951,
        position: [0.0, client::TURNTABLE_TOP_M, 0.0],
        yaw_rad: 0.6,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    };
    let _ = snapshot; // The tech tree view does not render the vehicle mesh; it's an overlay-only
    // screen over the dim hangar. Kept here so the example stays close to the
    // real garage frame structure.

    // Hero orbit camera — the same framing the garage opens with.
    let pivot = hangar_camera_pivot();
    let orbit_yaw = 0.60_f32;
    let orbit_pitch = 0.28_f32;
    let orbit_distance = 11.5_f32;
    let horizontal = orbit_distance * orbit_pitch.cos();
    let eye = pivot
        + glam::Vec3::new(
            horizontal * orbit_yaw.sin(),
            orbit_distance * orbit_pitch.sin(),
            horizontal * orbit_yaw.cos(),
        );
    let camera =
        Camera { eye: eye.to_array(), target: pivot.to_array(), vertical_fov_degrees: 32.0 };
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        aspect,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    renderer.scene_lighting = SceneLighting::garage_studio();
    renderer.set_sky(0.07, 0.05, 0.04);

    // The tech tree HUD overlay.
    let (font_w, font_h, font_coverage) = client::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);
    let hud = garage_overlay(true, aspect);
    renderer.set_hud(&ctx, &hud);

    renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

    let pixels = target.read_rgba8(&ctx)?;
    let file = File::create(&path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path} ({width}x{height}, {} HUD vertices)", hud.len());
    Ok(())
}
