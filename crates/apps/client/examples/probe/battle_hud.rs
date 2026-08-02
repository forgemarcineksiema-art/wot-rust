//! Offscreen staged battle HUD: renders the same battlefield frame twice — once with the neutral
//! third-person reticle, once with the informative sniper reticle — over a fully-populated HUD
//! (ammo panel, dealt/taken damage log, an incoming-hit arc). A visual QA aid for the HUD art
//! direction. `cargo run -p client --example probe -- battle_hud`

use std::fs::File;
use std::io::BufWriter;

use client::{
    BattleCameraController, BattleCameraEnvironment, CameraSubject, append_tank_mesh,
    battlefield_scene_mesh, demo_battle_hud,
};
use net::ClientInputCommand;
use renderer_api::view_projection_matrix;
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use server::{LocalAuthoritativeServer, ServerTickConfig};
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
    let environment = BattleCameraEnvironment::for_battlefield(&battlefield);
    let mut camera_controller = BattleCameraController::default();
    camera_controller.set_orbit_yaw(player_tank.yaw_rad);
    let camera = camera_controller.render_camera(&subject, &environment);
    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        aspect,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for tank in &snapshot.tanks {
        let color = if tank.tank_id == player { [0.30, 0.40, 0.28] } else { [0.46, 0.29, 0.25] };
        append_tank_mesh(&mut vertices, &mut indices, tank, color);
    }

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    let (font_w, font_h, font_coverage) = client::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);
    renderer.set_dynamic_mesh(&ctx, &vertices, &indices);

    for (sniper, name) in [(false, "battle_hud_third_person"), (true, "battle_hud_sniper")] {
        renderer.set_hud(&ctx, &demo_battle_hud(sniper, aspect));
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
