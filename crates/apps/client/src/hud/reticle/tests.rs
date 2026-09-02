use std::sync::OnceLock;

use game_core::math::HullPose;
use game_core::{TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use net::TankSnapshot;
use renderer_api::{Camera, view_projection_matrix};
use terrain::{HeightMap, StaticCoverObject};

use super::*;

/// A ridge the shell actually dies on. (Named for the muzzle-to-aim CHORD once, back when a
/// straight line sampled against the heightmap could veto the trace; the chord is gone and the
/// ridge is now judged by the arc that hits it.)
#[test]
fn feedback_marks_a_ridge_the_shell_dies_on() {
    let heightmap = ridge_heightmap();
    let muzzle = Vec3::new(20.0, 2.0, 5.0);
    let aim = Vec3::new(20.0, 0.0, 35.0);

    let feedback = reticle_feedback(query(&heightmap, &[], &[], muzzle, aim, 0.0, 0.0));

    assert_eq!(feedback.status, ReticleStatus::Blocked);
    assert!(feedback.actual_impact_world_point.z < aim.z, "impact should stop before the target");
}

/// A refusal has to name its cause. "Blocked" alone taught nothing: the range readout beside it
/// keeps answering "how far is what I am pointing at", which while blocked is a distance to
/// something this gun cannot reach — so the only number on screen was the one number that could
/// not be acted on (register I2). The feedback now carries the other one: metres to whatever eats
/// the round.
#[test]
fn a_blocked_shot_reports_how_far_it_actually_gets() {
    let heightmap = ridge_heightmap();
    let muzzle = Vec3::new(20.0, 2.0, 5.0);
    let aim = Vec3::new(20.0, 0.0, 35.0);

    let feedback = reticle_feedback(query(&heightmap, &[], &[], muzzle, aim, 0.0, 0.0));

    assert_eq!(feedback.status, ReticleStatus::Blocked);
    let block = feedback.block_distance_m.expect("a blocked shot must say where it dies");
    assert!(
        (block - feedback.actual_impact_world_point.distance(muzzle)).abs() < 1.0e-3,
        "the number is the range to the real impact, got {block}"
    );
    assert!(
        block < aim.distance(muzzle),
        "and it is SHORT of the crosshair — {block} against a {} m sight point",
        aim.distance(muzzle)
    );
}

/// An arriving shot has nothing to explain, and neither has one that sails PAST the sight point:
/// a round that expires downrange was obstructed by nothing at all, so its range is a fact about
/// the shell's lifetime rather than about the battlefield. Printing it would put a number on the
/// screen that answers no question the player asked.
#[test]
fn only_a_shot_stopped_short_prints_a_block_range() {
    let flat = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 1.15, 40.0);
    let aim = Vec3::new(40.0, 0.0, 140.0);
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);
    let arriving = reticle_feedback(query(&flat, &[], &[], muzzle, aim, 0.0, pitch));
    assert_eq!(arriving.status, ReticleStatus::Clear);
    assert_eq!(arriving.block_distance_m, None, "an arriving shot explains nothing");

    // Out of arc: the gun cannot depress to the sight point, so the round leaves flatter than
    // asked and comes down well BEYOND it. Blocked, correctly — and with nothing to point at.
    let steep = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let over = reticle_feedback(query(
        &steep,
        &[],
        &[],
        Vec3::new(40.0, 4.0, 40.0),
        Vec3::new(40.0, 0.0, 60.0),
        0.0,
        -0.14,
    ));
    assert_eq!(over.status, ReticleStatus::Blocked);
    assert!(
        over.actual_impact_world_point.distance(Vec3::new(40.0, 4.0, 40.0))
            > Vec3::new(40.0, 0.0, 60.0).distance(Vec3::new(40.0, 4.0, 40.0)),
        "the shot really does land beyond the sight point"
    );
    assert_eq!(over.block_distance_m, None, "an over-shot was obstructed by nothing");
}

#[test]
fn feedback_is_clear_when_current_gun_arc_lands_near_the_aim_point() {
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 1.15, 40.0);
    let aim = Vec3::new(40.0, 0.0, 140.0);
    let pitch = crate::aim::gun_pitch_to_hit(muzzle, aim, 895.0, 0.09);

    let feedback = reticle_feedback(query(&heightmap, &[], &[], muzzle, aim, 0.0, pitch));

    assert_eq!(
        feedback.status,
        ReticleStatus::Clear,
        "aim={aim:?}, impact={:?}, distance={}",
        feedback.actual_impact_world_point,
        feedback.actual_impact_world_point.distance(aim)
    );
    let fired = (aim - muzzle).normalize();
    assert!(
        feedback.actual_impact_world_point.distance(aim)
            < aim_match_tolerance_m(fired, muzzle.distance(aim), 895.0),
        "a nearly flat shot grazes the ground metres early — that IS arriving"
    );
}

/// The gun arc is judged in the HULL frame, where the simulation's own limits live. A tank nosed
/// down on a ridge reaches further below the horizon than its documented depression suggests —
/// that extra reach IS hull-down — and the sight must promise the shot the gun can take from the
/// slope it is standing on, not from an imaginary level one.
#[test]
fn the_gun_arc_is_judged_on_the_hull_the_tank_is_standing_on() {
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    // Four metres down over twenty: about -0.197 rad, past the level arc's -0.14 floor.
    let muzzle = Vec3::new(40.0, 4.0, 40.0);
    let aim = Vec3::new(40.0, 0.0, 60.0);

    let level = query(&heightmap, &[], &[], muzzle, aim, 0.0, -0.14);
    assert_eq!(
        reticle_feedback(level).status,
        ReticleStatus::Blocked,
        "a level hull genuinely cannot depress that far"
    );

    let nosed_down = ReticleFeedbackQuery {
        hull_pose: HullPose { yaw_rad: 0.0, pitch_rad: -0.20, roll_rad: 0.0 },
        ..level
    };
    let feedback = reticle_feedback(nosed_down);
    assert_eq!(
        feedback.status,
        ReticleStatus::Clear,
        "nose-down, the same sight point sits inside the gun's arc — impact {:?}",
        feedback.actual_impact_world_point
    );
}

/// The arrival window is set by the ARRIVAL ANGLE. It used to be one flat 4.5 m constant, which
/// priced the grazing case into every shot — including the thirty-metre street corner, where
/// 4.5 m is the whole difference between the window and the wall.
#[test]
fn the_arrival_window_follows_the_arrival_angle_not_the_range() {
    // A plunging shot lands where it was sent; a grazing one touches down metres early through
    // the same centimetres of vertical slack. The window has to know the difference.
    let steep = aim_match_tolerance_m(Vec3::new(0.0, -0.3, 1.0).normalize(), 300.0, 895.0);
    let grazing = aim_match_tolerance_m(Vec3::new(0.0, -0.012, 1.0).normalize(), 300.0, 895.0);
    assert!(steep < 1.0, "a steep arrival is judged in centimetres, got {steep}");
    assert!(grazing > 3.0, "a flat arrival must be allowed its graze, got {grazing}");

    // A slab two metres short of a thirty-metre sight point stops the shell dead.
    let heightmap = HeightMap::flat(80, 80, 5.0, 0.0).unwrap();
    let muzzle = Vec3::new(40.0, 2.0, 40.0);
    let aim = Vec3::new(40.0, 0.0, 70.0);
    let wall = StaticCoverObject {
        id: "wall".to_string(),
        name: "wall".to_string(),
        kind: terrain::StaticCoverKind::FarmBuilding,
        center: [40.0, 1.0, 68.5],
        half_extents_m: [4.0, 1.5, 0.5],
    };

    let feedback = reticle_feedback(query(
        &heightmap,
        std::slice::from_ref(&wall),
        &[],
        muzzle,
        aim,
        0.0,
        0.0,
    ));

    assert_eq!(feedback.status, ReticleStatus::Blocked);
    assert!(
        feedback.actual_impact_world_point.distance(aim) < 4.5,
        "and it stops INSIDE the old flat window, which called this shot clear"
    );
}

/// A crest the straight muzzle->aim line grazes is not a blocked shot: the shell flies a BALLISTIC
/// ARC, which rides above that chord for the whole flight. The sight used to sample the chord
/// against the heightmap and veto the trace with it — so a slow shell over a low ridge, the
/// classic indirect lob, read as "no shot" while the round would have sailed over.
#[test]
fn a_crest_the_arc_flies_over_is_not_blocked() {
    // 400 m/s over 600 m: about 1.5 s of flight, so the arc peaks roughly 2.7 m above the chord.
    let mut samples = vec![0.0; 70 * 70];
    for x in 0..70 {
        samples[35 * 70 + x] = 1.5; // a 1.5 m crest at mid-range; the chord there sits at 1.0
    }
    let heightmap = HeightMap::new(70, 70, 10.0, samples).unwrap();
    let muzzle = Vec3::new(300.0, 2.0, 50.0);
    let aim = Vec3::new(300.0, 0.0, 650.0);

    let feedback = reticle_feedback(ReticleFeedbackQuery {
        muzzle_velocity_mps: 400.0,
        ..query(&heightmap, &[], &[], muzzle, aim, 0.0, 0.0)
    });

    assert_eq!(
        feedback.status,
        ReticleStatus::Clear,
        "the arc clears the crest — impact {:?}",
        feedback.actual_impact_world_point
    );
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

pub(super) fn tank_snapshot(tank_id: TankId, position: [f32; 3]) -> TankSnapshot {
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
        gun_pitch_limits_rad: (sim::MIN_GUN_PITCH_RAD, sim::MAX_GUN_PITCH_RAD),
        hull_pose: HullPose { yaw_rad: 0.0, pitch_rad: 0.0, roll_rad: 0.0 },
        heightmap,
        cover,
        water: terrain::WaterView::DRY,
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

/// Inny Poziom A4: a casemate's sight is honest about its hull line. On the line the shot
/// arrives and the sight is CLEAR; 30° off it the shell leaves down the hull line anyway —
/// the sim forces the yaw — so the sight is BLOCKED and names the traverse limit, instead of
/// the green it used to show while the round left for the wrong bearing.
#[test]
fn a_casemates_sight_is_blocked_off_the_hull_line_and_clear_on_it() {
    let heightmap = HeightMap::flat(200, 200, 5.0, 0.0).unwrap();
    let jagdtiger = VehicleKind::Jagdtiger.spec();
    assert!(jagdtiger.has_fixed_casemate(), "precondition: the Jagdtiger is a casemate");
    let muzzle = Vec3::new(500.0, 2.0, 100.0);
    let on_line = tank_snapshot(TankId(2), [500.0, 0.0, 400.0]);
    let off_line = tank_snapshot(TankId(3), [500.0 + 300.0 * 0.5, 0.0, 100.0 + 300.0 * 0.866]);

    let clear = reticle_report(query_with_player(
        &heightmap,
        std::slice::from_ref(&on_line),
        muzzle,
        Vec3::new(500.0, 1.4, 400.0),
        0.0,
        &jagdtiger,
    ));
    assert_eq!(clear.feedback.status, ReticleStatus::Clear, "on the hull line the shot arrives");
    assert_eq!(clear.feedback.arc_limit, None);

    let blocked = reticle_report(query_with_player(
        &heightmap,
        std::slice::from_ref(&off_line),
        muzzle,
        Vec3::new(500.0 + 300.0 * 0.5, 1.4, 100.0 + 300.0 * 0.866),
        0.0,
        &jagdtiger,
    ));
    assert_eq!(
        blocked.feedback.status,
        ReticleStatus::Blocked,
        "30° off the hull line the casemate's round does not arrive"
    );
    assert_eq!(blocked.feedback.arc_limit, Some(crate::aim::ArcLimit::Traverse));
}
