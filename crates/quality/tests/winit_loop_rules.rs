use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn client_uses_winit_application_handler_model() {
    let root = workspace_root();
    let app_mod =
        fs::read_to_string(root.join("crates/client/src/app/mod.rs")).expect("client app exists");
    let lifecycle = fs::read_to_string(root.join("crates/client/src/app/lifecycle.rs"))
        .expect("client lifecycle exists");

    assert!(lifecycle.contains("ApplicationHandler"));
    assert!(lifecycle.contains(".with_maximized(true)"));
    assert!(!lifecycle.contains("Fullscreen"));
    assert!(app_mod.contains("run_app"));
    assert!(lifecycle.contains("about_to_wait"));
    assert!(lifecycle.contains("RedrawRequested"));
    assert!(!app_mod.contains("poll_events"));
    assert!(!lifecycle.contains("poll_events"));
}

#[test]
fn client_has_testable_fixed_tick_loop_driver() {
    let root = workspace_root();
    let loop_policy = fs::read_to_string(root.join("crates/client/src/loop_policy.rs"))
        .expect("loop policy exists");

    assert!(loop_policy.contains("FixedTickAccumulator"));
    assert!(loop_policy.contains("ClientLoopEvent"));
    assert!(loop_policy.contains("ClientLoopAction"));
    assert!(loop_policy.contains("uses_manual_event_polling() -> bool"));
    assert!(!loop_policy.contains("poll_events"));
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("quality crate should live under crates/quality")
        .to_path_buf()
}
