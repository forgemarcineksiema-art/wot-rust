// Rapier itself left the workspace 2026-08-02 (audit D6: an API surface consumed only by its
// own tests). parry3d stays, narrowly, for the live footprint-intersection query - and the
// same determinism discipline that used to pin rapier's features now pins parry's: a geometry
// library on the authoritative server path must not quietly grow SIMD or parallelism.
use quality::workspace_root;
use std::fs;

#[test]
fn parry_stays_a_pinned_minimal_geometry_dependency() {
    let manifest =
        fs::read_to_string(workspace_root().join("Cargo.toml")).expect("workspace manifest exists");
    assert!(
        !manifest.contains("rapier"),
        "rapier3d was removed 2026-08-02; it does not come back as a side effect of a feature          bump - re-adding it is a design decision with its own tests"
    );
    let parry_line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("parry3d ="))
        .expect("workspace declares parry3d");

    assert!(parry_line.contains("default-features = false"));
    assert!(parry_line.contains("\"dim3\""));
    assert!(parry_line.contains("\"f32\""));
    assert!(!parry_line.contains("parallel"));
    assert!(!parry_line.contains("simd-stable"));
    assert!(!parry_line.contains("simd-nightly"));
}
