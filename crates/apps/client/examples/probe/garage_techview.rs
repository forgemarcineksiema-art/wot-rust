use std::fs::File;
use std::io::BufWriter;

use client::{garage_overlay, hangar_camera_pivot, hangar_scene_mesh};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// Render the garage in the browse-only tech tree view and save a PNG. This exercises the
/// tech-tree overlay (nation columns + vehicle nodes) headlessly for visual review.
/// `cargo run -p client --example probe -- garage_techview -- out.png`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = crate::sub_arg(1).unwrap_or_else(|| "target/garage_techview.png".to_string());
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
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
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
    };
    let _ = snapshot; // The tech tree view does not render the vehicle mesh; it's an overlay-only
    // screen over the dim hangar. Kept here so the example stays close to the
    // real garage frame structure.

    // Hero orbit camera, READ from the constants the live garage rests at rather than copied
    // from them. This probe carried its own yaw/pitch/distance/FOV, and when the hero pitch was
    // lowered 0.28 -> 0.13 (the D20 relight, which exists because the old lens pointed under
    // every light in the room) the probe kept shooting the retired framing: a review artifact
    // showing a picture the game had stopped taking.
    let pivot = hangar_camera_pivot();
    let camera = Camera {
        eye: client::hero_orbit_eye().to_array(),
        target: pivot.to_array(),
        vertical_fov_degrees: client::HERO_FOV_DEGREES,
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
    // Match the real garage frame (`ensure_scene`): hero lighting + the interior backdrop the game
    // uses, so this review shot never flatters the scene differently than the player sees it.
    renderer.scene_lighting = SceneLighting::garage_hero();
    let (bg_r, bg_g, bg_b) = client::INTERIOR_BACKGROUND;
    renderer.set_interior_background(bg_r, bg_g, bg_b);

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
