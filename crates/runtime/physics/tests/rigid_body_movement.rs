//! Locking tests for the planar rigid-body hull: angular inertia, neutral pivot, lateral grip vs
//! drift, slope slide, downhill behaviour, and numerical stability. These pin the *emergent*
//! behaviour that replaced the old scalar kinematic model.

use game_core::TankSpec;
use game_core::math::horizontal_forward;
use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankKinematicState, TerrainContact,
    step_custom_tank_controller_on_contact,
};

const DT: f32 = 1.0 / 60.0;

fn drive(
    state: &mut TankKinematicState,
    settings: &TankControllerSettings,
    throttle: f32,
    steer: f32,
    contact: TerrainContact,
    ticks: u32,
) {
    let input = TankControlInput { throttle, steer, brake: 0.0 };
    for _ in 0..ticks {
        step_custom_tank_controller_on_contact(state, input, settings, contact, DT);
    }
}

/// Sideways speed in the hull's current frame â€” the slip that distinguishes gripping from drifting.
fn lateral_speed(state: &TankKinematicState) -> f32 {
    let forward = horizontal_forward(state.yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    state.velocity.dot(right)
}

#[test]
fn hull_rotation_carries_angular_inertia() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
    let mut state = TankKinematicState::default();

    // Build up a steady turn.
    drive(&mut state, &settings, 1.0, 1.0, TerrainContact::flat(0.0), 30);
    let turning_rate = state.yaw_rate_rad_s;
    assert!(turning_rate > 0.1, "a sustained steer should spin the hull up, got {turning_rate}");

    // Release the steer for a single tick: the rate must decay, not snap to zero.
    step_custom_tank_controller_on_contact(
        &mut state,
        TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 },
        &settings,
        TerrainContact::flat(0.0),
        DT,
    );
    let coasting_rate = state.yaw_rate_rad_s;
    assert!(
        coasting_rate > 0.0 && coasting_rate < turning_rate,
        "releasing steer must bleed the yaw rate gradually (inertia), got {coasting_rate} from {turning_rate}"
    );
}

#[test]
fn neutral_steer_pivots_in_place_without_throttle() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
    let mut state = TankKinematicState::default();

    // Steer with no throttle: the hull should rotate (counter-rotating tracks) but not translate.
    drive(&mut state, &settings, 0.0, 1.0, TerrainContact::flat(0.0), 30);

    assert!(state.yaw_rad > 0.05, "neutral steer must rotate the hull, got yaw {}", state.yaw_rad);
    assert!(
        state.velocity.length() < 0.05,
        "a pivot must not translate the hull, got speed {}",
        state.velocity.length()
    );
}

#[test]
fn hard_turn_at_speed_drifts_but_a_gentle_turn_grips() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());

    let drift_for = |steer: f32| {
        let mut state = TankKinematicState::default();
        // Reach top speed in a straight line, then turn. The force model approaches vmax
        // asymptotically, so the run-up is a long pull, not the old 5-second snap.
        drive(&mut state, &settings, 1.0, 0.0, TerrainContact::flat(0.0), 1800);
        drive(&mut state, &settings, 1.0, steer, TerrainContact::flat(0.0), 30);
        lateral_speed(&state).abs()
    };

    let hard = drift_for(1.0);
    let gentle = drift_for(0.2);

    // The scrub term bleeds speed through the turn, so the residual slide is smaller than the
    // old un-scrubbed model's â€” but a full-lock turn at speed must still visibly break loose.
    assert!(hard > 0.08, "a hard turn at speed must break grip and slide, got lateral {hard}");
    assert!(gentle < 0.05, "a gentle turn must hold its line, got lateral {gentle}");
}

#[test]
fn steep_low_traction_slope_slides_but_gentle_slope_holds() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());

    let drift_x = |side_slope: f32, roughness: f32| {
        // Traction falls off with side slope and roughness exactly as the heightmap sampler models.
        let traction = (1.0 - roughness * 0.45 - side_slope.abs() * 0.35).clamp(0.35, 1.0);
        let contact = TerrainContact {
            height_m: 0.0,
            forward_slope: 0.0,
            side_slope,
            roughness,
            traction,
            water_depth_m: 0.0,
        };
        let mut state = TankKinematicState::default();
        // Sit still on the slope (no throttle, no steer) and see whether it holds.
        drive(&mut state, &settings, 0.0, 0.0, contact, 120);
        state.position.x.abs()
    };

    let steep = drift_x(0.9, 0.5);
    let gentle = drift_x(0.2, 0.05);

    assert!(steep > 0.5, "a steep, low-traction face must slide the hull downhill, got {steep}");
    assert!(gentle < 0.05, "a gentle slope must hold the hull in place, got {gentle}");
}

#[test]
fn descending_a_grade_is_not_slower_than_flat() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());

    let top_speed = |forward_slope: f32| {
        let contact = TerrainContact {
            height_m: 0.0,
            forward_slope,
            side_slope: 0.0,
            roughness: 0.0,
            traction: 1.0,
            water_depth_m: 0.0,
        };
        let mut state = TankKinematicState::default();
        drive(&mut state, &settings, 1.0, 0.0, contact, 600);
        state.forward_speed()
    };

    let flat = top_speed(0.0);
    // Negative forward slope = the ground ahead is lower = downhill.
    let downhill = top_speed(-0.2);

    assert!(
        downhill >= flat * 0.99,
        "descending must not be penalised relative to flat (downhill {downhill} vs flat {flat})"
    );
}

#[test]
fn full_lock_turn_stays_finite_and_bounded() {
    let settings = TankControllerSettings::from_spec(&TankSpec::t55a());
    let mut state = TankKinematicState::default();

    drive(&mut state, &settings, 1.0, 1.0, TerrainContact::flat(0.0), 2000);

    assert!(state.velocity.is_finite(), "velocity must stay finite, got {:?}", state.velocity);
    assert!(state.yaw_rate_rad_s.is_finite(), "yaw rate must stay finite");
    assert!(
        state.speed() <= settings.max_forward_speed_mps * 1.5,
        "speed must stay bounded, got {}",
        state.speed()
    );
    assert!(
        state.yaw_rate_rad_s.abs() <= settings.turn_rate_rad_s * 1.5,
        "yaw rate must stay bounded, got {}",
        state.yaw_rate_rad_s
    );
}
