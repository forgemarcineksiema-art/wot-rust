use std::f32::consts::FRAC_PI_2;
use std::fs::{File, create_dir_all};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use client::{
    VehicleMeshCatalog, battlefield_scene_mesh, render_frame_from_objects, tank_render_objects,
};
use game_core::{TankId, VehicleKind};
use net::TankSnapshot;
use renderer_api::{Camera, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

const WIDTH: u32 = 1800;
const HEIGHT: u32 = 720;
const CENTER_X: f32 = 340.0;
const CENTER_Z: f32 = 300.0;
// 8.5 m keeps the seven-vehicle row inside every authored view's frustum (10 m cropped the
// end vehicles out of the oblique views).
const SPACING: f32 = 8.5;

#[derive(Debug, Clone, Copy)]
struct ViewSpec {
    slug: &'static str,
    eye_offset: [f32; 3],
    target_offset: [f32; 3],
    vertical_fov_degrees: f32,
    hull_yaw_rad: f32,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
}

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = crate::sub_arg(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/vehicle_geometry_views"));
    create_dir_all(&out_dir)?;

    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let (terrain_vertices, terrain_indices) = battlefield_scene_mesh(&battlefield);
    let base = battlefield.heightmap.sample_height(CENTER_X, CENTER_Z).unwrap_or(0.0);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, WIDTH, HEIGHT)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &terrain_vertices, &terrain_indices)?;
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    let mut catalog = VehicleMeshCatalog::default();

    for view in views() {
        let objects = lineup_objects(&mut catalog, view, &battlefield);
        for (handle, mesh) in catalog.take_pending_meshes() {
            renderer.register_mesh(&ctx, handle, &mesh);
        }

        let camera = camera_for(view, base);
        let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            WIDTH as f32 / HEIGHT as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.set_render_frame(&ctx, &render_frame_from_objects(objects));
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

        let path = out_dir.join(format!("{}.png", view.slug));
        write_png(&path, &target.read_rgba8(&ctx)?)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}

fn views() -> &'static [ViewSpec] {
    &[
        ViewSpec {
            slug: "01_front",
            eye_offset: [0.0, 10.5, 46.0],
            target_offset: [0.0, 1.4, 0.0],
            vertical_fov_degrees: 45.0,
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.08,
        },
        ViewSpec {
            slug: "02_rear",
            eye_offset: [0.0, 10.5, -46.0],
            target_offset: [0.0, 1.4, 0.0],
            vertical_fov_degrees: 45.0,
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.08,
        },
        ViewSpec {
            slug: "03_right_profile",
            eye_offset: [0.0, 10.0, 46.0],
            target_offset: [0.0, 1.35, 0.0],
            vertical_fov_degrees: 44.0,
            hull_yaw_rad: FRAC_PI_2,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.05,
        },
        ViewSpec {
            slug: "04_left_profile",
            eye_offset: [0.0, 10.0, 46.0],
            target_offset: [0.0, 1.35, 0.0],
            vertical_fov_degrees: 44.0,
            hull_yaw_rad: -FRAC_PI_2,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.05,
        },
        ViewSpec {
            slug: "05_high_three_quarter",
            eye_offset: [-24.0, 22.0, 32.0],
            target_offset: [0.0, 1.1, 0.0],
            vertical_fov_degrees: 42.0,
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.10,
        },
        ViewSpec {
            slug: "06_top_plan_oblique",
            eye_offset: [0.0, 44.0, 10.0],
            target_offset: [0.0, 0.8, 0.0],
            vertical_fov_degrees: 42.0,
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
        },
        ViewSpec {
            slug: "07_turret_slew_battle_oblique",
            eye_offset: [0.0, 14.0, -44.0],
            target_offset: [0.0, 1.3, 0.0],
            vertical_fov_degrees: 47.0,
            hull_yaw_rad: 2.5,
            turret_yaw_rad: 0.65,
            gun_pitch_rad: 0.20,
        },
    ]
}

fn lineup_objects(
    catalog: &mut VehicleMeshCatalog,
    view: &ViewSpec,
    battlefield: &terrain::BattlefieldMap,
) -> Vec<renderer_api::RenderObject> {
    let palette = [
        [0.34, 0.42, 0.30],
        [0.30, 0.38, 0.46],
        [0.46, 0.40, 0.26],
        [0.44, 0.30, 0.28],
        [0.32, 0.36, 0.40],
        [0.40, 0.34, 0.44],
        [0.28, 0.44, 0.40],
        [0.42, 0.44, 0.28],
    ];

    let mut render_objects = Vec::new();
    let roster = VehicleKind::PLAYABLE;
    let center_index = (roster.len().saturating_sub(1)) as f32 * 0.5;
    for (index, kind) in roster.into_iter().enumerate() {
        let x = CENTER_X + (index as f32 - center_index) * SPACING;
        let ground = battlefield.heightmap.sample_height(x, CENTER_Z).unwrap_or(0.0);
        let snapshot = TankSnapshot {
            tank_id: TankId(index as u64 + 1),
            team: game_core::TeamId(1),
            vehicle: kind,
            position: [x, ground, CENTER_Z],
            yaw_rad: view.hull_yaw_rad,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: view.turret_yaw_rad,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: view.gun_pitch_rad,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: kind.spec().gun.dispersion_mrad,
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
            hull_pitch_velocity_rad_s: 0.0,
            hull_roll_velocity_rad_s: 0.0,
        };
        render_objects.append(&mut tank_render_objects(catalog, &snapshot, palette[index]));
    }
    render_objects
}

fn camera_for(view: &ViewSpec, base: f32) -> Camera {
    Camera {
        eye: [
            CENTER_X + view.eye_offset[0],
            base + view.eye_offset[1],
            CENTER_Z + view.eye_offset[2],
        ],
        target: [
            CENTER_X + view.target_offset[0],
            base + view.target_offset[1],
            CENTER_Z + view.target_offset[2],
        ],
        vertical_fov_degrees: view.vertical_fov_degrees,
    }
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}
