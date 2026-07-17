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
    assert!(
        (25.0..=90.0).contains(&rollout),
        "a coasting hull rolls out over many hull lengths, got {rollout:.1} m"
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
    // scrabbles the hull meaningfully up it, then bleeds off and stalls â€” momentum climbing, not a
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
    // A 20Â° downhill (grade ~0.36) with the throttle released: the static track-lock must hold the
    // hull in place â€” the old kinetic-only model crept downhill on anything past ~8Â°.
    let grade = 0.36_f32;
    assert!(grade <= settings.static_grip_mu, "grade under test must be within static hold");
    let contact = TerrainContact {
        height_m: 0.0,
        forward_slope: -grade, // ground ahead is lower = downhill
        side_slope: 0.0,
        roughness: 0.0,
        traction: 1.0,
        water_depth_m: 0.0,
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

    // A face past the static grade does NOT hold â€” it slides downhill (+z here). A cliff is not a
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
    }
}
