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
    let stock = app.predictor.selected_shell();

    // Same frame, no server round-trip: the ballistic solution the reticle and the gun commands
    // read must change the instant the key lands. The default T-54's slot 1 is its authored
    // BK-5 HEAT (a different round entirely), so the selected shell — not just a stat — flips.
    app.request_ammo_slot(1);
    let special = app.predictor.selected_shell();
    assert_ne!(
        special.shell_type, stock.shell_type,
        "the selected round must switch immediately: {special:?} vs {stock:?}"
    );
    assert_ne!(
        special.penetration_mm_at_100m, stock.penetration_mm_at_100m,
        "and the reticle's penetration input moves with it"
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

// --- The ESC leave-battle modal -----------------------------------------------------------

use crate::hud::pause_menu::{EXIT_CENTER, PauseMenuButton, STAY_CENTER};
use winit::keyboard::{KeyCode, PhysicalKey};

fn in_battle() -> ClientApp {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    app.run_fixed_ticks(5);
    app
}

// --- Focus loss ------------------------------------------------------------------------------

/// Alt+Tab with Alt held: Alt is the free-look key, so the switch away happens mid-glance, and
/// whether the window ever sees Alt's release is the platform's choice. The app must not care:
/// losing focus ends the glance the way a release would — camera back on the aim, pitch restored
/// — instead of leaving the mouse on the camera and off the gun for the rest of the match.
#[test]
fn losing_focus_ends_a_free_look_and_returns_the_camera_to_the_aim() {
    let mut app = in_battle();
    app.camera_controller.set_orbit_yaw(0.5);
    app.desired_aim = crate::aim::DesiredAim::new(0.5, 0.0);
    let pitch_before = app.camera_controller.pitch_rad();
    app.begin_free_look();
    app.input.mouse_dx = 200.0;
    app.input.mouse_dy = 100.0;
    app.apply_mouse_look();
    assert!((app.camera_controller.orbit_yaw_rad() - 0.5).abs() > 0.1, "precondition: glancing");

    app.on_focus_change(false);

    assert!(!app.input.free_look, "the glance ends with the focus");
    assert_eq!(app.camera_controller.orbit_yaw_rad(), 0.5, "the camera is back on the aim");
    assert_eq!(app.camera_controller.pitch_rad(), pitch_before);
    // Mouse motion the OS may still deliver around the edge does not become a look delta
    // either way; and the gun follows the mouse again on return.
    app.on_focus_change(true);
    app.input.mouse_dx = 100.0;
    app.apply_mouse_look();
    assert!(app.desired_aim.yaw_rad() < 0.5, "back in focus the mouse aims the gun again");
}

/// The same for a Shift hold: the scope opened by Shift closes with the focus, restoring the
/// prior mode, and the hold latch is released — a latch left set would swallow the next Shift
/// press whole (`begin_sniper_hold` refuses while it thinks Shift is down).
#[test]
fn losing_focus_ends_a_sniper_hold_and_every_other_latch() {
    let mut app = in_battle();
    app.begin_sniper_hold();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
    app.input.set_shift(true);
    app.input.wheel_pending_lines = 0.6;
    app.input.forward = true;
    app.input.fire_pending = true;

    app.on_focus_change(false);

    assert_eq!(app.camera_controller.mode(), BattleCameraMode::ThirdPerson, "the hold released");
    assert!(app.input.sniper_hold_return.is_none(), "the latch is free for the next press");
    assert!(!app.input.shift);
    assert_eq!(app.input.wheel_pending_lines, 0.0);
    assert_eq!(app.input.throttle(), 0.0);
    assert!(!app.input.fire_pending);

    // And the next Shift press is not swallowed.
    app.begin_sniper_hold();
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
}

// --- OS key auto-repeat --------------------------------------------------------------------

/// A held key is ONE press. Windows re-sends a held key as a press every ~33 ms; every battle
/// binding is an edge, so each repeat used to be a second press the player never made: V
/// flickered the view between scope and third person thirty times a second, a held ESC opened
/// and dismissed the modal in a blur, and a held Space during a reload knocked the refusal
/// sound at the repeat rate. Repeats must reach no binding at all.
#[test]
fn os_key_repeat_is_not_a_second_press() {
    let key = |code| PhysicalKey::Code(code);
    let mut app = in_battle();

    // V: the press toggles once, the repeats leave the scope exactly where the press put it.
    app.on_key(key(KeyCode::KeyV), true, false);
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper);
    for _ in 0..7 {
        app.on_key(key(KeyCode::KeyV), true, true);
    }
    assert_eq!(app.camera_controller.mode(), BattleCameraMode::Sniper, "a held V must not flicker");
    app.on_key(key(KeyCode::KeyV), false, false);

    // ESC: the modal opens on the press and stays open through the repeats.
    app.on_key(key(KeyCode::Escape), true, false);
    assert!(app.pause_menu.is_some());
    for _ in 0..7 {
        app.on_key(key(KeyCode::Escape), true, true);
    }
    assert!(app.pause_menu.is_some(), "a held ESC must not answer its own question");
    app.on_key(key(KeyCode::Escape), false, false);
    app.on_key(key(KeyCode::Escape), true, false);
    assert!(app.pause_menu.is_none(), "the second real press dismisses it");

    // Space: the trigger latches on the press; a repeat arriving after the batch consumed it
    // must not latch again (that repeat was the refusal knock every 33 ms of a reload).
    app.on_key(key(KeyCode::Space), true, false);
    assert!(app.input.fire_pending);
    app.input.fire_pending = false; // the fixed-tick batch consumed the edge
    app.on_key(key(KeyCode::Space), true, true);
    assert!(!app.input.fire_pending, "a repeat is not a trigger pull");

    // 1/2/3: the switch rides one command; a repeat must not queue it again.
    app.on_key(key(KeyCode::Digit2), true, false);
    assert_eq!(app.input.pending_ammo_select, Some(1));
    app.input.pending_ammo_select = None;
    app.on_key(key(KeyCode::Digit2), true, true);
    assert_eq!(app.input.pending_ammo_select, None);

    // A held drive key loses nothing: its first press latched it, repeats change nothing, the
    // release still lands.
    app.on_key(key(KeyCode::KeyW), true, false);
    app.on_key(key(KeyCode::KeyW), true, true);
    assert_eq!(app.input.throttle(), 1.0);
    app.on_key(key(KeyCode::KeyW), false, false);
    assert_eq!(app.input.throttle(), 0.0);
}

/// ESC in a live battle asks the question instead of silently dropping the cursor, and ESC again
/// answers "stay" — the modal must never be a one-way door the player has to click out of.
#[test]
fn escape_raises_the_leave_battle_modal_and_escape_again_dismisses_it() {
    let mut app = in_battle();
    assert!(app.pause_menu.is_none(), "a fresh battle has no modal up");

    app.on_battle_keyboard(PhysicalKey::Code(KeyCode::Escape), true);
    assert!(app.pause_menu.is_some(), "ESC in battle raises the modal");

    app.on_battle_keyboard(PhysicalKey::Code(KeyCode::Escape), true);
    assert!(app.pause_menu.is_none(), "ESC again dismisses it");
    assert!(app.garage.has_started() && !app.garage.is_open(), "dismissing returns to the battle");
}

/// Nothing is pre-lit when the modal opens: a still cursor must not leave the destructive choice
/// sitting hot, where a reflexive click would leave the battle the player meant to stay in.
#[test]
fn the_modal_opens_with_neither_choice_under_the_cursor() {
    let mut app = in_battle();
    app.open_pause_menu();

    let menu = app.pause_menu.expect("modal open");
    assert_eq!(menu.hovered(), None);
}

/// The battle does NOT pause, so a hull whose driver is reading a menu must stop rather than
/// drive on blind. Opening the modal releases the held drive keys, and no key press reaches the
/// tank while it is up.
#[test]
fn the_open_modal_stops_the_hull_and_swallows_every_driving_key() {
    let mut app = in_battle();
    app.input.forward = true;
    app.input.brake = true;
    app.input.fire_pending = true;

    app.open_pause_menu();
    assert_eq!(app.input.throttle(), 0.0, "the held throttle is released, not latched");
    assert_eq!(app.input.brake_value(), 0.0);
    assert!(!app.input.fire_pending, "a pending shot is dropped, not queued for the dismissal");

    for key in [KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD, KeyCode::Space] {
        app.on_battle_keyboard(PhysicalKey::Code(key), true);
    }
    assert_eq!(app.input.throttle(), 0.0, "driving keys never reach the tank behind the modal");
    assert_eq!(app.input.steer(), 0.0);
    assert!(!app.input.fire_pending, "and neither does the trigger");

    // A release arriving after the modal opened still lands: swallowing it would strand the key.
    app.on_battle_keyboard(PhysicalKey::Code(KeyCode::KeyW), false);
    assert_eq!(app.input.throttle(), 0.0);
}

/// The mouse answers the modal, not the gun: motion collected over the buttons is discarded
/// instead of being spent as a look delta when the menu closes.
#[test]
fn the_modal_swallows_mouse_look_instead_of_swinging_the_turret() {
    let mut app = in_battle();
    app.camera_controller.set_orbit_yaw(0.0);
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    app.open_pause_menu();

    app.input.mouse_dx = 240.0;
    app.apply_mouse_look();
    assert_eq!(app.input.mouse_dx, 0.0, "the delta is consumed, not banked");
    assert_eq!(app.desired_aim.yaw_rad(), 0.0, "and it never reached the aim");

    app.input.mouse_dx = 240.0;
    app.close_pause_menu();
    assert_eq!(app.input.mouse_dx, 0.0, "closing clears what was collected over the buttons");
}

/// EXIT TO GARAGE leaves for the garage; STAY IN BATTLE returns to driving. Both are driven
/// through the cursor, so this locks the hit test and the actions together.
#[test]
fn clicking_exit_opens_the_garage_and_clicking_stay_returns_to_the_battle() {
    let mut app = in_battle();

    app.open_pause_menu();
    app.pause_menu.as_mut().expect("open").cursor_clip = STAY_CENTER;
    assert_eq!(app.pause_menu.expect("open").hovered(), Some(PauseMenuButton::Stay));
    app.pause_menu_primary_press();
    assert!(app.pause_menu.is_none(), "STAY dismisses the modal");
    assert!(!app.garage.is_open(), "and does NOT leave the battle");

    app.open_pause_menu();
    app.pause_menu.as_mut().expect("open").cursor_clip = EXIT_CENTER;
    assert_eq!(app.pause_menu.expect("open").hovered(), Some(PauseMenuButton::ExitToGarage));
    app.pause_menu_primary_press();
    assert!(app.pause_menu.is_none(), "the modal closes behind the choice");
    assert!(app.garage.is_open(), "EXIT TO GARAGE opens the garage over the battle");
}

/// A click that lands between the buttons must do nothing at all. A modal that dismissed on a
/// stray click would drop the player back into the battle without an answer.
#[test]
fn a_click_off_both_buttons_leaves_the_modal_up() {
    let mut app = in_battle();
    app.open_pause_menu();
    app.pause_menu.as_mut().expect("open").cursor_clip =
        [0.0, (EXIT_CENTER[1] + STAY_CENTER[1]) * 0.5];

    app.pause_menu_primary_press();

    assert!(app.pause_menu.is_some(), "the question stands until it is answered");
    assert!(!app.garage.is_open());
}

/// Before a battle exists (the player has never left the garage) ESC keeps its plain meaning:
/// hand the cursor back. Raising a "leave battle?" modal over the garage would be nonsense.
#[test]
fn escape_before_the_first_battle_raises_no_modal() {
    let mut app = ClientApp::new();
    assert!(!app.garage.has_started());

    app.on_battle_keyboard(PhysicalKey::Code(KeyCode::Escape), true);

    assert!(app.pause_menu.is_none());
}
