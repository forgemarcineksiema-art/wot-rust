use std::sync::OnceLock;

use game_core::{TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use renderer_api::{Camera, view_projection_matrix};
use terrain::{HeightMap, StaticCoverObject};

use super::*;

#[test]
fn feedback_marks_terrain_blocking_the_muzzle_line() {
    let heightmap = ridge_heightmap();
    let muzzle = Vec3::new(20.0, 2.0, 5.0);
    let aim = Vec3::new(20.0, 0.0, 35.0);

    let feedback = reticle_feedback(query(&heightmap, &[], &[], muzzle, aim, 0.0, 0.0));

    assert_eq!(feedback.status, ReticleStatus::Blocked);
    assert!(feedback.actual_impact_world_point.z < aim.z, "impact should stop before the target");
}

#[test]
fn feedback_is_clear_when_current_gun_arc_lands_near_the_aim_point() {
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 1.15, 40.0);
    let aim = Vec3::new(40.0, 0.0, 140.0);
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);

    let feedback = reticle_feedback(query(&heightmap, &[], &[], muzzle, aim, 0.0, pitch));

    assert_eq!(feedback.status, ReticleStatus::Clear);
    assert!(feedback.actual_impact_world_point.distance(aim) < 4.0);
}

#[test]
fn world_points_project_to_hud_clip_coordinates() {
    let camera =
        Camera { eye: [0.0, 1.0, -10.0], target: [0.0, 1.0, 0.0], vertical_fov_degrees: 60.0 };
    let view_proj = view_projection_matrix(&camera, 16.0 / 9.0, 0.5, 2000.0);

    let clip = world_to_clip_xy(Vec3::new(0.0, 1.0, 10.0), view_proj).expect("visible");

    assert!(clip[0].abs() < 1.0e-4);
    assert!(clip[1].abs() < 1.0e-4);
}

#[test]
fn feedback_marks_static_cover_blocking_the_shell_path() {
    let heightmap = HeightMap::flat(80, 80, 5.0, -50.0).unwrap();
    let muzzle = Vec3::new(40.0, 2.0, 40.0);
    let aim = Vec3::new(40.0, 1.0, 140.0);
    let cover = terrain::StaticCoverObject {
        id: "wall".to_string(),
        name: "wall".to_string(),
        kind: terrain::StaticCoverKind::FarmBuilding,
        center: [40.0, 2.0, 75.0],
        half_extents_m: [4.0, 3.0, 2.0],
    };
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);

    let feedback = reticle_feedback(query(
        &heightmap,
        std::slice::from_ref(&cover),
        &[],
        muzzle,
        aim,
        0.0,
        pitch,
    ));

    assert_eq!(feedback.status, ReticleStatus::Blocked);
    assert!(feedback.actual_impact_world_point.z < aim.z);
}

#[test]
fn feedback_marks_tank_hit_before_the_terrain_aim_point() {
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 1.78, 40.0);
    let aim = Vec3::new(40.0, 0.0, 140.0);
    let target = tank_snapshot(TankId(2), [40.0, 0.0, 82.0]);
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);

    let feedback = reticle_feedback(query(
        &heightmap,
        &[],
        std::slice::from_ref(&target),
        muzzle,
        aim,
        0.0,
        pitch,
    ));

    assert_eq!(feedback.status, ReticleStatus::Blocked);
    assert!(feedback.actual_impact_world_point.z < aim.z);
}

#[test]
fn penetration_hint_reads_the_player_shell_not_the_target_shell() {
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 1.78, 40.0);
    let aim = Vec3::new(40.0, 0.0, 140.0);
    let target = tank_snapshot(TankId(2), [40.0, 0.0, 82.0]);
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);

    // High-penetration gun vs. low-penetration gun; only the *firing* vehicle differs between the
    // two queries — the target armor and geometry are identical.
    let jagdtiger = VehicleKind::Jagdtiger.spec();
    let panther = VehicleKind::PantherII.spec();
    let with_jagdtiger = penetration_hint(query_with_player(
        &heightmap,
        std::slice::from_ref(&target),
        muzzle,
        aim,
        pitch,
        &jagdtiger,
    ))
    .expect("shell reaches the target");
    let with_panther = penetration_hint(query_with_player(
        &heightmap,
        std::slice::from_ref(&target),
        muzzle,
        aim,
        pitch,
        &panther,
    ))
    .expect("shell reaches the target");

    // The reported shell penetration must follow the player's gun. Before the fix both queries
    // read the target's shell and produced the same number; the only input that differs here is
    // `player_spec`, so the change in `shell_pen_mm` can only come from reading the player's shell.
    assert!(
        with_jagdtiger.shell_pen_mm > with_panther.shell_pen_mm,
        "penetration hint must reflect the firing vehicle's shell ({} vs {})",
        with_jagdtiger.shell_pen_mm,
        with_panther.shell_pen_mm,
    );
    // Same target armor faced regardless of who shoots it.
    assert_eq!(with_jagdtiger.armor_mm, with_panther.armor_mm);
    assert_eq!(with_jagdtiger.facing, with_panther.facing);
}

fn ridge_heightmap() -> HeightMap {
    let mut samples = vec![0.0; 5 * 5];
    for x in 0..5 {
        samples[2 * 5 + x] = 8.0;
    }
    HeightMap::new(5, 5, 10.0, samples).unwrap()
}

fn tank_snapshot(tank_id: TankId, position: [f32; 3]) -> TankSnapshot {
    let spec = game_core::VehicleKind::T54_1951.spec();
    TankSnapshot {
        tank_id,
        team: TeamId(2),
        vehicle: spec.kind,
        position,
        yaw_rad: std::f32::consts::PI,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 1000,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 2.9,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
    }
}

/// Shared spec for tests that exercise `reticle_feedback`, which ignores `player_spec`.
fn default_spec() -> &'static TankSpec {
    static SPEC: OnceLock<TankSpec> = OnceLock::new();
    SPEC.get_or_init(|| VehicleKind::T54_1951.spec())
}

fn query<'a>(
    heightmap: &'a HeightMap,
    cover: &'a [StaticCoverObject],
    tanks: &'a [TankSnapshot],
    muzzle: Vec3,
    aim: Vec3,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
) -> ReticleFeedbackQuery<'a> {
    ReticleFeedbackQuery {
        heightmap,
        cover,
        tanks,
        player_spec: default_spec(),
        owner: TankId(1),
        owner_team: TeamId(1),
        muzzle,
        aim,
        gun_direction: game_core::math::gun_direction(turret_yaw_rad, gun_pitch_rad),
        muzzle_velocity_mps: 895.0,
        drag_per_s: 0.09,
    }
}

/// Same trajectory as [`query`] — only the firing spec differs, so penetration hints read the
/// player's shell while the flight stays identical.
fn query_with_player<'a>(
    heightmap: &'a HeightMap,
    tanks: &'a [TankSnapshot],
    muzzle: Vec3,
    aim: Vec3,
    gun_pitch_rad: f32,
    player_spec: &'a TankSpec,
) -> ReticleFeedbackQuery<'a> {
    ReticleFeedbackQuery {
        player_spec,
        ..query(heightmap, &[], tanks, muzzle, aim, 0.0, gun_pitch_rad)
    }
}

#[test]
fn an_open_sky_shot_is_not_blocked_just_targetless() {
    // Aiming above the horizon over a flat map: the trace expires in flight with nothing hit.
    // That is a shot with no target — NOT a blocked shot. Flagging the whole sky "blocked"
    // teaches players to ignore the one signal that saves a wasted shell.
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 2.0, 40.0);
    let aim = muzzle + Vec3::new(0.0, 200.0, 1000.0); // well above the horizon

    let feedback = reticle_feedback(query(&heightmap, &[], &[], muzzle, aim, 0.0, 0.35));

    assert_eq!(feedback.status, ReticleStatus::Clear);
}
