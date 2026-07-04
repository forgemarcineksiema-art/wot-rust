mod gpu_context;
mod gpu_diagnostics;
mod gpu_layout;
mod msaa;
mod offscreen;
mod pipeline_registry;
mod readback_queue;
mod render_frame_batch;
mod renderer;
mod scene_pipeline;
mod scene_renderer;
mod scene_resources;
mod scene_target;
mod shader_validation;
mod surface_config;
mod texture_upload;
mod upload_buffers;
mod vehicle_pipeline;
mod vehicle_resources;
mod window_renderer;

use renderer_api::{
    GpuBackend, GpuDeviceType, RenderAdapterReport, RenderError, RenderLimitProfile,
    RenderLimitsSummary, TextureCompressionSupport,
};

pub use gpu_context::GpuContext;
pub use gpu_diagnostics::{GpuErrorPolicy, WgpuLabelPolicy};
pub use gpu_layout::{
    CameraUniform, GpuMat4, GpuVec3, TankVertex, encode_camera_uniform, tank_vertex_bytes,
};
pub use offscreen::{DEPTH_FORMAT, OffscreenTarget, clear_color};
pub use pipeline_registry::{PipelineHotReloadStats, PipelineRegistry, PipelineWarmupStats};
pub use readback_queue::{GpuReadbackQueue, ReadbackRequest, ReadbackRequestId, ReadbackResult};
pub use render_frame_batch::{RenderFrameBatchPlan, RenderObjectDraw};
pub use renderer::WgpuRenderer;
pub use scene_pipeline::scene_shader_source;
pub use scene_renderer::SceneRenderer;
pub use scene_renderer::env_group::build_shadow_bind_group_layout;
pub use scene_renderer::shadow::shadow_shader_source;
pub use scene_renderer::ssao::ssao_shader_source;
pub use scene_target::SceneRenderTarget;
pub use shader_validation::{
    WgslUniformBinding, WgslValidationReport, basic_tank_shader_source, validate_wgsl_shader,
};
pub use surface_config::select_present_mode;
pub use texture_upload::{
    TextureExtent, TextureFormat, TextureUpload, TextureUploadId, TextureUploadQueue,
};
pub use upload_buffers::{
    DynamicUniformAllocation, DynamicUniformRingBuffer, FrameUploadArena, InstanceBatch,
    InstanceBatchKind, InstanceBufferAllocator, UploadBatch,
};
pub use vehicle_pipeline::{build_vehicle_pipeline, vehicle_shader_source};
pub use vehicle_resources::{GpuVehicleMesh, VehicleMeshRegistry};
pub use window_renderer::WindowRenderer;

pub fn probe_startup_report(instance: &wgpu::Instance) -> Result<RenderAdapterReport, RenderError> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|error| RenderError::new(format!("failed to request wgpu adapter: {error}")))?;

    Ok(adapter_report_from_wgpu(adapter.get_info(), adapter.features(), &adapter.limits()))
}

pub fn adapter_report_from_wgpu(
    info: wgpu::AdapterInfo,
    features: wgpu::Features,
    limits: &wgpu::Limits,
) -> RenderAdapterReport {
    RenderAdapterReport::new(
        info.name,
        map_backend(info.backend),
        map_device_type(info.device_type),
        feature_names(features),
        map_limits(limits),
        texture_compression_support(features),
        features.contains(wgpu::Features::TIMESTAMP_QUERY),
    )
}

pub fn request_limits_for_profile(profile: RenderLimitProfile) -> wgpu::Limits {
    match profile {
        RenderLimitProfile::LowSpec => renderer_low_spec_limits(),
        RenderLimitProfile::Recommended => renderer_recommended_limits(),
        RenderLimitProfile::High => renderer_high_limits(),
    }
}

pub fn renderer_low_spec_limits() -> wgpu::Limits {
    wgpu::Limits::downlevel_webgl2_defaults()
}

pub fn renderer_recommended_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: 8_192,
        max_bind_groups: 4,
        max_sampled_textures_per_shader_stage: 16,
        max_storage_buffers_per_shader_stage: 8,
        max_color_attachments: 8,
        max_buffer_size: 256 * 1024 * 1024,
        ..renderer_low_spec_limits()
    }
}

pub fn renderer_high_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: 16_384,
        max_bind_groups: 8,
        max_sampled_textures_per_shader_stage: 32,
        max_storage_buffers_per_shader_stage: 16,
        max_color_attachments: 8,
        max_buffer_size: 512 * 1024 * 1024,
        ..renderer_recommended_limits()
    }
}

fn map_backend(backend: wgpu::Backend) -> GpuBackend {
    match backend {
        wgpu::Backend::Noop => GpuBackend::Noop,
        wgpu::Backend::Vulkan => GpuBackend::Vulkan,
        wgpu::Backend::Metal => GpuBackend::Metal,
        wgpu::Backend::Dx12 => GpuBackend::Dx12,
        wgpu::Backend::Gl => GpuBackend::Gl,
        wgpu::Backend::BrowserWebGpu => GpuBackend::BrowserWebGpu,
    }
}

fn map_device_type(device_type: wgpu::DeviceType) -> GpuDeviceType {
    match device_type {
        wgpu::DeviceType::Other => GpuDeviceType::Other,
        wgpu::DeviceType::IntegratedGpu => GpuDeviceType::IntegratedGpu,
        wgpu::DeviceType::DiscreteGpu => GpuDeviceType::DiscreteGpu,
        wgpu::DeviceType::VirtualGpu => GpuDeviceType::VirtualGpu,
        wgpu::DeviceType::Cpu => GpuDeviceType::Cpu,
    }
}

fn map_limits(limits: &wgpu::Limits) -> RenderLimitsSummary {
    RenderLimitsSummary {
        max_texture_dimension_2d: limits.max_texture_dimension_2d,
        max_bind_groups: limits.max_bind_groups,
        max_sampled_textures_per_shader_stage: limits.max_sampled_textures_per_shader_stage,
        max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
        max_color_attachments: limits.max_color_attachments,
        max_buffer_size: limits.max_buffer_size,
    }
}

fn texture_compression_support(features: wgpu::Features) -> TextureCompressionSupport {
    TextureCompressionSupport {
        bc: features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
        etc2: features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2),
        astc: features.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC),
    }
}

fn feature_names(features: wgpu::Features) -> Vec<String> {
    [
        (wgpu::Features::TEXTURE_COMPRESSION_BC, "texture-compression-bc"),
        (wgpu::Features::TEXTURE_COMPRESSION_ETC2, "texture-compression-etc2"),
        (wgpu::Features::TEXTURE_COMPRESSION_ASTC, "texture-compression-astc"),
        (wgpu::Features::TIMESTAMP_QUERY, "timestamp-query"),
        (wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS, "timestamp-query-inside-encoders"),
        (wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES, "timestamp-query-inside-passes"),
        (wgpu::Features::TEXTURE_BINDING_ARRAY, "texture-binding-array"),
        (wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY, "storage-resource-binding-array"),
        (wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY, "partially-bound-binding-array"),
        (wgpu::Features::EXPERIMENTAL_RAY_QUERY, "experimental:ray-query"),
        (wgpu::Features::EXPERIMENTAL_MESH_SHADER, "experimental:mesh-shader"),
    ]
    .into_iter()
    .filter(|(flag, _name)| features.contains(*flag))
    .map(|(_flag, name)| name.to_string())
    .collect()
}
