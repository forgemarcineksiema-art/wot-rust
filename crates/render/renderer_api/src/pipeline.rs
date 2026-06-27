use crate::DEFAULT_MSAA_SAMPLES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexLayoutKey {
    StaticMesh,
    SkinnedMesh,
    TerrainChunk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialPipelineFlags(pub u32);

impl MaterialPipelineFlags {
    pub const NONE: Self = Self(0);
    pub const ALBEDO_TEXTURED: Self = Self(1 << 0);
    pub const NORMAL_MAPPED: Self = Self(1 << 1);
    pub const EMISSIVE: Self = Self(1 << 2);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorFormat {
    Bgra8UnormSrgb,
    Rgba8UnormSrgb,
    Rgba16Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthFormat {
    Depth32Float,
    Depth24Plus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlphaMode {
    Opaque,
    Masked,
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub shader: ShaderHandle,
    pub vertex_layout: VertexLayoutKey,
    pub material_flags: MaterialPipelineFlags,
    pub color_format: ColorFormat,
    pub depth_format: Option<DepthFormat>,
    pub msaa_samples: u8,
    pub skinning_enabled: bool,
    pub alpha_mode: AlphaMode,
}

impl PipelineKey {
    pub const BASIC_TANK_SHADER: ShaderHandle = ShaderHandle(1);

    pub const fn tank_baseline(alpha_mode: AlphaMode) -> Self {
        Self {
            shader: Self::BASIC_TANK_SHADER,
            vertex_layout: VertexLayoutKey::StaticMesh,
            material_flags: MaterialPipelineFlags::ALBEDO_TEXTURED,
            color_format: ColorFormat::Bgra8UnormSrgb,
            depth_format: Some(DepthFormat::Depth32Float),
            msaa_samples: DEFAULT_MSAA_SAMPLES,
            skinning_enabled: false,
            alpha_mode,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineCacheMode {
    DevelopmentHotReload,
    ReleasePrewarm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineWarmupPlan {
    pub mode: PipelineCacheMode,
    keys: Vec<PipelineKey>,
}

impl PipelineWarmupPlan {
    pub fn baseline(mode: PipelineCacheMode) -> Self {
        Self::new(
            mode,
            [
                PipelineKey::tank_baseline(AlphaMode::Opaque),
                PipelineKey::tank_baseline(AlphaMode::Masked),
            ],
        )
    }

    pub fn new(mode: PipelineCacheMode, keys: impl IntoIterator<Item = PipelineKey>) -> Self {
        let mut unique = Vec::new();
        for key in keys {
            if !unique.contains(&key) {
                unique.push(key);
            }
        }
        Self { mode, keys: unique }
    }

    pub fn keys(&self) -> &[PipelineKey] {
        &self.keys
    }
}
