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

/// A tight, single-vehicle right-profile studio render of the IS-3 through the PBR catalog path —
/// the same path the garage uses, with the baked Forge artifacts in `target/forge` loaded so the
/// shot shows the real textured material (falling back to the live hybrid bake + neutral material
/// if no artifact is present). Best for inspecting the running gear and turret close up.
/// `cargo run -p client --example muzzle_probe -- out.png`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "target/muzzle_probe.png".to_string());
    let width = 1280u32;
    let height = 720u32;
    let aspect = width as f32 / height as f32;

    let battlefield = prokhorovka_hill_252_2();
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);
    let cx = 340.0_f32;
    let cz = 300.0_f32;
    let ground = battlefield.heightmap.sample_height(cx, cz).unwrap_or(0.0);

    // Side-on: rotate the hull 90° so its length faces the camera, gun level.
    let snapshot = TankSnapshot {
        tank_id: TankId(1),
        team: TeamId(1),
        vehicle: VehicleKind::TigerI,
        position: [cx, ground, cz],
        yaw_rad: FRAC_PI_2,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.04,
        hit_points: 1000,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: VehicleKind::TigerI.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
    };

    // Load the baked Forge artifacts (textured materials) the garage uses; harmless if absent.
    let mut catalog = VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!("note: no Forge artifacts loaded ({error}); using neutral material");
    }
    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.72, 0.76, 0.62]);
    let render_frame = render_frame_from_objects(objects);

    // A close, slightly raised right-profile camera that frames the whole hull and running gear.
    let camera = Camera {
        eye: [cx + 4.6, ground + 2.5, cz + 2.6],
        target: [cx + 5.1, ground + 2.15, cz],
        vertical_fov_degrees: 34.0,
    };
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
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(&ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(&ctx, handle, &maps);
    }
    renderer.set_vehicle_render_frame(&ctx, &render_frame);
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

    let pixels = target.read_rgba8(&ctx)?;
    let file = File::create(&path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path} ({width}x{height})");
    Ok(())
}
