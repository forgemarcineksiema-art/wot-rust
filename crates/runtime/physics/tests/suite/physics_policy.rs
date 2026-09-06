//! The one physics promise worth a test: the custom controller is DETERMINISTIC — same
//! inputs, same state, bit for bit. The two "ownership policy" tests that lived here died
//! with `policy.rs` (2026-08-02): they asserted that a const fn returns the constants it
//! was written to return, about a rapier integration that never existed.

use physics::{
    TankControlInput, TankControllerSettings, TankKinematicState, step_custom_tank_controller,
};

#[test]
fn custom_tank_controller_replays_same_inputs_exactly() {
    let inputs = [
        TankControlInput { throttle: 1.0, steer: 0.25, brake: 0.0 },
        TankControlInput { throttle: 0.5, steer: -0.1, brake: 0.0 },
        TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 },
    ];

    let first = replay_controller(inputs);
    let second = replay_controller(inputs);

    assert_eq!(first, second);
}

fn replay_controller(inputs: [TankControlInput; 3]) -> TankKinematicState {
    let settings = TankControllerSettings::arcade_default();
    let mut state = TankKinematicState::default();
    for input in inputs {
        step_custom_tank_controller(&mut state, input, &settings, 1.0 / 60.0);
    }
    state
}
