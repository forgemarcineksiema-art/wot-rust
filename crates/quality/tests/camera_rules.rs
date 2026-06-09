use std::fs;
use std::path::PathBuf;

#[test]
fn battle_camera_policy_doc_is_required() {
    assert!(
        workspace_root().join("docs/battle-camera-policy.md").is_file(),
        "missing docs/battle-camera-policy.md"
    );
}

#[test]
fn battle_camera_policy_names_core_constraints() {
    let doc = fs::read_to_string(workspace_root().join("docs/battle-camera-policy.md"))
        .expect("battle camera policy doc should be readable");

    for required in [
        "Third-person",
        "Sniper",
        "terrain clearance",
        "static cover collision",
        "turret yaw",
        "gun pitch",
        "server snapshots",
        "not own authoritative simulation",
    ] {
        assert!(doc.contains(required), "missing camera policy phrase: {required}");
    }
}

#[test]
fn client_render_path_uses_battle_camera_not_default_placeholder() {
    let dispatch = fs::read_to_string(workspace_root().join("crates/client/src/app/mod.rs"))
        .expect("client app should be readable");
    let render = fs::read_to_string(workspace_root().join("crates/client/src/app/render.rs"))
        .expect("client render path should be readable");

    assert!(dispatch.contains("BattleCameraController"));
    assert!(render.contains("render_camera"), "render path must build the battle camera");
    assert!(!render.contains("objects: Vec::new()"), "render path must not submit an empty frame");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("quality crate should live under crates/quality")
        .to_path_buf()
}
