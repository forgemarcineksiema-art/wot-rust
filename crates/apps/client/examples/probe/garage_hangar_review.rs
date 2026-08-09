use std::fs::File;
use std::io::BufWriter;

use client::{
    VehicleAssetCatalog, garage_overlay, garage_overlay_option_list, hangar_camera_pivot,
    hangar_scene_mesh, render_frame_from_objects, tank_vehicle_render_objects,
};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// Render the REAL garage hangar frame for review: workshop lighting, parked T-54 on the
/// turntable, hero orbit framing, full hangar UI overlay — the same composition
/// `render_garage` builds in-game. `cargo run -p client --example probe -- garage_hangar_review -- out.png`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = crate::sub_arg(1).unwrap_or_else(|| "target/garage_hangar_review.png".to_string());
    let width = 1600u32;
    let height = 900u32;
    let aspect = width as f32 / height as f32;

    let (terrain_vertices, terrain_indices) = hangar_scene_mesh();

    // Optional review args: `list <vehicle_index> <slot_index>` renders that vehicle parked with its
    // module option list open (slot 0=Turret,1=Gun,3=Engine,4=Suspension). Default: T-54, no list.
    let show_list = crate::sub_arg(2).as_deref() == Some("list");
    let vehicle_index = crate::sub_arg(3).and_then(|s| s.parse().ok()).unwrap_or(0usize);
    let slot_index = crate::sub_arg(4).and_then(|s| s.parse().ok()).unwrap_or(1usize);
    let vehicle = VehicleKind::PLAYABLE[vehicle_index.min(VehicleKind::PLAYABLE.len() - 1)];

    // Parked hero: same pose as `garage_preview_snapshot`.
    let spec = vehicle.spec();
    let snapshot = TankSnapshot {
        tank_id: TankId(0),
        team: TeamId(1),
        vehicle,
        position: [0.0, client::TURNTABLE_TOP_M, 0.0],
        yaw_rad: scene_build::hangar::HERO_PARK_YAW,
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

    let mut catalog = VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!("note: no Forge artifacts loaded ({error}); using neutral material");
    }
    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.72, 0.76, 0.62]);
    let render_frame = render_frame_from_objects(objects);

    // Hero orbit framing, READ from `scene_build::hangar` rather than copied out of it. This file
    // used to hard-code (0.60, 0.28, 14.0) and a 32 deg lens beside the constants that were moved
    // into one place precisely so a reframing could not miss a caller — which meant the frame a
    // human reviewed here would silently keep the OLD framing after any change to the shared
    // numbers, while the golden moved. That is the same drift that cost the review set its
    // foliage atlas; the fix is to have no second copy to forget.
    let pivot = hangar_camera_pivot();
    let eye = scene_build::hangar::hero_orbit_eye();
    // `slot <name>` reviews a module framing instead of the hero shot — the SAME framing the
    // live camera flies (A3 composed backgrounds), read from the single source.
    let slot_name = (crate::sub_arg(2).as_deref() == Some("slot")).then(|| crate::sub_arg(3));
    let camera = if let Some(name) = slot_name.flatten() {
        let (_, framing) = scene_build::hangar::slot_framings()
            .into_iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("unknown slot framing {name}"));
        let (slot_eye, slot_target) = scene_build::hangar::slot_eye(framing);
        Camera {
            eye: slot_eye.to_array(),
            target: slot_target.to_array(),
            vertical_fov_degrees: scene_build::hangar::HERO_FOV_DEGREES,
        }
    } else {
        Camera {
            eye: eye.to_array(),
            target: pivot.to_array(),
            vertical_fov_degrees: scene_build::hangar::HERO_FOV_DEGREES,
        }
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
    // Match `ensure_scene(SceneKind::Garage)`: hero lighting + interior backdrop, both READ
    // from where the client reads them (a copied literal here locked a night sky through the
    // roof openings the game fills with daylight).
    renderer.scene_lighting = SceneLighting::garage_hero();
    let (bg_r, bg_g, bg_b) = scene_build::hangar::INTERIOR_BACKGROUND;
    renderer.set_interior_background(bg_r, bg_g, bg_b);
    renderer.set_hero_probe(Some(scene_build::hangar::hangar_hero_probe()));
    renderer.set_interior_detail_normal(true);
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(&ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(&ctx, handle, &maps);
    }
    renderer.set_vehicle_render_frame(&ctx, &render_frame);

    let (font_w, font_h, font_coverage) = client::hud_font_atlas();
    renderer.set_hud_font_atlas(&ctx, font_w, font_h, font_coverage);
    let hud = if show_list {
        garage_overlay_option_list(vehicle_index, slot_index, aspect)
    } else {
        garage_overlay(false, aspect)
    };
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
