use renderer_api::{
    GpuBackend, GpuDeviceType, RenderAdapterReport, RenderCapabilityTier, RenderLimitsSummary,
    TextureCompressionSupport,
};

#[test]
fn tier0_covers_low_end_or_software_paths() {
    let report = report_with(
        GpuBackend::Gl,
        GpuDeviceType::Cpu,
        TextureCompressionSupport::default(),
        false,
        RenderLimitsSummary {
            max_texture_dimension_2d: 4_096,
            max_bind_groups: 4,
            max_sampled_textures_per_shader_stage: 8,
            max_storage_buffers_per_shader_stage: 4,
            max_color_attachments: 4,
            max_buffer_size: 128 * 1024 * 1024,
        },
        Vec::new(),
    );

    assert_eq!(report.capability_tier, RenderCapabilityTier::Tier0);
}

#[test]
fn tier1_is_default_desktop_gaming_path() {
    let report = report_with(
        GpuBackend::Dx12,
        GpuDeviceType::DiscreteGpu,
        TextureCompressionSupport { bc: true, etc2: false, astc: false },
        false,
        RenderLimitsSummary::default(),
        vec!["texture-compression-bc".to_string()],
    );

    assert_eq!(report.capability_tier, RenderCapabilityTier::Tier1);
}

#[test]
fn tier2_requires_stronger_limits_compression_and_timestamps() {
    let report = report_with(
        GpuBackend::Vulkan,
        GpuDeviceType::DiscreteGpu,
        TextureCompressionSupport { bc: true, etc2: true, astc: true },
        true,
        RenderLimitsSummary {
            max_texture_dimension_2d: 16_384,
            max_bind_groups: 8,
            max_sampled_textures_per_shader_stage: 32,
            max_storage_buffers_per_shader_stage: 16,
            max_color_attachments: 8,
            max_buffer_size: 512 * 1024 * 1024,
        },
        vec!["timestamp-query".to_string()],
    );

    assert_eq!(report.capability_tier, RenderCapabilityTier::Tier2);
    assert!(report.startup_log_line().contains("timestamp_query=true"));
}

#[test]
fn experimental_features_select_experimental_path() {
    let report = report_with(
        GpuBackend::Vulkan,
        GpuDeviceType::DiscreteGpu,
        TextureCompressionSupport { bc: true, etc2: true, astc: true },
        true,
        RenderLimitsSummary::default(),
        vec!["experimental:mesh-shader".to_string()],
    );

    assert_eq!(report.capability_tier, RenderCapabilityTier::Experimental);
}

fn report_with(
    backend: GpuBackend,
    device_type: GpuDeviceType,
    texture_compression: TextureCompressionSupport,
    timestamp_query: bool,
    limits: RenderLimitsSummary,
    features: Vec<String>,
) -> RenderAdapterReport {
    RenderAdapterReport::new(
        "test adapter".to_string(),
        backend,
        device_type,
        features,
        limits,
        texture_compression,
        timestamp_query,
    )
}
