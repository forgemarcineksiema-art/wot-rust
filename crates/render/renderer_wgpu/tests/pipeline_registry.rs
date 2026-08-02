use renderer_api::{AlphaMode, PipelineKey, RenderSettings};
use renderer_wgpu::{PipelineRegistry, WgpuRenderer};

#[test]
fn draw_call_requires_a_prewarmed_pipeline() {
    let registry = PipelineRegistry::default();
    let missing = PipelineKey::tank_baseline(AlphaMode::Opaque);

    let error = registry
        .require_for_draw(&missing)
        .expect_err("draw call must not compile missing pipelines");

    assert!(error.message.contains("prewarm"));
    // ONE, not zero. This is the assertion that proves the counter is an instrument rather than a
    // decoration: until 2026-08-02 the field behind it was never written, so all four tests in
    // this file were asserting `usize::default()` and would have passed with the registry gutted.
    assert_eq!(
        registry.compilation_requests_during_draw(),
        1,
        "a draw that misses the cache is exactly the mid-frame compile this counter exists to see"
    );
}

#[test]
fn prewarm_registers_pipeline_keys_before_draw() {
    let mut registry = PipelineRegistry::default();
    let key = PipelineKey::tank_baseline(AlphaMode::Opaque);

    let stats = registry.prewarm([key]);

    assert_eq!(stats.created, 1);
    assert!(registry.require_for_draw(&key).is_ok());
    assert_eq!(registry.cached_pipeline_count(), 1);
    assert_eq!(registry.compilation_requests_during_draw(), 0);
}

#[test]
fn dev_hot_reload_replaces_cached_key_outside_draw() {
    let mut registry = PipelineRegistry::default();
    let key = PipelineKey::tank_baseline(AlphaMode::Masked);

    registry.prewarm([key]);
    let stats = registry.hot_reload([key]);

    assert_eq!(stats.replaced, 1);
    assert!(registry.require_for_draw(&key).is_ok());
    assert_eq!(registry.compilation_requests_during_draw(), 0);
}

#[test]
fn renderer_prewarms_baseline_pipelines_on_startup() {
    let renderer = WgpuRenderer::new(RenderSettings::default());
    let key = PipelineKey::tank_baseline(AlphaMode::Opaque);

    assert!(renderer.require_pipeline_for_draw(&key).is_ok());
    assert_eq!(renderer.pipeline_registry().compilation_requests_during_draw(), 0);
}
