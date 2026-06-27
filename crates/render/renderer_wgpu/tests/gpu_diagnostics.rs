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
fn gpu_error_policy_requires_scopes_and_uncaptured_handler() {
    let policy = GpuErrorPolicy::default();

    assert!(policy.uses_error_scopes());
    assert!(policy.installs_uncaptured_error_handler());
    assert!(policy.uncaptured_errors_are_fatal());
}
