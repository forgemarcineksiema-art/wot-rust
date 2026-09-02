//! Offscreen staged battle HUD: renders the same battlefield frame twice — once with the neutral
//! third-person reticle, once with the informative sniper reticle — over a fully-populated HUD
//! (ammo panel, dealt/taken damage log, an incoming-hit arc); the sniper frame lays the gun on
//! the nearest enemy and brackets every enemy in the scope (Inny Poziom A9). A visual QA aid
//! for the HUD art direction. `cargo run -p client --example probe -- battle_hud`

use std::fs::File;
use std::io::BufWriter;

use battle_host::{LocalAuthoritativeServer, ServerTickConfig};
use client::{
    BattleCameraController, BattleCameraEnvironment, BattleCameraMode, CameraSubject,
    append_tank_mesh, battlefield_scene_mesh, demo_battle_hud, spot_bracket_for_hull,
};
use net::ClientInputCommand;
use renderer_api::view_projection_matrix;
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use sim::TankCommand;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    let aspect = width as f32 / height as f32;

    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);

    // Drive a few ticks so the scene reads as a real battle, not a spawn pose.
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::default());
    let player = server.player_tank();
    for tick in 0..90u64 {
        let command = TankCommand { throttle: 1.0, steer: 0.3, ..TankCommand::idle() };
        server.tick_with_input(ClientInputCommand { client_tick: tick, tank_id: player, command });
    }
    let snapshot = server.latest_snapshot();
    let player_tank = snapshot.tanks.iter().find(|t| t.tank_id == player).expect("player");

    let subject = CameraSubject::from_snapshot(player_tank.clone(), 0.0);
    // The sniper frame looks AT something: lay the gun on the nearest enemy so the scope
    // holds a hull for the bracket to mark (a review aid — the battle lays its own gun).
    let mut aimed = player_tank.clone();
    if let Some(nearest) =
        snapshot.tanks.iter().filter(|t| t.tank_id != player && t.team != player_tank.team).min_by(
            |a, b| {
                let da = glam::Vec3::from_array(a.position)
                    .distance(glam::Vec3::from_array(player_tank.position));
                let db = glam::Vec3::from_array(b.position)
                    .distance(glam::Vec3::from_array(player_tank.position));
                da.total_cmp(&db)
            },
        )
    {
        let to =
            glam::Vec3::from_array(nearest.position) - glam::Vec3::from_array(player_tank.position);
        // Lay the gun through the same hull-frame conversion the client's gun-laying uses,
        // so the scope (which follows the gun) holds the hull, pitch included.
        let (yaw, pitch) = game_core::math::world_direction_to_turret(player_tank.hull_pose(), to);
        aimed.turret_yaw_rad = yaw;
        aimed.gun_pitch_rad = pitch;
        println!(
            "battle_hud sniper: nearest enemy {:?} at {:.1} m, laid yaw {:.3} pitch {:.3}",
            nearest.tank_id,
            to.length(),
            yaw,
            pitch
        );
    }
    let sniper_subject = CameraSubject::from_snapshot(aimed, 0.0);
    let environment = BattleCameraEnvironment::for_battlefield(&battlefield);
    let mut camera_controller = BattleCameraController::default();
    camera_controller.set_orbit_yaw(player_tank.yaw_rad);
    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for tank in &snapshot.tanks {
        let color = if tank.tank_id == player { [0.30, 0.40, 0.28] } else { [0.46, 0.29, 0.25] };
        append_tank_mesh(&mut vertices, &mut indices, tank, color);
    }

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    let (font_w, font_h, font_coverage) = client::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);
    renderer.set_dynamic_mesh(&ctx, &vertices, &indices);

    for (sniper, name) in [(false, "battle_hud_third_person"), (true, "battle_hud_sniper")] {
        // The sniper frame is the real scope: the sniper eye and field on the nearest enemy,
        // and every enemy in it wearing the A9 corner bracket (all of them, as a review aid —
        // the game gates the bracket on the server's spotting bit).
        camera_controller.set_mode(if sniper {
            BattleCameraMode::Sniper
        } else {
            BattleCameraMode::ThirdPerson
        });
        let framed = if sniper { &sniper_subject } else { &subject };
        let camera = camera_controller.render_camera(framed, &environment);
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        let mut hud = demo_battle_hud(sniper, aspect);
        if sniper {
            for tank in
                snapshot.tanks.iter().filter(|t| t.tank_id != player && t.team != player_tank.team)
            {
                spot_bracket_for_hull(
                    tank.position,
                    tank.yaw_rad,
                    tank.vehicle,
                    view_proj,
                    aspect,
                    &mut hud,
                );
            }
        }
        renderer.set_hud(&ctx, &hud);
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/{name}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path} ({width}x{height})");
    }
    Ok(())
}
