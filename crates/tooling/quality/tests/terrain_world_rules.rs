use quality::workspace_root;
use std::fs;

#[test]
fn terrain_large_world_policy_doc_is_required() {
    assert!(
        workspace_root().join("docs/terrain-large-world-policy.md").is_file(),
        "missing docs/terrain-large-world-policy.md"
    );
}

#[test]
fn terrain_policy_doc_names_all_map_systems_and_precision_rules() {
    let doc = fs::read_to_string(workspace_root().join("docs/terrain-large-world-policy.md"))
        .expect("terrain policy doc should be readable");

    for required in [
        "heightmap / terrain chunks",
        "collision terrain",
        "render terrain LOD",
        "splat maps",
        "roads",
        "decorations",
        "cover objects",
        "spawn points",
        "capture zones",
        "navmesh / bot navigation",
        "occlusion/visibility sectors",
        "minimap data",
        "f32",
        "origin rebasing",
        "depth range [0, 1]",
    ] {
        assert!(doc.contains(required), "missing terrain policy phrase: {required}");
    }
}
