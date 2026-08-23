// The gate holds rules about code. The movement policy lives at `docs/vehicle-movement-policy.md`,
// cited here.
use quality::workspace_root;
use std::fs;

#[test]
fn sim_uses_custom_physics_movement_model() {
    let root = workspace_root();
    let drive = fs::read_to_string(root.join("crates/runtime/sim/src/tank_drive.rs"))
        .expect("sim tank-drive source");

    // The shared drive step runs the custom controller via the physics world step (the same step
    // the client predictor calls), not rapier. The world step comes in two halves — decide a
    // velocity, then spend it — so the roster can solve hull-to-hull contacts in between; both
    // halves are `physics`, which is the rule this gate is actually about.
    assert!(drive.contains("TankControllerSettings::from_spec"));
    assert!(drive.contains("physics::advance_tank_on_world"));
    assert!(drive.contains("physics::settle_tank_on_world"));
}
