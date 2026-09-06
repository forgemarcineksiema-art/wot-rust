//! The diagnostics POLICY types describe the renderer; these tests hold them to its source, so
//! a label the policy requires is a label a resource carries, and an error handler the policy
//! claims is a call the device is created with. Before this the label list named four labels
//! nothing had ever carried and its test compared the list against itself.

use std::path::Path;

use renderer_wgpu::{GpuErrorPolicy, WgpuLabelPolicy};

fn crate_source() -> String {
    fn walk(dir: &Path, out: &mut String) {
        for entry in std::fs::read_dir(dir).expect("renderer_wgpu/src is readable") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push_str(&std::fs::read_to_string(&path).expect("source is readable"));
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    out
}

/// Every label the policy requires is a label the code gives a resource — as a string literal
/// somewhere in this crate — and is well-formed. A required label that no resource carries is
/// the lie this list used to be.
#[test]
fn every_required_gpu_label_is_one_the_renderer_actually_gives() {
    let source = crate_source();
    for required in WgpuLabelPolicy::required_startup_labels() {
        assert!(WgpuLabelPolicy::is_valid_label(required), "{required} is not snake case");
        assert!(
            source.contains(&format!("\"{required}\"")),
            "the policy requires the GPU label `{required}` but no resource in renderer_wgpu \
             carries it"
        );
    }
}

/// The error policy describes the device that `gpu_context` creates: a device-lost callback and
/// an uncaptured-error handler that log, no error scopes. Each claim is checked against the
/// source, so the struct cannot drift back into promising a handler that does not exist.
#[test]
fn the_gpu_error_policy_describes_the_handlers_the_device_is_created_with() {
    let policy = GpuErrorPolicy::default();
    let source = crate_source();

    assert_eq!(
        policy.installs_uncaptured_error_handler(),
        source.contains("on_uncaptured_error("),
        "the policy and the source disagree about the uncaptured-error handler"
    );
    assert!(source.contains("set_device_lost_callback("), "the lost-device callback is wired");
    assert_eq!(
        policy.uses_error_scopes(),
        source.contains("push_error_scope("),
        "the policy and the source disagree about error scopes"
    );
    // ...and they LOG, not abort — a shipped game must not crash on a transient driver quirk.
    assert!(!policy.uncaptured_errors_are_fatal());
}
