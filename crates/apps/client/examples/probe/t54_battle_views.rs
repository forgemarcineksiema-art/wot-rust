//! T-54 rendered under the **battle** lighting profile (`SceneLighting::battlefield_default`) through
//! the live PBR vehicle path — the same light the windowed game uses. The `t54_views` studio shots
//! use `garage_studio`, so this closes the QA gap where humans reviewed a profile the game never
//! ships. See `docs/atmosphere-policy.md`.
//! `cargo run -p client --example probe -- t54_battle_views -- target/t54_battle`  (writes `_threequarter.png`, …)

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

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let prefix = crate::sub_arg(1).unwrap_or_else(|| "target/t54_battle".to_string());
    let width = 1100u32;
    let height = 640u32;
    let aspect = width as f32 / height as f32;

    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
    };

    let mut catalog = VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!("note: no Forge artifacts loaded ({error}); using neutral material");
    }
    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.42, 0.46, 0.34]);
    let render_frame = render_frame_from_objects(objects);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    renderer.scene_lighting = SceneLighting::battlefield_default();
    // Focus the sun-shadow box on the tank (not the camera) so the cast + self shadow are centred.
    renderer.shadow_focus = Some([cx, ground, cz]);
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(&ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(&ctx, handle, &maps);
    }
    renderer.set_vehicle_render_frame(&ctx, &render_frame);

    // With yaw = FRAC_PI_2 the tank's nose points toward +X, so the front cameras sit on the +X
    // side (the earlier -X "front" camera actually photographed the rear).
    let tc = [cx, ground + 1.15, cz];
    let views = [
        ("threequarter", [cx + 5.5, ground + 3.2, cz + 5.0], tc),
        ("side", [cx, ground + 2.0, cz + 8.5], tc),
        // Far enough that the 2.9 m gun overhang (muzzle at z 5.95) doesn't swallow the frame.
        ("front", [cx + 11.0, ground + 1.7, cz], tc),
        ("rear", [cx - 8.5, ground + 1.7, cz], tc),
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
