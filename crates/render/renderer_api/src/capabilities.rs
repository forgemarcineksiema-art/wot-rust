#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCapabilityTier {
    Tier0,
    Tier1,
    Tier2,
    Experimental,
}

impl RenderCapabilityTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tier0 => "tier0",
            Self::Tier1 => "tier1",
            Self::Tier2 => "tier2",
            Self::Experimental => "experimental",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    Noop,
    Vulkan,
    Metal,
    Dx12,
    Gl,
    BrowserWebGpu,
    Other,
}

impl GpuBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Dx12 => "dx12",
            Self::Gl => "gl",
            Self::BrowserWebGpu => "browser-webgpu",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuDeviceType {
    Other,
    IntegratedGpu,
    DiscreteGpu,
    VirtualGpu,
    Cpu,
}

impl GpuDeviceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::IntegratedGpu => "integrated-gpu",
            Self::DiscreteGpu => "discrete-gpu",
            Self::VirtualGpu => "virtual-gpu",
            Self::Cpu => "cpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureCompressionSupport {
    pub bc: bool,
    pub etc2: bool,
    pub astc: bool,
}

impl TextureCompressionSupport {
    pub fn any(self) -> bool {
        self.bc || self.etc2 || self.astc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLimitsSummary {
    pub max_texture_dimension_2d: u32,
    pub max_bind_groups: u32,
    pub max_sampled_textures_per_shader_stage: u32,
    pub max_storage_buffers_per_shader_stage: u32,
    pub max_color_attachments: u32,
    pub max_buffer_size: u64,
}

impl Default for RenderLimitsSummary {
    fn default() -> Self {
        Self {
            max_texture_dimension_2d: 8_192,
            max_bind_groups: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 8,
            max_color_attachments: 8,
            max_buffer_size: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderAdapterReport {
    pub adapter_name: String,
    pub backend: GpuBackend,
    pub device_type: GpuDeviceType,
    pub feature_names: Vec<String>,
    pub limits: RenderLimitsSummary,
    pub texture_compression: TextureCompressionSupport,
    pub timestamp_query_supported: bool,
    pub capability_tier: RenderCapabilityTier,
}

impl RenderAdapterReport {
    pub fn new(
        adapter_name: String,
        backend: GpuBackend,
        device_type: GpuDeviceType,
        feature_names: Vec<String>,
        limits: RenderLimitsSummary,
        texture_compression: TextureCompressionSupport,
        timestamp_query_supported: bool,
    ) -> Self {
        let capability_tier = classify_tier(
            device_type,
            &feature_names,
            limits,
            texture_compression,
            timestamp_query_supported,
        );
        Self {
            adapter_name,
            backend,
            device_type,
            feature_names,
            limits,
            texture_compression,
            timestamp_query_supported,
            capability_tier,
        }
    }

    pub fn startup_log_line(&self) -> String {
        format!(
            "adapter={} backend={} device_type={} tier={} features=[{}] limits={} compression={} timestamp_query={}",
            self.adapter_name,
            self.backend.as_str(),
            self.device_type.as_str(),
            self.capability_tier.as_str(),
            self.feature_names.join(","),
            self.limits.log_summary(),
            self.texture_compression.log_summary(),
            self.timestamp_query_supported
        )
    }
}

impl RenderLimitsSummary {
    pub fn log_summary(self) -> String {
        format!(
            "tex2d={} bind_groups={} sampled_textures={} storage_buffers={} color_attachments={} max_buffer={}",
            self.max_texture_dimension_2d,
            self.max_bind_groups,
            self.max_sampled_textures_per_shader_stage,
            self.max_storage_buffers_per_shader_stage,
            self.max_color_attachments,
            self.max_buffer_size
        )
    }
}

impl TextureCompressionSupport {
    pub fn log_summary(self) -> String {
        format!("bc={} etc2={} astc={}", self.bc, self.etc2, self.astc)
    }
}

fn classify_tier(
    device_type: GpuDeviceType,
    feature_names: &[String],
    limits: RenderLimitsSummary,
    compression: TextureCompressionSupport,
    timestamp_query_supported: bool,
) -> RenderCapabilityTier {
    if feature_names.iter().any(|feature| feature.starts_with("experimental:")) {
        return RenderCapabilityTier::Experimental;
    }

    if device_type == GpuDeviceType::Cpu
        || limits.max_texture_dimension_2d < 8_192
        || limits.max_bind_groups < 4
    {
        return RenderCapabilityTier::Tier0;
    }

    let strong_limits = limits.max_texture_dimension_2d >= 16_384
        && limits.max_bind_groups >= 8
        && limits.max_sampled_textures_per_shader_stage >= 32
        && limits.max_storage_buffers_per_shader_stage >= 16
        && limits.max_buffer_size >= 512 * 1024 * 1024;

    if strong_limits && compression.any() && timestamp_query_supported {
        RenderCapabilityTier::Tier2
    } else {
        RenderCapabilityTier::Tier1
    }
}
