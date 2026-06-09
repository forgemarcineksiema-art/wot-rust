use std::{fs, path::PathBuf};

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
    let drive = fs::read_to_string(root.join("crates/sim/src/tank_drive.rs"))
        .expect("sim tank-drive source");

    // The shared drive step runs the custom controller via the physics world step (the same step
    // the client predictor calls), not rapier.
    assert!(drive.contains("TankControllerSettings::from_spec"));
    assert!(drive.contains("step_tank_on_world_with_tanks"));
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("quality crate should live under crates/quality")
        .to_path_buf()
}
