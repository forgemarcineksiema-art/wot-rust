use game_core::TankSpec;
use physics::{
    TankControlInput, TankControllerSettings, TankKinematicState, TerrainContact,
    step_custom_tank_controller_on_contact,
};

#[test]
fn tank_spec_power_to_weight_changes_acceleration_profile() {
    let t55a = TankControllerSettings::from_spec(&TankSpec::t55a());
    let tiger_ii = TankControllerSettings::from_spec(&TankSpec::tiger_ii_ausf_b());

    assert!(t55a.acceleration_mps2 > tiger_ii.acceleration_mps2);
    assert!(t55a.max_forward_speed_mps > tiger_ii.max_forward_speed_mps);
    assert!(tiger_ii.brake_deceleration_mps2 > tiger_ii.acceleration_mps2);
}

#[test]
fn uphill_contact_reduces_acceleration() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
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
        },
        0.5,
    );

    assert!(rough.forward_speed() < flat.forward_speed());
    assert!(rough.yaw_rad < flat.yaw_rad);
    assert_eq!(rough.position.y, 4.0);
}

#[test]
fn braking_overrides_throttle_and_decelerates() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
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
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
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
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
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
fn unclimbable_face_stops_incoming_momentum() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
    // A hull charging an unclimbable face at 10 m/s must not punch through it on momentum: a face
    // past the gradeability limit is a barrier (the tracks lose drive and the nose digs in), so
    // the forward momentum is arrested rather than carrying the hull over the top.
    let mut state = TankKinematicState {
        velocity: glam::Vec3::new(0.0, 0.0, 10.0),
        ..TankKinematicState::default()
    };
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let contact = contact_with_slope(settings.max_climb_grade + 0.1);

    let mut max_z = 0.0_f32;
    for _ in 0..120 {
        step_custom_tank_controller_on_contact(&mut state, input, &settings, contact, 1.0 / 60.0);
        max_z = max_z.max(state.position.z);
    }

    assert!(
        state.forward_speed().abs() < 0.05,
        "an unclimbable face must arrest forward momentum, got {}",
        state.forward_speed()
    );
    assert!(
        max_z < 1.0,
        "incoming momentum must not carry the hull up an unclimbable face, reached z={max_z}"
    );
}

fn contact_with_slope(forward_slope: f32) -> TerrainContact {
    TerrainContact { height_m: 0.0, forward_slope, side_slope: 0.0, roughness: 0.0, traction: 1.0 }
}
