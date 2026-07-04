use std::fs::File;
use std::io::BufWriter;

use client::{
    BattleCameraController, BattleCameraEnvironment, CameraSubject, HudVitals, append_tank_mesh,
    battlefield_scene_mesh, build_hud, shell_tracer_vertices,
};
use net::ClientInputCommand;
use renderer_api::view_projection_matrix;
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use server::{LocalAuthoritativeServer, ServerTickConfig};
use sim::TankCommand;
use terrain::prokhorovka_hill_252_2;

/// Drive the authoritative server, then render the resulting battle offscreen and save a
/// PNG. This exercises the real, playable systems (terrain-following driving + a fired
/// shell) headlessly. `cargo run -p client --example screenshot -- out.png`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "target/scene.png".to_string());
    let width = 1280u32;
    let height = 720u32;

    let battlefield = prokhorovka_hill_252_2();
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);

    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::default());
    let player = server.player_tank();
    let total_ticks = 150u64;
    for tick in 0..total_ticks {
        let command = TankCommand {
            throttle: 1.0,
            steer: 0.5,
            brake: 0.0,
            turret_yaw_delta: 0.18,
            gun_pitch_delta: 0.0,
            fire: tick == total_ticks - 2,
        };
        server.tick_with_input(ClientInputCommand { client_tick: tick, tank_id: player, command });
    }
    let snapshot = server.latest_snapshot();
    let player_tank = snapshot.tanks.iter().find(|tank| tank.tank_id == player).expect("player");
    println!("player at {:?}, shells in flight: {}", player_tank.position, snapshot.shells.len());

    let subject = CameraSubject::from_snapshot(player_tank.clone(), 0.0);
    let environment = BattleCameraEnvironment::for_battlefield(&battlefield);
    let mut camera_controller = BattleCameraController::default();
    camera_controller.set_orbit_yaw(player_tank.yaw_rad);
    let camera = camera_controller.render_camera(&subject, &environment);
    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        width as f32 / height as f32,
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
    // The in-flight shell draws as an additive tracer streak through the FX pass, exactly like
    // the live client — this exercises the offscreen FX path end to end.
    renderer.set_fx(&ctx, &shell_tracer_vertices(&snapshot.shells, camera.eye));
    let player_spec = player_tank.vehicle.spec();
    let hud = build_hud(
        HudVitals {
            hit_points: player_tank.hit_points,
            max_hit_points: player_spec.hit_points,
            reload_remaining_s: player_tank.reload_remaining_s,
            reload_seconds: player_spec.gun.reload_seconds,
        },
        width as f32 / height as f32,
    );
    renderer.set_hud(&ctx, &hud);
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
