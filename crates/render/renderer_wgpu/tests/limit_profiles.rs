use renderer_api::{RenderFeature, RenderLimitProfile, RenderSettings};
use renderer_wgpu::{
    WgpuRenderer, renderer_high_limits, renderer_low_spec_limits, renderer_recommended_limits,
    request_limits_for_profile,
};

#[test]
fn low_spec_limits_are_not_just_adapter_limits() {
    let adapter_limits = wgpu::Limits {
        max_texture_dimension_2d: 16_384,
        max_bind_groups: 8,
        max_sampled_textures_per_shader_stage: 32,
        max_storage_buffers_per_shader_stage: 16,
        max_color_attachments: 8,
        max_buffer_size: 512 * 1024 * 1024,
        ..Default::default()
    };

    let requested = renderer_low_spec_limits();

    assert_ne!(requested.max_texture_dimension_2d, adapter_limits.max_texture_dimension_2d);
    assert_ne!(requested.max_bind_groups, adapter_limits.max_bind_groups);
    assert!(requested.max_texture_dimension_2d <= adapter_limits.max_texture_dimension_2d);
    assert!(requested.max_bind_groups <= adapter_limits.max_bind_groups);
}

#[test]
fn limit_profiles_are_monotonic_for_renderer_needs() {
    let low = renderer_low_spec_limits();
    let recommended = renderer_recommended_limits();
    let high = renderer_high_limits();

    assert!(low.max_texture_dimension_2d <= recommended.max_texture_dimension_2d);
    assert!(recommended.max_texture_dimension_2d <= high.max_texture_dimension_2d);
    assert!(low.max_bind_groups <= recommended.max_bind_groups);
    assert!(recommended.max_bind_groups <= high.max_bind_groups);
    assert!(
        low.max_sampled_textures_per_shader_stage
            <= recommended.max_sampled_textures_per_shader_stage
    );
    assert!(
        recommended.max_sampled_textures_per_shader_stage
            <= high.max_sampled_textures_per_shader_stage
    );
}

#[test]
fn renderer_can_start_with_minimal_limit_profile() {
    let renderer = WgpuRenderer::new_with_limit_profile(
        RenderSettings::default(),
        RenderLimitProfile::LowSpec,
    );

    assert_eq!(renderer.limit_profile(), RenderLimitProfile::LowSpec);
    assert_eq!(renderer.requested_limits(), &renderer_low_spec_limits());
    assert!(renderer.feature_plan().has_feature(RenderFeature::ForwardPlus));
    assert!(!renderer.feature_plan().has_feature(RenderFeature::RayTracing));
}

#[test]
fn profile_selector_returns_explicit_request_limits() {
    assert_eq!(request_limits_for_profile(RenderLimitProfile::LowSpec), renderer_low_spec_limits());
    assert_eq!(
        request_limits_for_profile(RenderLimitProfile::Recommended),
        renderer_recommended_limits()
    );
    assert_eq!(request_limits_for_profile(RenderLimitProfile::High), renderer_high_limits());
}
