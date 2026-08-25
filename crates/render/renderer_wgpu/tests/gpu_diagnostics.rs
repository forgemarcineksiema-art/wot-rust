use renderer_wgpu::{GpuErrorPolicy, WgpuLabelPolicy};

#[test]
fn wgpu_backend_has_required_renderdoc_labels() {
    let labels = WgpuLabelPolicy::required_startup_labels();

    for required in [
        "terrain_depth_prepass_pipeline",
        "tank_pbr_pipeline",
        "shell_tracer_vertex_buffer",
        "shadow_map_2048",
    ] {
        assert!(labels.contains(&required), "missing required GPU label: {required}");
        assert!(WgpuLabelPolicy::is_valid_label(required));
    }
}

#[test]
fn gpu_error_policy_installs_a_handler_that_logs_rather_than_aborts() {
    let policy = GpuErrorPolicy::default();

    assert!(policy.uses_error_scopes());
    // The device-lost callback and uncaptured-error handler are wired in `gpu_context`.
    assert!(policy.installs_uncaptured_error_handler());
    // ...and they LOG, not abort — a shipped game must not crash on a transient driver quirk.
    assert!(!policy.uncaptured_errors_are_fatal());
}
