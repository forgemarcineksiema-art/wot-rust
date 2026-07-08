//! Multi-angle T-54 studio render: front, top-down, three-quarter, rear-three-quarter and side into
//! separate PNGs, so the turret/mantlet casting is judged from every revealing angle â€” including the
//! rear â€” not just the flattering profile.
//! `cargo run -p client --example t54_views -- target/t54`  (writes `_front.png`, `_top.png`, â€¦)

use std::f32::consts::FRAC_PI_2;
use std::fs::File;
use std::io::BufWriter;

use client::{
    VehicleAssetCatalog, battlefield_scene_mesh, render_frame_from_objects,
    tank_vehicle_render_objects,
};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::prokhorovka_hill_252_2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prefix = std::env::args().nth(1).unwrap_or_else(|| "target/t54".to_string());
    let width = 1100u32;
    let height = 640u32;
    let aspect = width as f32 / height as f32;

    let battlefield = prokhorovka_hill_252_2();
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);
    let cx = 340.0_f32;
    let cz = 300.0_f32;
    let ground = battlefield.heightmap.sample_height(cx, cz).unwrap_or(0.0);

    let snapshot = TankSnapshot {
        tank_id: TankId(1),
        team: TeamId(1),
        vehicle: VehicleKind::T54_1951,
        position: [cx, ground, cz],
        yaw_rad: FRAC_PI_2,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 1000,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: VehicleKind::T54_1951.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    };

    let mut catalog = VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!("note: no Forge artifacts loaded ({error}); using neutral material");
    }
    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.72, 0.76, 0.62]);
    let render_frame = render_frame_from_objects(objects);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    renderer.scene_lighting = SceneLighting::garage_studio();
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(&ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(&ctx, handle, &maps);
    }
    renderer.set_vehicle_render_frame(&ctx, &render_frame);

    // The hull faces +X (yaw 90Â°), so the gun points toward -X... pick eyes around the turret centre.
    let tc = [cx, ground + 1.55, cz];
    // The hull faces +X, so the gun points toward -X: -X eyes are the front, +X eyes the rear.
    let views = [
        ("front", [cx - 7.0, ground + 1.7, cz], tc),
        ("top", [cx - 0.6, ground + 6.5, cz + 0.2], tc),
        ("threequarter", [cx - 5.5, ground + 3.2, cz + 5.0], tc),
        ("rearthreequarter", [cx + 5.5, ground + 3.2, cz - 5.0], tc),
        ("side", [cx, ground + 2.0, cz + 8.5], [cx, ground + 1.15, cz]),
    ];

    let projection = CameraProjectionPolicy::webgpu_default();
    for (name, eye, look) in views {
        let camera = Camera { eye, target: look, vertical_fov_degrees: 34.0 };
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("{prefix}_{name}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
