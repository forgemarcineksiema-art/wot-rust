use quality::workspace_root;
use std::fs;

#[test]
fn vehicle_movement_policy_doc_is_required() {
    let root = workspace_root();
    let doc = fs::read_to_string(root.join("docs/vehicle-movement-policy.md"))
        .expect("vehicle movement policy doc must exist");

    for required in [
        "TankSpec",
        "power-to-weight",
        "terrain contact",
        "heightmap",
        "slope",
        "roughness",
        "brake",
        "fixed tick",
        "server authoritative",
    ] {
        assert!(doc.contains(required), "movement doc missing phrase: {required}");
    }
}

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
