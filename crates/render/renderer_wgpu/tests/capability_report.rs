use renderer_api::{GpuBackend, GpuDeviceType, RenderCapabilityTier, TextureCompressionSupport};
use renderer_wgpu::adapter_report_from_wgpu;

#[test]
fn wgpu_adapter_data_is_mapped_to_renderer_api_report() {
    let info = wgpu::AdapterInfo {
        name: "Test RTX".to_string(),
        vendor: 0,
        device: 0,
        device_type: wgpu::DeviceType::DiscreteGpu,
        device_pci_bus_id: String::new(),
        driver: "test-driver".to_string(),
        driver_info: "test-driver-info".to_string(),
        backend: wgpu::Backend::Dx12,
        subgroup_min_size: 32,
        subgroup_max_size: 32,
        transient_saves_memory: false,
    };
    let features = wgpu::Features::TEXTURE_COMPRESSION_BC
        | wgpu::Features::TEXTURE_COMPRESSION_ASTC
        | wgpu::Features::TIMESTAMP_QUERY;
    let limits = wgpu::Limits {
        max_texture_dimension_2d: 16_384,
        max_bind_groups: 8,
        max_sampled_textures_per_shader_stage: 32,
        max_storage_buffers_per_shader_stage: 16,
        max_color_attachments: 8,
        max_buffer_size: 512 * 1024 * 1024,
        ..Default::default()
    };

    let report = adapter_report_from_wgpu(info, features, &limits);

    assert_eq!(report.adapter_name, "Test RTX");
    assert_eq!(report.backend, GpuBackend::Dx12);
    assert_eq!(report.device_type, GpuDeviceType::DiscreteGpu);
    assert_eq!(
        report.texture_compression,
        TextureCompressionSupport { bc: true, etc2: false, astc: true }
    );
    assert!(report.timestamp_query_supported);
    assert_eq!(report.capability_tier, RenderCapabilityTier::Tier2);
}

#[test]
fn native_experimental_features_are_named_for_fallback_policy() {
    let info = wgpu::AdapterInfo {
        name: "Experimental Vulkan".to_string(),
        vendor: 0,
        device: 0,
        device_type: wgpu::DeviceType::DiscreteGpu,
        device_pci_bus_id: String::new(),
        driver: "test-driver".to_string(),
        driver_info: "test-driver-info".to_string(),
        backend: wgpu::Backend::Vulkan,
        subgroup_min_size: 32,
        subgroup_max_size: 32,
        transient_saves_memory: false,
    };
    let features = wgpu::Features::EXPERIMENTAL_RAY_QUERY
        | wgpu::Features::EXPERIMENTAL_MESH_SHADER
        | wgpu::Features::TEXTURE_BINDING_ARRAY
        | wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY
        | wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY;

    let report = adapter_report_from_wgpu(info, features, &wgpu::Limits::default());

    assert!(report.feature_names.contains(&"experimental:ray-query".to_string()));
    assert!(report.feature_names.contains(&"experimental:mesh-shader".to_string()));
    assert!(report.feature_names.contains(&"texture-binding-array".to_string()));
    assert!(report.feature_names.contains(&"storage-resource-binding-array".to_string()));
    assert!(report.feature_names.contains(&"partially-bound-binding-array".to_string()));
}
