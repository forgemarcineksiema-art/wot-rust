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

/// Raised rear-¾ and front-¾ deck shots for the whole fleet — the close-up review angle for
/// engine decks, hull hatches, and headlights (model-logic gate, de-clone audit #1/#2/#12).
/// `cargo run -p client --example probe -- deck_probe -- <out_dir>`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = crate::sub_arg(1).unwrap_or_else(|| "target/deck_probe".to_string());
    std::fs::create_dir_all(&out_dir)?;
    let width = 1280u32;
    let height = 720u32;
    let aspect = width as f32 / height as f32;

    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);
    let cx = 340.0_f32;
    let cz = 300.0_f32;
    let ground = battlefield.heightmap.sample_height(cx, cz).unwrap_or(0.0);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    renderer.scene_lighting = SceneLighting::garage_studio();
    let projection = CameraProjectionPolicy::webgpu_default();

    let fleet = [
        (VehicleKind::T54_1951, "t54"),
        (VehicleKind::TigerI, "tiger_i"),
        (VehicleKind::TigerII, "tiger_ii"),
        (VehicleKind::Jagdtiger, "jagdtiger"),
        (VehicleKind::PantherII, "panther_ii"),
        (VehicleKind::IS3, "is3"),
        (VehicleKind::Centurion, "centurion"),
        (VehicleKind::T34_85, "t34_85"),
    ];

    for (kind, slug) in fleet {
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
            rack_fire_remaining_s: None,
            crew_unconscious_mask: 0,
            crew_weakened_mask: 0,
            crew_down_remaining_s: Default::default(),
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

        // Rear-¾ raised (engine deck + roof hatches) and front-¾ raised (glacis + headlights).
        let views = [
            ("deck", [cx + 4.5, ground + 5.2, cz - 7.5]),
            ("bow", [cx + 4.5, ground + 4.0, cz + 8.0]),
        ];
        for (view, eye) in views {
            let camera = Camera { eye, target: [cx, ground + 1.3, cz], vertical_fov_degrees: 34.0 };
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
