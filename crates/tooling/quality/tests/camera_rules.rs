// Doc-existence and phrase-grep tests removed by decision (battle-first, 2026-08-01): the gate
// holds rules about code. The camera policy lives at `docs/battle-camera-policy.md`, cited here.
use quality::workspace_root;
use std::fs;

#[test]
fn client_render_path_uses_battle_camera_not_default_placeholder() {
    let dispatch = fs::read_to_string(workspace_root().join("crates/apps/client/src/app/mod.rs"))
        .expect("client app should be readable");
    let render = fs::read_to_string(workspace_root().join("crates/apps/client/src/app/render.rs"))
        .expect("client render path should be readable");
    let camera_link =
        fs::read_to_string(workspace_root().join("crates/apps/client/src/app/camera_link.rs"))
            .expect("client camera plumbing should be readable");

    assert!(dispatch.contains("BattleCameraController"));
    assert!(
        render.contains("presented_camera_for_player") && camera_link.contains("present("),
        "the render path must draw through the presented battle camera"
    );
    assert!(
        camera_link.contains("render_camera"),
        "the aim path must keep the unfiltered battle camera"
    );
    assert!(!render.contains("objects: Vec::new()"), "render path must not submit an empty frame");
}
