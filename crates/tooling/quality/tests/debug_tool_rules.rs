use std::fs;
use std::path::PathBuf;

#[test]
fn debug_tools_policy_doc_is_required() {
    assert!(
        workspace_root().join("docs/debug-tools-policy.md").is_file(),
        "missing docs/debug-tools-policy.md"
    );
}

#[test]
fn debug_tools_policy_names_required_first_week_tools_and_wgpu_error_rules() {
    let doc = fs::read_to_string(workspace_root().join("docs/debug-tools-policy.md"))
        .expect("debug tools policy doc should be readable");

    for required in [
        "debug draw line/box/sphere",
        "raycast visualizer",
        "hitbox/armor plate overlay",
        "penetration normal overlay",
        "server snapshot inspector",
        "entity inspector",
        "GPU frame timing",
        "CPU profiler markers",
        "asset reload",
        "free camera",
        "network stats overlay",
        "terrain_depth_prepass_pipeline",
        "tank_pbr_pipeline",
        "shell_tracer_vertex_buffer",
        "shadow_map_2048",
        "push_error_scope",
        "on_uncaptured_error",
    ] {
        assert!(doc.contains(required), "missing debug policy phrase: {required}");
    }
}

fn workspace_root() -> PathBuf {
    // Layout-agnostic: the nearest ancestor whose Cargo.toml declares [workspace].
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
    {
        assert!(dir.pop(), "a Cargo.toml with [workspace] should exist in an ancestor");
    }
    dir
}
