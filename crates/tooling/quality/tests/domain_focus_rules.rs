use quality::workspace_root;
use std::fs;

#[test]
fn domain_focus_doc_is_required() {
    assert!(
        workspace_root().join("docs/armored-battle-domain.md").is_file(),
        "missing docs/armored-battle-domain.md"
    );
}

#[test]
fn architecture_doc_rejects_general_purpose_engine_direction() {
    let root = workspace_root();
    let architecture = fs::read_to_string(root.join("docs/architecture.md"))
        .expect("architecture doc should be readable");

    assert!(architecture.contains("armored vehicle battles"));
    assert!(architecture.contains("not a general-purpose engine"));
    assert!(
        architecture.contains("terrain, LOD, shadows, spotting, shell physics, and networking")
    );
}

#[test]
fn readme_names_the_project_domain_before_the_stack() {
    let readme =
        fs::read_to_string(workspace_root().join("README.md")).expect("README should be readable");

    assert!(readme.contains("armored vehicle battles on large terrain maps"));
}
