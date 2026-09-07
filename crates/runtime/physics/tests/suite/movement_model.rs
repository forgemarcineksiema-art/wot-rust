use game_core::TankSpec;
use physics::{
    TankControlInput, TankControllerSettings, TankKinematicState, TerrainContact,
    step_custom_tank_controller_on_contact,
};

#[test]
fn tank_spec_power_to_weight_changes_acceleration_profile() {
    let t54 = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let tiger_ii = TankControllerSettings::from_spec(&TankSpec::tiger_ii_ausf_b());

    assert!(t54.drive_power_mps3 > tiger_ii.drive_power_mps3);
    assert!(t54.max_forward_speed_mps > tiger_ii.max_forward_speed_mps);
}

#[test]
fn acceleration_tapers_and_top_speed_is_an_equilibrium_not_a_clamp() {
    // The force model's contract: a hard launch, a visible mid-band taper, and a top speed the
    // hull approaches asymptotically at the spec value. For the Soviet medium that means the
    // 0 -> 12 m/s (43 km/h) pull takes seconds, not the old sub-2s constant-accel snap.
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let dt = 1.0 / 60.0;
    let mut state = TankKinematicState::default();

    let mut to_12_s = None;
    let mut elapsed = 0.0_f32;
    for _ in 0..(90 * 60) {
        step_custom_tank_controller_on_contact(
            &mut state,
            input,
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
        elapsed += dt;
        if to_12_s.is_none() && state.forward_speed() >= 12.0 {
            to_12_s = Some(elapsed);
        }
    }

    let to_12_s = to_12_s.expect("the hull reaches 12 m/s");
    assert!(
        (4.0..=12.0).contains(&to_12_s),
        "0->43 km/h should take arcade-condensed seconds, got {to_12_s:.1} s"
    );
    let vmax = settings.max_forward_speed_mps;
    assert!(
        (state.forward_speed() - vmax).abs() < 0.5,
        "after 90 s the hull sits on its top-speed equilibrium: {} vs {vmax}",
        state.forward_speed()
    );
    assert!(
        state.forward_speed() <= vmax + 0.01,
        "the equilibrium must not overshoot the spec top speed"
    );
}

#[test]
fn releasing_the_throttle_rolls_out_over_many_hull_lengths() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 13.0),
        ..TankKinematicState::default()
    };
    let idle = TankControlInput { throttle: 0.0, steer: 0.0, brake: 0.0 };
    let start_z = state.position.z;
    let dt = 1.0 / 60.0;
    for _ in 0..(30 * 60) {
        step_custom_tank_controller_on_contact(
            &mut state,
            idle,
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
    }
    let rollout = state.position.z - start_z;
    assert!(state.speed() < 0.05, "the hull eventually stops");
    // Engine braking (J3): a T-54 from 13 m/s rolls out ~22 m — a few hull lengths, not the old
    // 45 m glide the owner read as a hull that would not stop.
    assert!(
        (15.0..=35.0).contains(&rollout),
        "a coasting hull rolls out over a few hull lengths, got {rollout:.1} m"
    );
}

#[test]
fn turning_at_speed_scrubs_forward_speed() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let dt = 1.0 / 60.0;
    let mut straight = TankKinematicState::default();
    let mut turning = TankKinematicState::default();
    for _ in 0..(8 * 60) {
        step_custom_tank_controller_on_contact(
            &mut straight,
            TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 },
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
        step_custom_tank_controller_on_contact(
            &mut turning,
            TankControlInput { throttle: 1.0, steer: 1.0, brake: 0.0 },
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
    }
    assert!(
        turning.speed() < straight.speed() - 0.4,
        "a full-lock turn must bleed speed: {} vs {}",
        turning.speed(),
        straight.speed()
    );
}

#[test]
fn uphill_contact_reduces_acceleration() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let mut flat = TankKinematicState::default();
    let mut uphill = TankKinematicState::default();

    step_custom_tank_controller_on_contact(
        &mut flat,
        input,
        &settings,
        TerrainContact::flat(0.0),
        1.0,
    );
    step_custom_tank_controller_on_contact(
        &mut uphill,
        input,
        &settings,
        TerrainContact {
            height_m: 0.0,
            forward_slope: 0.24,
            side_slope: 0.0,
            roughness: 0.0,
            traction: 1.0,
            water_depth_m: 0.0,
            ground: physics::GroundScales::grass(),
        },
        1.0,
    );

    assert!(uphill.forward_speed() < flat.forward_speed());
    assert!(uphill.position.z < flat.position.z);
}

#[test]
fn rough_contact_limits_traction_and_keeps_tank_grounded() {
    let settings = TankControllerSettings::from_spec(&TankSpec::panther_ii());
    let input = TankControlInput { throttle: 1.0, steer: 1.0, brake: 0.0 };
    let mut flat = TankKinematicState::default();
    let mut rough = TankKinematicState::default();

    step_custom_tank_controller_on_contact(
        &mut flat,
        input,
        &settings,
        TerrainContact::flat(4.0),
        0.5,
    );
    step_custom_tank_controller_on_contact(
        &mut rough,
        input,
        &settings,
        TerrainContact {
            height_m: 4.0,
            forward_slope: 0.0,
            side_slope: 0.18,
            roughness: 0.65,
            traction: 0.55,
            water_depth_m: 0.0,
            ground: physics::GroundScales::grass(),
        },
        0.5,
    );

    assert!(rough.forward_speed() < flat.forward_speed());
    assert!(rough.yaw_rad < flat.yaw_rad);
    // Grounding is the vertical resolver's job now (the world stepper composes the two): the
    // rising ground under the hull carries it up, exactly like the old kinematic snap.
    let step = physics::resolve_vertical(&mut rough, 4.0, true, 0.0, 0.5);
    assert!(step.grounded && step.landing_impact_mps == 0.0);
    assert_eq!(rough.position.y, 4.0);
}

#[test]
fn braking_overrides_throttle_and_decelerates() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 8.0),
        ..TankKinematicState::default()
    };

    // Holding throttle AND brake must slow the tank, not keep accelerating forward.
    step_custom_tank_controller_on_contact(
        &mut state,
        TankControlInput { throttle: 1.0, steer: 0.0, brake: 1.0 },
        &settings,
        TerrainContact::flat(0.0),
        0.2,
    );

    assert!(state.forward_speed() < 8.0, "brake must decelerate even with throttle held");
}

#[test]
fn opposite_throttle_does_not_erase_cruise_speed_in_one_tick() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 8.0),
        ..TankKinematicState::default()
    };

    step_custom_tank_controller_on_contact(
        &mut state,
        TankControlInput { throttle: -1.0, steer: 0.0, brake: 0.0 },
        &settings,
        TerrainContact::flat(0.0),
        1.0 / 60.0,
    );

    assert!(
        (7.5..8.0).contains(&state.forward_speed()),
        "one reverse tick must start decelerating 8 m/s, not erase it: {}",
        state.forward_speed()
    );
    assert!(state.position.z > 0.1, "the hull still carries its forward momentum");
}

#[test]
fn opposite_throttle_decelerates_monotonically_before_reversing() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let reverse = TankControlInput { throttle: -1.0, steer: 0.0, brake: 0.0 };
    let contact = TerrainContact::flat(0.0);
    let dt = 1.0 / 60.0;
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 8.0),
        ..TankKinematicState::default()
    };
    let mut previous = state.forward_speed();
    let mut crossing = None;

    for tick in 1..=(8 * 60) {
        step_custom_tank_controller_on_contact(&mut state, reverse, &settings, contact, dt);
        let speed = state.forward_speed();
        assert!(
            speed <= previous + 1.0e-6,
            "held reverse must reduce forward speed monotonically: {previous} -> {speed}"
        );
        if speed <= 0.0 {
            crossing = Some((tick, previous, speed));
            break;
        }
        previous = speed;
    }

    let (crossing_tick, before, after) =
        crossing.expect("full reverse eventually reverses the hull");
    assert!(
        crossing_tick > 30,
        "8 m/s must take physical time to bleed away, crossed after {crossing_tick} ticks"
    );
    assert!(
        before < 0.2 && after > -0.2,
        "the force step must pass near zero instead of snapping across it: {before} -> {after}"
    );
}

#[test]
fn drive_holds_startup_creep_on_an_unclimbable_grade() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let drive = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let grade = (settings.max_climb_grade + settings.momentum_climb_ceiling) * 0.5;
    let contact = contact_with_slope(grade);
    let dt = 1.0 / 60.0;

    let mut stalled = TankKinematicState::default();
    for _ in 0..120 {
        step_custom_tank_controller_on_contact(&mut stalled, drive, &settings, contact, dt);
        assert_eq!(
            stalled.forward_speed(),
            0.0,
            "full forward at a stalled start must not oscillate into rollback"
        );
    }
    assert_eq!(stalled.position.z, 0.0, "the held hull must not creep down the grade");

    let mut tiny_rollback = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, -0.02),
        ..TankKinematicState::default()
    };
    step_custom_tank_controller_on_contact(&mut tiny_rollback, drive, &settings, contact, dt);
    assert_eq!(
        tiny_rollback.forward_speed(),
        0.0,
        "sub-epsilon rollback is startup creep and must still be caught"
    );
}

#[test]
fn reverse_steering_mirrors_forward_steering() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let mut forward = TankKinematicState::default();
    let mut reverse = TankKinematicState::default();

    step_custom_tank_controller_on_contact(
        &mut forward,
        TankControlInput { throttle: 1.0, steer: 1.0, brake: 0.0 },
        &settings,
        TerrainContact::flat(0.0),
        0.5,
    );
    step_custom_tank_controller_on_contact(
        &mut reverse,
        TankControlInput { throttle: -1.0, steer: 1.0, brake: 0.0 },
        &settings,
        TerrainContact::flat(0.0),
        0.5,
    );

    assert!(forward.yaw_rad > 0.0);
    assert!(
        reverse.yaw_rad < 0.0,
        "holding the same steer while reversing should mirror the hull turn, got {}",
        reverse.yaw_rad
    );
}

/// The counter-throttle turn (player verdict 2026-08-22): reversing, then W+D — the tank used
/// to kick visibly LEFT for the whole braking phase (steer sense followed the travel direction)
/// and only then unwind into the commanded right turn, eating reaction time. A tracked hull's
/// yaw is its belt difference and the belts do what the driver commands: from the first tick of
/// W+D the yaw may only go toward the commanded side — not a single tick the wrong way — even
/// while the hull is still sliding backward. Locked in both mirror images.
#[test]
fn a_counter_throttle_turn_never_kicks_toward_the_stale_travel() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let dt = 1.0 / 60.0;

    // Reversing at speed, then forward+right (W+D). The forward tick count covers the whole
    // braking phase and the sign flip of the travel direction.
    let mut hull = TankKinematicState::default();
    let reverse = TankControlInput { throttle: -1.0, steer: 0.0, brake: 0.0 };
    for _ in 0..240 {
        step_custom_tank_controller_on_contact(
            &mut hull,
            reverse,
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
    }
    assert!(hull.forward_speed() < -3.0, "fixture: the hull must be reversing at speed");
    let turn_in = TankControlInput { throttle: 1.0, steer: 1.0, brake: 0.0 };
    let mut crossed_forward = false;
    for _ in 0..240 {
        step_custom_tank_controller_on_contact(
            &mut hull,
            turn_in,
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
        assert!(
            hull.yaw_rad >= -1.0e-6,
            "W+D out of a reverse kicked the hull the WRONG way: yaw {} at speed {}",
            hull.yaw_rad,
            hull.forward_speed()
        );
        crossed_forward |= hull.forward_speed() > 0.0;
    }
    assert!(crossed_forward, "fixture: the hull must have crossed into forward drive");
    assert!(hull.yaw_rad > 0.05, "the commanded right turn must actually develop");

    // Mirror image: driving forward, then S+D — the yaw may only go toward the reverse-mirrored
    // command (negative), never a tick toward the stale forward travel.
    let mut mirror = TankKinematicState::default();
    let forward = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    for _ in 0..240 {
        step_custom_tank_controller_on_contact(
            &mut mirror,
            forward,
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
    }
    assert!(mirror.forward_speed() > 3.0, "fixture: the hull must be driving at speed");
    let back_out = TankControlInput { throttle: -1.0, steer: 1.0, brake: 0.0 };
    for _ in 0..240 {
        step_custom_tank_controller_on_contact(
            &mut mirror,
            back_out,
            &settings,
            TerrainContact::flat(0.0),
            dt,
        );
        assert!(
            mirror.yaw_rad <= 1.0e-6,
            "S+D out of a forward drive kicked the hull the WRONG way: yaw {} at speed {}",
            mirror.yaw_rad,
            mirror.forward_speed()
        );
    }
    assert!(mirror.yaw_rad < -0.05, "the mirrored turn must actually develop");
}

#[test]
fn slope_past_climb_limit_stalls_the_tank_but_gentle_slope_does_not() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };

    let mut steep = TankKinematicState::default();
    let mut gentle = TankKinematicState::default();
    for _ in 0..60 {
        step_custom_tank_controller_on_contact(
            &mut steep,
            input,
            &settings,
            contact_with_slope(settings.max_climb_grade + 0.1),
            1.0 / 60.0,
        );
        step_custom_tank_controller_on_contact(
            &mut gentle,
            input,
            &settings,
            contact_with_slope(settings.max_climb_grade * 0.4),
            1.0 / 60.0,
        );
    }

    assert!(
        steep.forward_speed().abs() < 0.05,
        "an embankment-grade face must stall the tank (got {})",
        steep.forward_speed()
    );
    assert!(
        gentle.forward_speed() > 1.0,
        "a climbable grade must still move the tank (got {})",
        gentle.forward_speed()
    );
}

#[test]
fn momentum_carries_the_hull_up_a_steep_hump_then_it_stalls() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    // A steep-but-below-ceiling face (in the momentum-climb band): a committed run-up at 10 m/s
    // scrabbles the hull meaningfully up it, then bleeds off and stalls — momentum climbing, not a
    // wall, but no free crest either.
    let grade = settings.max_climb_grade + 0.06;
    assert!(grade < settings.momentum_climb_ceiling, "grade under test must be in the climb band");
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 10.0),
        ..TankKinematicState::default()
    };
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let contact = contact_with_slope(grade);
    let start_z = state.position.z;

    for _ in 0..600 {
        step_custom_tank_controller_on_contact(&mut state, input, &settings, contact, 1.0 / 60.0);
    }

    let climbed = state.position.z - start_z;
    assert!(climbed > 3.0, "momentum must carry the hull up the hump, climbed only {climbed}");
    assert!(
        state.forward_speed().abs() < 1.0,
        "the climb must bleed off (no free crest), still moving at {}",
        state.forward_speed()
    );
}

#[test]
fn a_face_past_the_ceiling_is_a_hard_wall_even_with_momentum() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    // A cliff / railway embankment (past the momentum ceiling) stays a barrier: a 10 m/s charge is
    // arrested and cannot carry the hull up.
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 10.0),
        ..TankKinematicState::default()
    };
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let contact = contact_with_slope(settings.momentum_climb_ceiling + 0.2);
    let start_z = state.position.z;

    let mut max_z = start_z;
    for _ in 0..120 {
        step_custom_tank_controller_on_contact(&mut state, input, &settings, contact, 1.0 / 60.0);
        max_z = max_z.max(state.position.z);
    }

    assert!(
        state.forward_speed().abs() < 0.05,
        "a cliff must arrest forward momentum, got {}",
        state.forward_speed()
    );
    assert!(
        max_z - start_z < 1.0,
        "momentum must not carry the hull up a cliff, got {}",
        max_z - start_z
    );
}

#[test]
fn a_parked_hull_holds_a_slope_it_used_to_creep_down() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    // A 20° downhill (grade ~0.36) with the throttle released: the static track-lock must hold the
    // hull in place — the old kinetic-only model crept downhill on anything past ~8°.
    let grade = 0.36_f32;
    assert!(grade <= settings.static_grip_mu, "grade under test must be within static hold");
    let contact = TerrainContact {
        height_m: 0.0,
        forward_slope: -grade, // ground ahead is lower = downhill
        side_slope: 0.0,
        roughness: 0.0,
        traction: 1.0,
        water_depth_m: 0.0,
        ground: physics::GroundScales::grass(),
    };
    let idle = TankControlInput { throttle: 0.0, steer: 0.0, brake: 0.0 };
    let mut state = TankKinematicState::default();
    let start_z = state.position.z;
    for _ in 0..600 {
        step_custom_tank_controller_on_contact(&mut state, idle, &settings, contact, 1.0 / 60.0);
    }
    assert!(
        (state.position.z - start_z).abs() < 0.05,
        "a parked hull must hold the slope, drifted {}",
        state.position.z - start_z
    );

    // A face past the static grade does NOT hold — it slides downhill (+z here). A cliff is not a
    // parking spot.
    let steep = TerrainContact { forward_slope: -1.3, ..contact };
    let mut sliding = TankKinematicState::default();
    for _ in 0..600 {
        step_custom_tank_controller_on_contact(&mut sliding, idle, &settings, steep, 1.0 / 60.0);
    }
    assert!(
        sliding.position.z > 1.0,
        "a too-steep face must still slide, got {}",
        sliding.position.z
    );
}

fn contact_with_slope(forward_slope: f32) -> TerrainContact {
    TerrainContact {
        height_m: 0.0,
        forward_slope,
        side_slope: 0.0,
        roughness: 0.0,
        traction: 1.0,
        water_depth_m: 0.0,
        ground: physics::GroundScales::grass(),
    }
}

/// The ground is a MATERIAL, not only a shape.
///
/// `traction` used to be pure geometry — slope, side slope, roughness — so a cobbled street, a
/// ploughed field and a wet river bank all drove identically. The surface now comes from
/// `terrain::GroundClassifier`, the same rule the picture's splat is baked from, which is the
/// honest version of the promise: what you see under the track is what you feel under it.
#[test]
fn the_surface_under_the_tracks_changes_how_the_hull_drives() {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let dt = 1.0 / 60.0;
    let run = |ground: physics::GroundScales| {
        let contact = TerrainContact { ground, ..TerrainContact::flat(0.0) };
        let mut state = TankKinematicState::default();
        for _ in 0..600 {
            step_custom_tank_controller_on_contact(
                &mut state,
                TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 },
                &settings,
                contact,
                dt,
            );
        }
        state.position.length()
    };

    let grass = run(physics::GroundScales::grass());
    // Worn earth: the same bite, but real drag.
    let dirt = run(physics::GroundScales { grip: 0.95, rolling_resist: 1.35 });
    let rock = run(physics::GroundScales { grip: 1.04, rolling_resist: 0.9 });

    assert!(grass > 90.0, "the control run must actually cover ground, got {grass} m");
    assert!(dirt < grass - 1.0, "worn earth must cost real distance: {dirt} m vs {grass} m");
    assert!(rock > grass + 0.5, "rock must roll easier than grass: {rock} m vs {grass} m");
}

/// ...and grass is exactly the old model. Every scale is measured against it, so a map with no
/// material data drives bit-identically to the model before the ground had any — which is what
/// makes the change safe to land under every replay fixture that predates it.
#[test]
fn grass_is_bit_identical_to_the_model_before_ground_material() {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let dt = 1.0 / 60.0;
    let mut with_material = TankKinematicState::default();
    let mut without = TankKinematicState::default();
    let grass =
        TerrainContact { ground: physics::GroundScales::grass(), ..TerrainContact::flat(0.0) };
    // `TerrainContact::flat` already defaults to grass; this is the same contact by construction,
    // and the assertion is that saying so explicitly changes not one bit.
    let default_contact = TerrainContact::flat(0.0);
    for _ in 0..600 {
        let input = TankControlInput { throttle: 1.0, steer: 0.35, brake: 0.0 };
        step_custom_tank_controller_on_contact(&mut with_material, input, &settings, grass, dt);
        step_custom_tank_controller_on_contact(&mut without, input, &settings, default_contact, dt);
    }
    assert_eq!(with_material, without);
}

/// J1: the WoT habit — S while rolling forward BRAKES. From top speed a T-54 under full opposing
/// throttle stands in under two seconds and never reverses before it has stopped.
#[test]
fn opposing_throttle_brakes_a_rolling_hull_to_a_stop_before_it_reverses() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let reverse = TankControlInput { throttle: -1.0, steer: 0.0, brake: 0.0 };
    let contact = TerrainContact::flat(0.0);
    let dt = 1.0 / 60.0;
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, settings.max_forward_speed_mps),
        ..TankKinematicState::default()
    };
    let mut stopped_at = None;
    for tick in 1..=(4 * 60) {
        step_custom_tank_controller_on_contact(&mut state, reverse, &settings, contact, dt);
        if state.forward_speed() <= 0.0 {
            stopped_at = Some(tick);
            break;
        }
    }
    let tick = stopped_at.expect("held S stops a hull from top speed within four seconds");
    let seconds = tick as f32 * dt;
    assert!(seconds < 1.9, "a T-54 from 50 km/h must stand in under 1.9 s, took {seconds:.2} s");
    assert!(
        state.position.z < 13.0,
        "...and inside 13 m, not the old 22 m: {:.1} m",
        state.position.z
    );
    assert!(state.forward_speed() > -0.2, "the stop passes near zero, no snap across it");
}

/// J1: the pedal and the opposing throttle are ONE brake — the same deceleration, so a driver who
/// stops on S and one who stops on Ctrl stand in the same place.
#[test]
fn the_pedal_and_the_opposing_throttle_brake_identically() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let contact = TerrainContact::flat(0.0);
    let dt = 1.0 / 60.0;
    let start = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 10.0),
        ..TankKinematicState::default()
    };
    let mut on_pedal = start;
    let mut on_throttle = start;
    for _ in 0..30 {
        step_custom_tank_controller_on_contact(
            &mut on_pedal,
            TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 },
            &settings,
            contact,
            dt,
        );
        step_custom_tank_controller_on_contact(
            &mut on_throttle,
            TankControlInput { throttle: -1.0, steer: 0.0, brake: 0.0 },
            &settings,
            contact,
            dt,
        );
    }
    assert!(
        on_pedal.forward_speed() < 7.0,
        "half a second of brake bites: {}",
        on_pedal.forward_speed()
    );
    assert!(
        (on_pedal.forward_speed() - on_throttle.forward_speed()).abs() < 1.0e-4,
        "S and Ctrl must be the same brake: {} vs {}",
        on_pedal.forward_speed(),
        on_throttle.forward_speed()
    );
}

/// J2: the brake is what the ground gives. On a riverbed's quarter traction the demand is capped
/// at the track grip (0.6 x g x 0.25 = 1.8 m/s^2) — a hull brakes as badly as it drives there.
#[test]
fn the_brake_is_capped_by_the_ground_s_grip() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let brake = TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 };
    let dt = 1.0 / 60.0;
    let slick = TerrainContact { traction: 0.25, ..TerrainContact::flat(0.0) };
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 8.0),
        ..TankKinematicState::default()
    };
    step_custom_tank_controller_on_contact(&mut state, brake, &settings, slick, dt);
    let lost = 8.0 - state.forward_speed();
    let grip_cap = 0.6 * game_core::math::GRAVITY_MPS2 * 0.25;
    // Rolling resistance and drag add a little on top of the brake itself.
    assert!(
        lost < (grip_cap + 1.0) * dt,
        "on quarter traction the brake may not exceed the grip cap: lost {lost} m/s in a tick"
    );
    assert!(lost > 0.5 * grip_cap * dt, "...but it still brakes: lost {lost} m/s");

    let mut firm = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 8.0),
        ..TankKinematicState::default()
    };
    step_custom_tank_controller_on_contact(
        &mut firm,
        brake,
        &settings,
        TerrainContact::flat(0.0),
        dt,
    );
    assert!(
        8.0 - firm.forward_speed() > 2.0 * lost,
        "firm ground brakes at least twice as hard as the riverbed"
    );
}

/// J5: a steer release settles the heading. From full lock at 8 m/s the T-54's heading may
/// overshoot the point of release by no more than two degrees before it holds.
#[test]
fn a_steer_release_overshoots_the_heading_by_at_most_two_degrees() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let contact = TerrainContact::flat(0.0);
    let dt = 1.0 / 60.0;
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 8.0),
        ..TankKinematicState::default()
    };
    let steering = TankControlInput { throttle: 1.0, steer: 1.0, brake: 0.0 };
    for _ in 0..60 {
        step_custom_tank_controller_on_contact(&mut state, steering, &settings, contact, dt);
    }
    let released_at = state.yaw_rad;
    let straight = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let mut peak = released_at;
    for _ in 0..120 {
        step_custom_tank_controller_on_contact(&mut state, straight, &settings, contact, dt);
        peak = peak.max(state.yaw_rad);
    }
    let overshoot = (peak - released_at).to_degrees();
    assert!(
        overshoot <= 2.0,
        "the heading overshot the release by {overshoot:.1} deg (the old spool: 6.7 deg)"
    );
    assert!(state.yaw_rate_rad_s.abs() < 1.0e-3, "...and the rotation has stopped");
}

/// J6: the launch climbs through the gears — the thrust is a torque curve read through the box,
/// so a full-throttle launch shows every shift as a beat (a drop and a recovery), never a
/// continuous 1/v grind, and no single tick jolts harder than the track grip.
#[test]
fn a_full_throttle_launch_shifts_through_every_gear_without_a_jolt() {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let contact = TerrainContact::flat(0.0);
    let dt = 1.0 / 60.0;
    let full = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let mut state = TankKinematicState::default();
    let mut previous_accel = None::<f32>;
    let mut previous_speed = 0.0;
    let mut shifts = 0;
    let mut last_gear = physics::engine_state(&settings, 0.0).gear;
    for tick in 0..(20 * 60) {
        step_custom_tank_controller_on_contact(&mut state, full, &settings, contact, dt);
        let speed = state.forward_speed();
        let accel = (speed - previous_speed) / dt;
        let gear = physics::engine_state(&settings, speed).gear;
        if gear != last_gear {
            shifts += 1;
            last_gear = gear;
        }
        if let Some(prev) = previous_accel
            && tick > 1
        {
            assert!(
                (accel - prev).abs() <= 4.0,
                "tick {tick}: the thrust jumped {prev:.2} -> {accel:.2} m/s^2 in one tick"
            );
        }
        previous_accel = Some(accel);
        previous_speed = speed;
    }
    assert_eq!(
        shifts,
        spec.gearbox.gear_count() - 1,
        "a launch to top speed goes through every ratio once"
    );
    assert!(
        (state.forward_speed() - settings.max_forward_speed_mps).abs() < 0.5,
        "and the top-speed equilibrium did not move with the box: {}",
        state.forward_speed()
    );
}

/// J6: the engine's revs are the gear's — at a standstill the box is in first at idle, at top
/// speed it is in top gear at the governor, and the gear never goes DOWN as the speed goes up.
#[test]
fn the_gear_is_a_function_of_speed_and_the_revs_are_the_gear_s() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    let at_rest = physics::engine_state(&settings, 0.0);
    assert_eq!(at_rest.gear, 0);
    assert!((at_rest.rpm_norm - physics::IDLE_RPM_NORM).abs() < 1.0e-6);
    let flat_out = physics::engine_state(&settings, settings.max_forward_speed_mps);
    assert_eq!(flat_out.gear, settings.gearbox.gear_count() - 1);
    assert!((flat_out.rpm_norm - 1.0).abs() < 1.0e-6);
    let mut last = 0;
    for step in 0..200 {
        let speed = settings.max_forward_speed_mps * step as f32 / 200.0;
        let gear = physics::engine_state(&settings, speed).gear;
        assert!(gear >= last, "the box shifted down while speeding up at {speed} m/s");
        last = gear;
    }
}

/// J6: the box never starves the drive. Above the clutch floor the thrust through the gears is the
/// rated power over the speed at every speed a launch passes through — the gears decide the revs,
/// not the rating — so the mobility table, the climbing envelope and every replay hold to the bit.
#[test]
fn the_box_decides_the_revs_and_never_the_rating() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t54_1951());
    for step in 1..400 {
        let speed = settings.max_forward_speed_mps * step as f32 / 400.0;
        let through_the_box = physics::engine_thrust_mps2(&settings, speed, 1.0);
        let rated = settings.drive_power_mps3 / speed.max(settings.min_force_speed_mps);
        assert_eq!(
            through_the_box.to_bits(),
            rated.to_bits(),
            "at {speed:.2} m/s the box delivered {through_the_box} against the rating {rated}"
        );
    }
}
