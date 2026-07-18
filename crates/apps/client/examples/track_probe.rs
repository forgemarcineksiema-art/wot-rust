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

/// The track autopsy camera: VERY close shots of the running gear — the driven end wrap, the
/// top run seen along its length, and the ground run at wheel height — per fleet vehicle.
/// `cargo run -p client --example track_probe -- <out_dir>`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| "target/track_probe".to_string());
    std::fs::create_dir_all(&out_dir)?;
    let width = 1280u32;
    let height = 720u32;
    let aspect = width as f32 / height as f32;

    let battlefield = prokhorovka_hill_252_2();
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);
    let cx = 340.0_f32;
    let cz = 300.0_f32;
    let ground = battlefield.heightmap.sample_height(cx, cz).unwrap_or(0.0);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    renderer.scene_lighting = SceneLighting::garage_studio();
    let projection = CameraProjectionPolicy::webgpu_default();

    let fleet = [
        (VehicleKind::T54_1951, "t54"),
        (VehicleKind::TigerI, "tiger_i"),
        (VehicleKind::TigerII, "tiger_ii"),
        (VehicleKind::PantherII, "panther_ii"),
        (VehicleKind::IS3, "is3"),
        (VehicleKind::Centurion, "centurion"),
        (VehicleKind::T34_85, "t34_85"),
    ];

    for (kind, slug) in fleet {
        let bp = game_core::VehicleBlueprint::for_vehicle(kind);
        let (end_z, track_x, top_y) = bp
            .map(|b| (b.track.end_z, (b.track.inner_x + b.track.outer_x) * 0.5, b.track.top_y))
            .unwrap_or((2.6, 1.4, 0.9));
        let snapshot = TankSnapshot {
            tank_id: TankId(1),
            team: TeamId(1),
            vehicle: kind,
            position: [cx, ground, cz],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.02,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 0.0,
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
            fuel_fire: false,
        };

        let mut catalog = VehicleAssetCatalog::default();
        if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
            eprintln!("note: no Forge artifacts loaded ({error}); using neutral material");
        }
        let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.72, 0.76, 0.62]);
        let render_frame = render_frame_from_objects(objects);
        for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
            renderer.register_vehicle_mesh(&ctx, handle, &mesh);
        }
        for (handle, maps) in catalog.take_pending_vehicle_materials() {
            renderer.register_vehicle_material(&ctx, handle, &maps);
        }
        renderer.set_vehicle_render_frame(&ctx, &render_frame);

        // World-space: vehicle yaw 0, so hull +z is world +z; track plane at x = cx + track_x.
        let axle_y = ground + 0.45;
        let views = [
            // Dead side-on at axle height, long lens — the user's review angle.
            ("side", [cx + track_x + 9.0, axle_y + 0.25, cz], [cx + track_x, axle_y + 0.15, cz]),
            // The driven end wrap, seen from just outside and slightly ahead.
            (
                "wrap",
                [cx + track_x + 1.6, ground + 0.9, cz + end_z + 1.2],
                [cx + track_x - 0.2, ground + 0.55, cz + end_z - 0.2],
            ),
            // Along the top run, close and shallow.
            (
                "top",
                [cx + track_x + 1.1, ground + top_y + 0.55, cz + 3.4],
                [cx + track_x - 0.1, ground + top_y - 0.05, cz - 0.5],
            ),
            // The ground run and wheel feet, from low mid-flank.
            (
                "ground",
                [cx + track_x + 2.2, ground + 0.45, cz + 0.3],
                [cx + track_x - 0.3, ground + 0.35, cz - 0.4],
            ),
        ];
        for (view, eye, look) in views {
            let camera = Camera { eye, target: look, vertical_fov_degrees: 38.0 };
            let view_proj = view_projection_matrix(
                &camera,
                aspect,
                projection.near_plane_m(),
                projection.far_plane_m(),
            );
            renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
            let pixels = target.read_rgba8(&ctx)?;
            let path = format!("{out_dir}/{slug}_{view}.png");
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
