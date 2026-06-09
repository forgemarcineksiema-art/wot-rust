use renderer_api::{
    GpuBackend, GpuDeviceType, RenderAdapterReport, RenderFeature, RenderLimitsSummary,
    TextureCompressionSupport, select_render_feature_plan,
};

#[test]
fn baseline_plan_uses_boring_cross_platform_features() {
    let plan = select_render_feature_plan(&tier0_report(), &[]);

    for feature in [
        RenderFeature::ForwardPlus,
        RenderFeature::ShadowMaps,
        RenderFeature::TerrainChunks,
        RenderFeature::Instancing,
        RenderFeature::Lod,
        RenderFeature::FrustumCulling,
        RenderFeature::OcclusionCulling,
        RenderFeature::Particles,
        RenderFeature::PostProcess,
        RenderFeature::Ui,
        RenderFeature::DebugDraw,
    ] {
        assert!(plan.has_feature(feature), "missing baseline feature: {feature:?}");
    }

    assert!(!plan.has_feature(RenderFeature::RayTracing));
    assert!(!plan.has_feature(RenderFeature::MeshShaders));
    assert!(!plan.has_feature(RenderFeature::BindlessResources));
    assert!(!plan.has_feature(RenderFeature::GpuDrivenRendering));
}

#[test]
fn unsupported_high_end_requests_fall_back_to_baseline_features() {
    let plan = select_render_feature_plan(
        &tier0_report(),
        &[
            RenderFeature::RayTracing,
            RenderFeature::MeshShaders,
            RenderFeature::BindlessResources,
            RenderFeature::GpuDrivenRendering,
        ],
    );

    assert_eq!(plan.fallback_for(RenderFeature::RayTracing), Some(RenderFeature::ShadowMaps));
    assert_eq!(plan.fallback_for(RenderFeature::MeshShaders), Some(RenderFeature::Instancing));
    assert_eq!(
        plan.fallback_for(RenderFeature::BindlessResources),
        Some(RenderFeature::ForwardPlus)
    );
    assert_eq!(
        plan.fallback_for(RenderFeature::GpuDrivenRendering),
        Some(RenderFeature::FrustumCulling)
    );
}

#[test]
fn experimental_features_only_enable_on_explicit_experimental_report() {
    let report = RenderAdapterReport::new(
        "native experimental".to_string(),
        GpuBackend::Vulkan,
        GpuDeviceType::DiscreteGpu,
        vec!["experimental:ray-query".to_string(), "experimental:mesh-shader".to_string()],
        RenderLimitsSummary {
            max_texture_dimension_2d: 16_384,
            max_bind_groups: 8,
            max_sampled_textures_per_shader_stage: 32,
            max_storage_buffers_per_shader_stage: 16,
            max_color_attachments: 8,
            max_buffer_size: 512 * 1024 * 1024,
        },
        TextureCompressionSupport { bc: true, etc2: false, astc: false },
        true,
    );
    let plan = select_render_feature_plan(
        &report,
        &[RenderFeature::RayTracing, RenderFeature::MeshShaders],
    );

    assert!(plan.has_feature(RenderFeature::RayTracing));
    assert!(plan.has_feature(RenderFeature::MeshShaders));
    assert!(plan.fallbacks.is_empty());
}

fn tier0_report() -> RenderAdapterReport {
    RenderAdapterReport::new(
        "low spec".to_string(),
        GpuBackend::Gl,
        GpuDeviceType::IntegratedGpu,
        Vec::new(),
        RenderLimitsSummary {
            max_texture_dimension_2d: 4_096,
            max_bind_groups: 4,
            max_sampled_textures_per_shader_stage: 8,
            max_storage_buffers_per_shader_stage: 4,
            max_color_attachments: 4,
            max_buffer_size: 128 * 1024 * 1024,
        },
        TextureCompressionSupport::default(),
        false,
    )
}
