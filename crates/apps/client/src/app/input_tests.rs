//! Locks the battle input bindings: mouse look and free look, wheel zoom accumulation,
//! sniper entry alignment, and the 1/2/3 ammo keys + V camera toggle.

use winit::event::MouseScrollDelta;

use super::ClientApp;
use crate::{BattleCameraInput, BattleCameraMode};

#[test]
fn mouse_look_updates_aim_yaw_when_free_look_is_off() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.camera_controller.set_orbit_yaw(0.0);
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.input.mouse_dx = 100.0;

    app.apply_mouse_look();

    assert!((app.desired_aim.yaw_rad() - app.camera_controller.orbit_yaw_rad()).abs() < 1.0e-5);
    assert!(app.desired_aim.yaw_rad() < 0.0);
}

#[test]
fn third_person_mouse_look_leaves_desired_pitch_to_the_sight_solver() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    let pitch_before = app.camera_controller.pitch_rad();
    app.input.mouse_dy = 100.0;

    app.apply_mouse_look();

    // The TPP gun follows the screen-centre ray; the raw desired pitch must not drift with
    // the mouse, or entering sniper later starts from accumulated junk.
    assert_eq!(app.desired_aim.pitch_rad(), 0.0);
    assert!(app.camera_controller.pitch_rad() > pitch_before, "mouse back tilts the boom");
}

#[test]
fn sniper_mouse_back_aims_down_matching_the_third_person_sense() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.enter_sniper_mode();
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.input.mouse_dy = 100.0; // mouse pulled back/down

    app.apply_mouse_look();

    // In third person, mouse back looks down; the sniper view must agree, and gun pitch
    // raises the muzzle, so looking down means a *negative* desired pitch.
    assert!(app.desired_aim.pitch_rad() < 0.0);
}

#[test]
fn sniper_mouse_look_slows_with_magnification() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.enter_sniper_mode();
    let wide_fov = app.camera_controller.sniper_fov_degrees();
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.input.mouse_dx = 100.0;
    app.apply_mouse_look();
    let wide_turn = app.desired_aim.yaw_rad().abs();

    // Step the zoom to the narrowest FOV and repeat the identical mouse motion.
    while app.camera_controller.sniper_fov_degrees() > 3.0 {
        app.camera_controller.apply_input(BattleCameraInput {
            orbit_yaw_delta_rad: 0.0,
            pitch_delta_rad: 0.0,
            zoom_delta_m: -0.8,
        });
    }
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.camera_controller.set_orbit_yaw(0.0);
    app.input.mouse_dx = 100.0;
    app.apply_mouse_look();
    let narrow_turn = app.desired_aim.yaw_rad().abs();

    let expected_ratio = 3.0 / wide_fov;
    assert!(
        (narrow_turn / wide_turn - expected_ratio).abs() < 1.0e-3,
        "look speed must scale with FOV: {narrow_turn} vs {wide_turn}"
    );
}

#[test]
fn free_look_release_returns_the_camera_to_the_aim() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.camera_controller.set_orbit_yaw(0.5);
    app.desired_aim = crate::aim::DesiredAim::new(0.5, 0.0);
    let pitch_before = app.camera_controller.pitch_rad();

    app.begin_free_look();
    app.input.mouse_dx = 200.0;
    app.input.mouse_dy = 100.0;
    app.apply_mouse_look();
    assert!((app.camera_controller.orbit_yaw_rad() - 0.5).abs() > 0.1, "free look orbits");

    app.end_free_look();

    // The glance must not move the gun: aim is untouched, the camera snaps back to it.
    assert_eq!(app.desired_aim.yaw_rad(), 0.5);
    assert_eq!(app.camera_controller.orbit_yaw_rad(), 0.5);
    assert_eq!(app.camera_controller.pitch_rad(), pitch_before);
}

#[test]
fn mouse_wheel_is_ignored_until_battle_control_starts() {
    let mut app = ClientApp::new();
    let distance_before = app.camera_controller.distance_m();

    app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));

    assert_eq!(app.camera_controller.distance_m(), distance_before);
}

#[test]
fn fractional_wheel_events_accumulate_to_whole_notches() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    let distance_before = app.camera_controller.distance_m();

    // A touchpad swipe arrives as many small pixel deltas; 3 x 20 px = exactly one notch.
    for _ in 0..3 {
        app.on_mouse_wheel(MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
            0.0, 20.0,
        )));
    }

    let moved = distance_before - app.camera_controller.distance_m();
    assert!((moved - 0.8).abs() < 1.0e-5, "one notch moves one zoom step, got {moved}");
}

#[test]
fn scrolling_past_the_shortest_boom_opens_sniper_on_the_crosshair_sight_ray() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.run_fixed_ticks(1); // seed prediction so the sight ray resolves
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.75); // stale, unreachable pitch
    let seed = app.world_sight_seed().expect("crosshair sight ray");

    for _ in 0..15 {
        app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
    }

    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
    assert_opened_on_seed(&mut app, seed);
}

/// Regression: entering the sniper seeds the sight from the turret-ring axis (where the optics
/// sit), NOT the muzzle. A muzzle-origin seed skews the opening view toward the barrel line while
/// the turret is mid-traverse or reversed — the bug where "I aim back, press Shift, and the scope
/// looks forward". Here the aim flicks straight back and the sniper opens while the turret is still
/// far from that bearing: the opening view must track the crosshair (back), not the lagging gun.
#[test]
fn sniper_entry_seeds_from_the_ring_so_a_traversing_turret_does_not_skew_the_view() {
    use std::f32::consts::PI;
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.run_fixed_ticks(30); // settle facing forward
    app.desired_aim = crate::aim::DesiredAim::new(PI, 0.0); // flick the aim straight back
    app.run_fixed_ticks(120); // enter mid-slew: the muzzle is metres off the ring axis

    let tank = app.local_render_tank().expect("local tank");
    assert!(
        (tank.yaw_rad + tank.turret_yaw_rad).abs() < 2.0,
        "precondition: the turret is still well short of the backward aim, so a muzzle-origin \
         seed would skew hard"
    );

    let (seed_yaw, _) = app.world_sight_seed().expect("crosshair sight ray");
    // World azimuth; straight back is +/-PI. It must land within a few degrees of back.
    let back_err = (seed_yaw.abs() - PI).abs();
    assert!(
        back_err < 0.05,
        "sniper must open where the crosshair aims (back), got yaw {seed_yaw} ({} deg off back)",
        back_err.to_degrees()
    );
}

#[test]
fn entering_sniper_with_the_key_opens_on_the_crosshair_sight_ray() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.run_fixed_ticks(40); // let the gun converge on the sight solution
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.75); // stale, unreachable pitch
    let seed = app.world_sight_seed().expect("crosshair sight ray");

    app.enter_sniper_mode();

    assert_opened_on_seed(&mut app, seed);
}

/// The sniper opened on the crosshair's world sight ray, clamped to gun reach: the live desired aim
/// equals `seed` fed through the exact same open path (`new` + `clamp_desired_aim_to_gun_reach`),
/// and the stale 0.75 pitch did not leak through.
fn assert_opened_on_seed(app: &mut ClientApp, seed: (f32, f32)) {
    let opened = app.desired_aim;
    assert!((opened.pitch_rad() - 0.75).abs() > 1.0e-3, "stale pitch must not leak");
    app.desired_aim = crate::aim::DesiredAim::new(seed.0, seed.1);
    app.clamp_desired_aim_to_gun_reach();
    let expected = app.desired_aim;
    assert!(
        (opened.yaw_rad() - expected.yaw_rad()).abs() < 1.0e-4
            && (opened.pitch_rad() - expected.pitch_rad()).abs() < 1.0e-4,
        "opened {opened:?} must equal the clamped crosshair seed {expected:?}"
    );
}

#[test]
fn free_look_moves_camera_without_changing_aim_yaw() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.camera_controller.set_orbit_yaw(0.0);
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.input.free_look = true;
    app.input.mouse_dx = 100.0;

    app.apply_mouse_look();

    assert!(app.camera_controller.orbit_yaw_rad() < 0.0);
    assert_eq!(app.desired_aim.yaw_rad(), 0.0);
}

#[test]
fn selecting_ammo_queues_one_command_and_leaves_the_camera_alone() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    let mode_before = app.camera_controller.mode();

    app.request_ammo_slot(1);

    assert_eq!(app.predictor.selected_ammo(), 1, "the predictor adopts the slot instantly");
    assert_eq!(app.camera_controller.mode(), mode_before, "2 no longer touches the camera");
    assert_eq!(app.input.pending_ammo_select, Some(1));

    // The request rides exactly one fixed-tick batch and reaches the authoritative server.
    app.run_fixed_ticks(4);
    assert_eq!(app.input.pending_ammo_select, None, "consumed once, not resent");
    let snapshot = app.player_snapshot().expect("player snapshot");
    assert_eq!(snapshot.selected_ammo, 1, "the server switched to the requested slot");
}

#[test]
fn v_toggles_between_third_person_and_sniper() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::ThirdPerson);

    app.toggle_camera_mode();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);

    app.toggle_camera_mode();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::ThirdPerson);
}

#[test]
fn shift_hold_enters_sniper_and_releases_to_third_person() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::ThirdPerson);

    app.begin_sniper_hold();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);

    app.end_sniper_hold();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::ThirdPerson);
}

#[test]
fn shift_hold_released_while_v_sniper_stays_in_sniper() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.toggle_camera_mode(); // V puts us in sniper first
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);

    // A Shift hold that starts in sniper must not eject the player on release.
    app.begin_sniper_hold();
    app.end_sniper_hold();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
}

#[test]
fn shift_hold_opens_on_the_crosshair_sight_ray() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.run_fixed_ticks(40); // let the gun converge on the sight solution
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.75); // stale, unreachable pitch
    let seed = app.world_sight_seed().expect("crosshair sight ray");

    app.begin_sniper_hold();

    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
    assert_opened_on_seed(&mut app, seed);
}

#[test]
fn key_entry_always_opens_sniper_at_the_default_zoom() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    let default_fov = app.camera_controller.settings().sniper_fov_degrees;

    // Open sniper, then dial the magnification deeper with the wheel.
    app.enter_sniper_mode();
    assert!((app.camera_controller.sniper_fov_degrees() - default_fov).abs() < 1.0e-4);
    for _ in 0..4 {
        app.on_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
    }
    assert!(app.camera_controller.sniper_fov_degrees() < default_fov, "wheel dialled in");

    // Leave and re-enter with a key: the zoom snaps back to the default, not the last step.
    app.camera_controller.set_mode(BattleCameraMode::ThirdPerson);
    app.enter_sniper_mode();
    assert!(
        (app.camera_controller.sniper_fov_degrees() - default_fov).abs() < 1.0e-4,
        "key entry resets to the default zoom, so an absent-minded peek never opens at max zoom"
    );
}

#[test]
fn shift_hold_ignored_while_garage_open() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.open_garage(); // Shift+click here cycles a module; the scope must not open behind it
    let mode_before = app.camera_controller.mode();

    app.begin_sniper_hold();

    assert_eq!(app.camera_controller.mode(), mode_before, "garage swallows the sniper hold");
}

#[test]
fn switching_ammo_updates_the_reticle_ballistics_immediately() {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    let stock_velocity = app.predictor.selected_shell().muzzle_velocity_mps;

    // Same frame, no server round-trip: APCR flies faster, so the ballistic solution the
    // reticle and the gun commands read must change the instant the key lands.
    app.request_ammo_slot(1);
    let apcr_velocity = app.predictor.selected_shell().muzzle_velocity_mps;
    assert!(
        apcr_velocity > stock_velocity * 1.1,
        "APCR must out-fly stock immediately: {apcr_velocity} vs {stock_velocity}"
    );
}

#[test]
fn garage_mouse_delta_is_discarded_before_battle_control_starts() {
    let mut app = ClientApp::new();
    app.camera_controller.set_orbit_yaw(0.0);
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.input.mouse_dx = 240.0;

    app.confirm_garage_selection();
    app.apply_mouse_look();

    assert_eq!(app.input.mouse_dx, 0.0);
    assert_eq!(app.camera_controller.orbit_yaw_rad(), 0.0);
    assert_eq!(app.desired_aim.yaw_rad(), 0.0);
}
