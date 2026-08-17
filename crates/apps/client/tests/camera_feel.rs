//! Camera FEEL locks, split from `battle_camera.rs` for the file budget: the follow rig's
//! response to shots, teleports, terrain bounce, and the sniper eye's attitude + damping.

use client::{
    BattleCameraController, BattleCameraEnvironment, BattleCameraMode, BattleCameraSettings,
    CameraSubject,
};
use game_core::TankId;
use net::TankSnapshot;
use terrain::HeightMap;

fn tank_snapshot(position: [f32; 3], hull_yaw_rad: f32, turret_yaw_rad: f32) -> TankSnapshot {
    let spec = game_core::VehicleKind::PrototypeMedium.spec();
    TankSnapshot {
        tank_id: TankId(1),
        team: game_core::TeamId(1),
        vehicle: spec.kind,
        position,
        yaw_rad: hull_yaw_rad,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
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
    }
}

#[test]
fn the_players_shot_nudges_the_third_person_rig_and_leaves_sniper_rigid() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let position = [20.0, 0.0, 20.0];
    let subject = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0);

    // Third person: settle the follow rig, then fire — the eye must dip back/down and recover.
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);
    for _ in 0..240 {
        camera.advance(position, 0.0, 1.0 / 60.0);
    }
    let settled = camera.render_camera(&subject, &environment).eye;
    camera.fire_kick(subject.view_yaw_rad);
    let mut max_back = 0.0_f32;
    let mut max_down = 0.0_f32;
    for _ in 0..30 {
        camera.advance(position, 0.0, 1.0 / 60.0);
        let eye = camera.render_camera(&subject, &environment).eye;
        // view_yaw 0 faces +Z: the kick pushes the rig toward -Z and down.
        max_back = max_back.max(settled[2] - eye[2]);
        max_down = max_down.max(settled[1] - eye[1]);
    }
    assert!(max_back > 0.01, "the shot visibly nudges the rig back, got {max_back}");
    assert!(max_down > 0.005, "and settles it slightly down, got {max_down}");
    for _ in 0..240 {
        camera.advance(position, 0.0, 1.0 / 60.0);
    }
    let recovered = camera.render_camera(&subject, &environment).eye;
    assert!((recovered[2] - settled[2]).abs() < 0.01, "the rig recovers to its settle");

    // Sniper: the same kick must not move the eye at all — aiming tolerates no theatrics.
    let mut sniper = BattleCameraController::new(BattleCameraSettings::default());
    sniper.set_mode(BattleCameraMode::Sniper);
    sniper.advance(position, 0.0, 1.0 / 60.0);
    let before = sniper.render_camera(&subject, &environment).eye;
    sniper.fire_kick(subject.view_yaw_rad);
    sniper.advance(position, 0.0, 1.0 / 60.0);
    let after = sniper.render_camera(&subject, &environment).eye;
    assert_eq!(before, after, "sniper eye stays rigid through the shot");
}

/// Taking a hit must be FELT: a directional shove of the third-person rig that recovers on the
/// spring, and in sniper a strictly vertical scope dip bounded by the micro-damper — the jolt
/// reads, but the aim never smears sideways.
#[test]
fn an_incoming_hit_rocks_the_rig_and_dips_the_sniper_scope_vertically() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let position = [20.0, 0.0, 20.0];
    let subject = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0);

    // Third person: the shove displaces the settled rig along the push and down, then recovers.
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);
    for _ in 0..240 {
        camera.advance(position, 0.0, 1.0 / 60.0);
    }
    let settled = camera.render_camera(&subject, &environment).eye;
    camera.damage_shudder(glam::Vec3::new(0.0, 0.0, -1.0), 0.2);
    let mut max_shift = 0.0_f32;
    let mut max_down = 0.0_f32;
    for _ in 0..30 {
        camera.advance(position, 0.0, 1.0 / 60.0);
        let eye = camera.render_camera(&subject, &environment).eye;
        max_shift = max_shift.max(settled[2] - eye[2]);
        max_down = max_down.max(settled[1] - eye[1]);
    }
    assert!(max_shift > 0.01, "the hit visibly shoves the rig along the push, got {max_shift}");
    assert!(max_down > 0.005, "and settles it slightly down, got {max_down}");
    for _ in 0..240 {
        camera.advance(position, 0.0, 1.0 / 60.0);
    }
    let recovered = camera.render_camera(&subject, &environment).eye;
    assert!((recovered[2] - settled[2]).abs() < 0.01, "the rig recovers to its settle");

    // Sniper: the dip is vertical only and bounded — x/z of the eye must not move at all.
    let mut sniper = BattleCameraController::new(BattleCameraSettings::default());
    sniper.set_mode(BattleCameraMode::Sniper);
    for _ in 0..60 {
        sniper.advance(position, 0.0, 1.0 / 60.0);
    }
    let before = sniper.render_camera(&subject, &environment).eye;
    sniper.damage_shudder(glam::Vec3::new(1.0, 0.0, 0.0), 1.0);
    let mut max_dip = 0.0_f32;
    for _ in 0..30 {
        sniper.advance(position, 0.0, 1.0 / 60.0);
        let eye = sniper.render_camera(&subject, &environment).eye;
        assert_eq!(eye[0], before[0], "a hit must not smear the sniper aim in x");
        assert_eq!(eye[2], before[2], "a hit must not smear the sniper aim in z");
        max_dip = max_dip.max(before[1] - eye[1]);
    }
    assert!(max_dip > 0.005, "the scope visibly dips, got {max_dip}");
    assert!(max_dip <= 0.121, "the micro-damper bounds the dip, got {max_dip}");
}

#[test]
fn terrain_bounce_does_not_pulse_the_fov_but_real_speed_widens_it() {
    let environment = BattleCameraEnvironment::empty();
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, 0.0), 0.0);
    let base_fov = BattleCameraSettings::default().third_person_fov_degrees;

    // Bouncing in place (vertical hull motion, zero rigid-body speed): the FOV must not open.
    // The speed is a tick-domain input now, so presented-position noise cannot reach the cue.
    let mut parked = BattleCameraController::new(BattleCameraSettings::default());
    for frame in 0..240 {
        let y = if frame % 2 == 0 { 0.0 } else { 0.4 }; // violent 24 m/s vertical jitter
        parked.advance([0.0, y, 0.0], 0.0, 1.0 / 60.0);
    }
    let jittered = parked.render_camera(&subject, &environment).vertical_fov_degrees;
    assert!((jittered - base_fov).abs() < 0.05, "vertical jitter is not speed, got {jittered}");

    // Real horizontal cruise: the FOV opens up.
    let mut cruising = BattleCameraController::new(BattleCameraSettings::default());
    let mut z = 0.0;
    for _ in 0..240 {
        z += 14.0 / 60.0;
        cruising.advance([0.0, 0.0, z], 14.0, 1.0 / 60.0);
    }
    let opened = cruising
        .render_camera(
            &CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, z], 0.0, 0.0), 0.0),
            &environment,
        )
        .vertical_fov_degrees;
    assert!(opened > base_fov + 1.0, "cruise speed widens the view, got {opened}");
}

#[test]
fn a_spawn_teleport_cannot_outrun_the_follow_anchor() {
    let environment = BattleCameraEnvironment::empty();
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    for _ in 0..120 {
        camera.advance([0.0, 0.0, 0.0], 0.0, 1.0 / 60.0);
    }
    let settled = camera
        .render_camera(
            &CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, 0.0), 0.0),
            &environment,
        )
        .eye[2];
    // The hull teleports 50 m; one frame later the eye must be within the clamped lag of the
    // new position, not stranded at the old one easing over.
    camera.advance([0.0, 0.0, 50.0], 0.0, 1.0 / 60.0);
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 50.0], 0.0, 0.0), 0.0);
    let eye = camera.render_camera(&subject, &environment).eye;
    assert!(
        eye[2] - settled > 49.0,
        "the anchor lag is clamped to well under a meter: eye advanced {}",
        eye[2] - settled
    );
}

#[test]
fn sniper_eye_rides_the_hull_attitude_and_damps_only_the_vertical_jolt() {
    let environment = BattleCameraEnvironment::empty();

    // Attitude: a nose-up hull must move the eye (the optics ride the tank, not float level).
    let level = CameraSubject::from_snapshot(tank_snapshot([10.0, 2.0, 10.0], 0.0, 0.0), 0.0);
    let mut pitched = level;
    pitched.hull_pitch_rad = 0.20;
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);
    camera.advance([10.0, 2.0, 10.0], 0.0, 1.0 / 60.0);
    let eye_level = camera.render_camera(&level, &environment).eye;
    let eye_pitched = camera.render_camera(&pitched, &environment).eye;
    let moved = (glam::Vec3::from_array(eye_level) - glam::Vec3::from_array(eye_pitched)).length();
    assert!(moved > 0.1, "a pitched hull relocates the sniper optics, moved {moved}");

    // Vertical damping: a sudden hull step reaches the eye softened, then settles; the aim
    // direction never lags (target - eye direction is pose-driven, not smoothed).
    let mut rig = BattleCameraController::new(BattleCameraSettings::default());
    rig.set_mode(BattleCameraMode::Sniper);
    for _ in 0..60 {
        rig.advance([10.0, 2.0, 10.0], 0.0, 1.0 / 60.0);
    }
    let before = rig.render_camera(&level, &environment).eye[1];
    rig.advance([10.0, 2.3, 10.0], 0.0, 1.0 / 60.0); // 0.3 m rut step in one frame
    let bumped = CameraSubject::from_snapshot(tank_snapshot([10.0, 2.3, 10.0], 0.0, 0.0), 0.0);
    let after = rig.render_camera(&bumped, &environment).eye[1];
    assert!(
        after - before < 0.25,
        "the first frame of a 0.3 m step reaches the eye softened, got {}",
        after - before
    );
    for _ in 0..60 {
        rig.advance([10.0, 2.3, 10.0], 0.0, 1.0 / 60.0);
    }
    let settled = rig.render_camera(&bumped, &environment).eye[1];
    assert!(
        (settled - (before + 0.3)).abs() < 0.02,
        "the damper settles onto the true height, got {settled} vs {}",
        before + 0.3
    );
}

#[test]
fn switching_modes_travels_the_view_instead_of_teleporting_it() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let subject = CameraSubject::from_snapshot(tank_snapshot([20.0, 0.0, 20.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.advance([20.0, 0.0, 20.0], 0.0, 1.0 / 60.0);
    let tpp = camera.present(&subject, &environment, 1.0 / 60.0);

    camera.set_mode(BattleCameraMode::Sniper);
    // One frame in: the presented FOV has left TPP but not yet reached the sniper step.
    let mid = camera.present(&subject, &environment, 1.0 / 60.0);
    let sniper_fov = camera.sniper_fov_degrees();
    assert!(mid.vertical_fov_degrees < tpp.vertical_fov_degrees - 1.0, "the blend departs");
    assert!(mid.vertical_fov_degrees > sniper_fov + 1.0, "and has not yet arrived");

    // A quarter second later the transition is over and the sniper camera is exact.
    let mut settled = mid;
    for _ in 0..15 {
        settled = camera.present(&subject, &environment, 1.0 / 60.0);
    }
    assert!((settled.vertical_fov_degrees - sniper_fov).abs() < 1.0e-3);

    // The LOGICAL camera never blends: aiming reads the destination immediately.
    let logical = camera.render_camera(&subject, &environment);
    assert_eq!(logical.vertical_fov_degrees, sniper_fov);
}

/// The scope surround rides the SAME clock as the camera's mode blend: it irises in while the
/// view travels into the optics and lifts away as it leaves. Before this, the vignette popped in
/// full-strength on the first sniper frame while the camera was still mid-flight — the hard cut
/// the whole transition blend exists to remove.
#[test]
fn the_scope_surround_rides_the_mode_blend_clock() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let subject = CameraSubject::from_snapshot(tank_snapshot([20.0, 0.0, 20.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.advance([20.0, 0.0, 20.0], 0.0, 1.0 / 60.0);
    camera.present(&subject, &environment, 1.0 / 60.0);
    assert_eq!(camera.scope_dressing(), 0.0, "no scope dressing in third person");

    // Entry: exactly mid-blend (0.14 s transition; smoothstep(0.5) = 0.5) the housing is half in.
    camera.set_mode(BattleCameraMode::Sniper);
    camera.present(&subject, &environment, 0.07);
    let entering = camera.scope_dressing();
    assert!(
        (entering - 0.5).abs() < 0.05,
        "mid-entry the surround must be half-irised, got {entering}"
    );
    for _ in 0..15 {
        camera.present(&subject, &environment, 1.0 / 60.0);
    }
    assert_eq!(camera.scope_dressing(), 1.0, "settled sniper shows the full surround");

    // Exit: the housing lifts away on the same clock, not a hard cut.
    camera.set_mode(BattleCameraMode::ThirdPerson);
    camera.present(&subject, &environment, 0.07);
    let leaving = camera.scope_dressing();
    assert!(
        (leaving - 0.5).abs() < 0.05,
        "mid-exit the surround must be half-lifted, got {leaving}"
    );
    for _ in 0..15 {
        camera.present(&subject, &environment, 1.0 / 60.0);
    }
    assert_eq!(camera.scope_dressing(), 0.0, "back in third person nothing remains");
}

/// The transition FOV blends in MAGNIFICATION space: perceived zoom is 1/FOV, so a linear FOV
/// sweep snaps violently at the wide end and crawls at the narrow end. Locked here: mid-blend the
/// presented FOV sits well BELOW the linear midpoint (the harmonic path), so the zoom rate reads
/// constant to the eye.
#[test]
fn the_mode_blend_zooms_at_a_perceptually_constant_rate() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let subject = CameraSubject::from_snapshot(tank_snapshot([20.0, 0.0, 20.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.advance([20.0, 0.0, 20.0], 0.0, 1.0 / 60.0);
    let tpp = camera.present(&subject, &environment, 1.0 / 60.0);

    camera.set_mode(BattleCameraMode::Sniper);
    // Land exactly mid-blend (the 0.14 s transition, eased: smoothstep(0.5) = 0.5).
    let mid = camera.present(&subject, &environment, 0.07);
    let sniper_fov = camera.sniper_fov_degrees();
    let linear_mid = (tpp.vertical_fov_degrees + sniper_fov) * 0.5;
    let harmonic_mid = 2.0 / (1.0 / tpp.vertical_fov_degrees + 1.0 / sniper_fov);
    assert!(
        (mid.vertical_fov_degrees - harmonic_mid).abs() < 0.5,
        "mid-blend FOV must ride the magnification path ({harmonic_mid:.1}), got {:.1}",
        mid.vertical_fov_degrees
    );
    assert!(
        mid.vertical_fov_degrees < linear_mid - 4.0,
        "the magnification path sits well below the linear midpoint ({linear_mid:.1})"
    );
}

#[test]
fn the_boom_snaps_shorter_at_a_wall_and_recovers_smoothly_past_it() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let mut environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let subject = CameraSubject::from_snapshot(tank_snapshot([20.0, 0.0, 20.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.advance([20.0, 0.0, 20.0], 0.0, 1.0 / 60.0);
    let open = camera.present(&subject, &environment, 1.0 / 60.0);
    let target = glam::Vec3::from_array(open.target);
    let open_boom = (glam::Vec3::from_array(open.eye) - target).length();

    // A wall drops behind the tank: the boom must shorten THIS frame (never clip)...
    environment.add_obstacle(client::CameraObstacle::aabb(
        "wall",
        [20.0, 5.0, 16.0],
        [6.0, 8.0, 0.6],
    ));
    let blocked = camera.present(&subject, &environment, 1.0 / 60.0);
    let blocked_boom = (glam::Vec3::from_array(blocked.eye) - target).length();
    assert!(blocked_boom < open_boom - 2.0, "the boom shortens instantly at the wall");

    // ...and once the wall is gone it eases back out instead of popping.
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let first = camera.present(&subject, &environment, 1.0 / 60.0);
    let first_boom = (glam::Vec3::from_array(first.eye) - target).length();
    assert!(first_boom > blocked_boom + 0.05, "recovery starts");
    assert!(first_boom < open_boom - 1.0, "but takes visible time, no pop");
    let mut last = first;
    for _ in 0..90 {
        last = camera.present(&subject, &environment, 1.0 / 60.0);
    }
    let final_boom = (glam::Vec3::from_array(last.eye) - target).length();
    assert!((final_boom - open_boom).abs() < 0.05, "the boom fully recovers");
}

/// Immersja B1: the presented TPP rig rides a fraction of the sprung hull — a brake dive
/// dips the VIEW the way it dips the tank, heave lowers the whole rig — while the LOGICAL
/// camera is bit-identical with the springs loaded (aiming never reads them), the sniper
/// never moves at all, and defensive caps mean a runaway spring can nod the view but never
/// throw it.
#[test]
fn the_presented_tpp_rides_the_sprung_hull_and_the_logical_and_sniper_never_do() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let position = [20.0, 0.0, 20.0];
    let calm = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0);
    // A brake dive: the spring noses down 0.03 rad and sags 0.12 m under the transfer.
    let diving = calm.with_sprung_attitude(-0.03, -0.12);

    // The LOGICAL camera ignores the springs entirely — bit-identical output.
    let mut logical = BattleCameraController::new(BattleCameraSettings::default());
    logical.set_mode(BattleCameraMode::ThirdPerson);
    assert_eq!(
        logical.render_camera(&calm, &environment),
        logical.render_camera(&diving, &environment),
        "aiming must never read the presentation springs"
    );

    // The PRESENTED TPP rides them: the view direction dips and the rig sits lower.
    let mut calm_rig = BattleCameraController::new(BattleCameraSettings::default());
    calm_rig.set_mode(BattleCameraMode::ThirdPerson);
    let mut dive_rig = BattleCameraController::new(BattleCameraSettings::default());
    dive_rig.set_mode(BattleCameraMode::ThirdPerson);
    let presented_calm = calm_rig.present(&calm, &environment, 1.0 / 60.0);
    let presented_dive = dive_rig.present(&diving, &environment, 1.0 / 60.0);
    let dir_of = |camera: &renderer_api::Camera| {
        (glam::Vec3::from_array(camera.target) - glam::Vec3::from_array(camera.eye))
            .normalize_or_zero()
    };
    assert!(
        dir_of(&presented_dive).y < dir_of(&presented_calm).y - 1.0e-4,
        "the brake dive visibly dips the presented view"
    );
    let expected_heave = -0.12 * 0.5;
    assert!(
        (presented_dive.eye[1] - (presented_calm.eye[1] + expected_heave)).abs() < 1.0e-3,
        "heave lowers the rig by its fraction, got {} vs {}",
        presented_dive.eye[1],
        presented_calm.eye[1]
    );

    // Caps: an absurd spring input nods the view inside the caps, never throws it.
    let mut absurd_rig = BattleCameraController::new(BattleCameraSettings::default());
    absurd_rig.set_mode(BattleCameraMode::ThirdPerson);
    let presented_absurd =
        absurd_rig.present(&calm.with_sprung_attitude(-10.0, -10.0), &environment, 1.0 / 60.0);
    let tilt = dir_of(&presented_absurd).angle_between(dir_of(&presented_calm));
    assert!(tilt <= 0.021, "view tilt stays inside the 0.02 rad cap, got {tilt}");
    assert!(
        (presented_absurd.eye[1] - presented_calm.eye[1]).abs() <= 0.201,
        "rig drop stays inside the 0.2 m cap"
    );

    // Sniper: bit-identical with the springs loaded — aiming tolerates no theatrics.
    let mut sniper_calm = BattleCameraController::new(BattleCameraSettings::default());
    sniper_calm.set_mode(BattleCameraMode::Sniper);
    let mut sniper_dive = BattleCameraController::new(BattleCameraSettings::default());
    sniper_dive.set_mode(BattleCameraMode::Sniper);
    assert_eq!(
        sniper_calm.present(&calm, &environment, 1.0 / 60.0),
        sniper_dive.present(&diving, &environment, 1.0 / 60.0),
        "the sniper eye stays rigid through the springs"
    );
}

/// Immersja B2, first promise: the shot's rock reaches the presented camera THROUGH the
/// spring — the chain B1 built carries it for free. The hull's fire impulse rocks the
/// presentation spring; the spring's residual is exactly what the presented rig rides.
#[test]
fn the_shots_rock_reaches_the_presented_camera_through_the_spring() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let level = engine::AttitudeSample { terrain_pitch_rad: 0.0, terrain_roll_rad: 0.0 };
    let still =
        engine::TankMotion { forward_speed_mps: 0.0, accel_long_mps2: 0.0, yaw_rate_rad_s: 0.0 };
    let mut attitude = engine::HullAttitude::default();
    for _ in 0..240 {
        attitude.step([0.0, 0.0, 0.0], still, level, 1.0, 1.0 / 60.0);
    }
    let settled = attitude.pitch_rad;
    attitude.fire_impulse(0.0); // over the bow: the nose answers in pitch
    for _ in 0..5 {
        attitude.step([0.0, 0.0, 0.0], still, level, 1.0, 1.0 / 60.0);
    }
    let residual = attitude.pitch_rad - settled;
    assert!(residual.abs() > 5.0e-4, "the shot must rock the spring, got {residual}");

    let position = [20.0, 0.0, 20.0];
    let calm = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0);
    let rocked = calm.with_sprung_attitude(residual, attitude.heave_m);
    let mut calm_rig = BattleCameraController::new(BattleCameraSettings::default());
    calm_rig.set_mode(BattleCameraMode::ThirdPerson);
    let mut rocked_rig = BattleCameraController::new(BattleCameraSettings::default());
    rocked_rig.set_mode(BattleCameraMode::ThirdPerson);
    assert_ne!(
        calm_rig.present(&calm, &environment, 1.0 / 60.0),
        rocked_rig.present(&rocked, &environment, 1.0 / 60.0),
        "the presented rig must answer the shot's rock"
    );
}

/// Immersja B2, second promise: a rough ride shivers the presented rig, a standing tank's
/// camera is bit-stable, an absurd amplitude stays inside the hard cap, and the sniper
/// never trembles at all. No RNG — the same input sequence renders the same frames.
#[test]
fn a_rough_ride_shivers_the_rig_and_a_standing_tank_never_does() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let position = [20.0, 0.0, 20.0];
    let calm = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0);
    let rough = calm.with_ride_shake(0.03);

    let spread = |subject: &CameraSubject| {
        let mut rig = BattleCameraController::new(BattleCameraSettings::default());
        rig.set_mode(BattleCameraMode::ThirdPerson);
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for _ in 0..48 {
            let eye_y = rig.present(subject, &environment, 1.0 / 60.0).eye[1];
            lo = lo.min(eye_y);
            hi = hi.max(eye_y);
        }
        hi - lo
    };
    assert!(spread(&rough) > 0.02, "the rough ride must visibly shiver, got {}", spread(&rough));
    assert!(spread(&calm) < 1.0e-6, "a standing tank's rig is bit-stable");
    assert!(
        spread(&calm.with_ride_shake(5.0)) <= 0.101,
        "an absurd amplitude stays inside the 0.05 m cap"
    );

    // Determinism: the same sequence renders the same frames, bit for bit.
    let run = |subject: &CameraSubject| {
        let mut rig = BattleCameraController::new(BattleCameraSettings::default());
        rig.set_mode(BattleCameraMode::ThirdPerson);
        (0..16).map(|_| rig.present(subject, &environment, 1.0 / 60.0).eye).collect::<Vec<_>>()
    };
    assert_eq!(run(&rough), run(&rough), "no RNG in presentation");

    // Sniper: the tremor never reaches the optics.
    let mut sniper_a = BattleCameraController::new(BattleCameraSettings::default());
    sniper_a.set_mode(BattleCameraMode::Sniper);
    let mut sniper_b = BattleCameraController::new(BattleCameraSettings::default());
    sniper_b.set_mode(BattleCameraMode::Sniper);
    assert_eq!(
        sniper_a.present(&calm, &environment, 1.0 / 60.0),
        sniper_b.present(&rough, &environment, 1.0 / 60.0),
        "the sniper never trembles"
    );
}

/// Immersja B3, the freeze law made a gate: excite EVERY feel channel at once — follow
/// spring displacement, own-shot kick, damage shudder, landing slam, sprung dive/heave,
/// ride tremor — then freeze the inputs the way `freeze_motion()` leaves them (zero speed,
/// zero residuals, zero amplitude, no new impulses) and the presented camera must settle
/// to BIT-stability. A future feel channel with unretired state fails this by construction:
/// register your retirement or you do not ship (docs/battle-camera-policy.md).
#[test]
fn every_feel_channel_retires_when_motion_freezes() {
    let heightmap = HeightMap::flat(64, 64, 1.0, 0.0).expect("heightmap");
    let environment = BattleCameraEnvironment::with_terrain(&heightmap);
    let position = [20.0, 0.0, 20.0];
    let excited = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0)
        .with_sprung_attitude(-0.03, -0.12)
        .with_ride_shake(0.03);

    let mut rig = BattleCameraController::new(BattleCameraSettings::default());
    rig.set_mode(BattleCameraMode::ThirdPerson);
    // Every impulse channel at once, over a few frames of fast driving.
    for _ in 0..10 {
        rig.advance(position, 12.0, 1.0 / 60.0);
        rig.present(&excited, &environment, 1.0 / 60.0);
    }
    rig.fire_kick(0.0);
    rig.damage_shudder(glam::Vec3::new(1.0, 0.0, -1.0), 0.8);
    rig.impact_kick(5.0);
    rig.present(&excited, &environment, 1.0 / 60.0);

    // The freeze: what the world looks like after freeze_motion() — the predictor's speed
    // and accel are zero, so the spring residuals, tremor amplitude and FOV boost inputs
    // all arrive as zero; no further impulses fire.
    let frozen = CameraSubject::from_snapshot(tank_snapshot(position, 0.0, 0.0), 0.0);
    for _ in 0..600 {
        rig.advance(position, 0.0, 1.0 / 60.0);
        rig.present(&frozen, &environment, 1.0 / 60.0);
    }
    let a = {
        rig.advance(position, 0.0, 1.0 / 60.0);
        rig.present(&frozen, &environment, 1.0 / 60.0)
    };
    let b = {
        rig.advance(position, 0.0, 1.0 / 60.0);
        rig.present(&frozen, &environment, 1.0 / 60.0)
    };
    assert_eq!(a, b, "a frozen world must carry a bit-stable presented camera");
}
