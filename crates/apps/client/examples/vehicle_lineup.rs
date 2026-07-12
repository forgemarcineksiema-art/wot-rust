use std::fs::File;
use std::io::BufWriter;

use client::{
    VehicleMeshCatalog, battlefield_scene_mesh, render_frame_from_objects, tank_render_objects,
};
use game_core::{TankId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{Camera, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::prokhorovka_hill_252_2;

/// Render every production vehicle side by side offscreen through the baked [`RenderFrame`] path.
/// `cargo run -p client --example vehicle_lineup -- out.png`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "target/vehicle_lineup.png".to_string());
    let width = 1800u32;
    let height = 620u32;

    let battlefield = prokhorovka_hill_252_2();
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);

    // A roughly level patch of the map; tanks are spread across it on the X axis.
    let center_x = 340.0_f32;
    let center_z = 300.0_f32;
    let spacing = 10.0_f32;
    let palette = [
        [0.34, 0.42, 0.30],
        [0.30, 0.38, 0.46],
        [0.46, 0.40, 0.26],
        [0.44, 0.30, 0.28],
        [0.32, 0.36, 0.40],
        [0.40, 0.34, 0.44],
        [0.28, 0.44, 0.40],
        [0.42, 0.44, 0.28],
    ];

    let mut catalog = VehicleMeshCatalog::default();
    let mut render_objects = Vec::new();
    let roster = VehicleKind::PLAYABLE;
    let center_index = (roster.len().saturating_sub(1)) as f32 * 0.5;
    for (index, kind) in roster.into_iter().enumerate() {
        let x = center_x + (index as f32 - center_index) * spacing;
        let ground = battlefield.heightmap.sample_height(x, center_z).unwrap_or(0.0);
        let snapshot = TankSnapshot {
            tank_id: TankId(index as u64 + 1),
            team: game_core::TeamId(1),
            vehicle: kind,
            position: [x, ground, center_z],
            yaw_rad: 2.5,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.20,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: kind.spec().gun.dispersion_mrad,
            module_hit_points: kind.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
        };
        render_objects.append(&mut tank_render_objects(&mut catalog, &snapshot, palette[index]));
    }

    let base = battlefield.heightmap.sample_height(center_x, center_z).unwrap_or(0.0);
    // Pulled back far enough that the WHOLE seven-vehicle roster fits the frame — the old
    // 34 m boom cropped the end vehicles out of their own lineup.
    let camera = Camera {
        eye: [center_x, base + 15.0, center_z - 44.0],
        target: [center_x, base + 1.3, center_z],
        vertical_fov_degrees: 44.0,
    };
    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        width as f32 / height as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    for (handle, mesh) in catalog.take_pending_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    renderer.set_render_frame(&ctx, &render_frame_from_objects(render_objects));
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

    let pixels = target.read_rgba8(&ctx)?;
    let file = File::create(&path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path} ({width}x{height}) â€” {} vehicles", VehicleKind::PLAYABLE.len());
    Ok(())
}
