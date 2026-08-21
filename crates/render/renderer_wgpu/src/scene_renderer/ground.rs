//! The Terrain Material 2.0 ground slot: the battlefield heightfield drawn through its own
//! pipeline (`terrain.wgsl`) with the baked splat + macro-normal maps and the map's material
//! set at group 1. Separate from the generic terrain slot (cover, skirt, scenery keep the
//! scene pipeline) so the ground's per-pixel layers never bleed onto a barn wall. Chunked and
//! frustum-culled per pass exactly like the statics; the depth passes draw it through the
//! existing depth pipelines (same `SceneVertex` stride).

use renderer_api::{
    Frustum, SceneChunk, SceneVertex, TERRAIN_LAYERS, TerrainGroundMaps, TerrainMaterialSet,
    chunk_scene_indices,
};
use wgpu::util::DeviceExt;

use super::SceneRenderer;
use super::terrain::TERRAIN_CHUNK_SIZE_M;
use crate::GpuContext;
use crate::shader_library::{
    CAMERA_COMMON_WGSL, LIGHTING_COMMON_WGSL, MEADOW_COMMON_WGSL, NOISE_COMMON_WGSL,
    SHADOW_COMMON_WGSL, compose_shader,
};

pub fn terrain_shader_source() -> String {
    compose_shader(&[
        CAMERA_COMMON_WGSL,
        LIGHTING_COMMON_WGSL,
        SHADOW_COMMON_WGSL,
        NOISE_COMMON_WGSL,
        MEADOW_COMMON_WGSL,
        include_str!("../shaders/terrain.wgsl"),
    ])
}

/// The 6-vec4 uniform `terrain.wgsl` reads: 4x (albedo rgb + detail amp), per-layer gloss,
/// params (extent.xy, macro strength, field-patch strength).
fn encode_materials(set: &TerrainMaterialSet, extent_m: [f32; 2]) -> [[f32; 4]; 6] {
    let mut packed = [[0.0f32; 4]; 6];
    for (i, layer) in set.layers.iter().enumerate() {
        packed[i] = [layer.albedo[0], layer.albedo[1], layer.albedo[2], layer.detail];
    }
    packed[TERRAIN_LAYERS] =
        [set.layers[0].gloss, set.layers[1].gloss, set.layers[2].gloss, set.layers[3].gloss];
    packed[TERRAIN_LAYERS + 1] =
        [extent_m[0], extent_m[1], set.macro_normal_strength, set.field_patch_strength];
    packed
}

/// The bound battlefield ground: geometry, chunk table and the group-1 material resources.
pub(crate) struct GroundBinding {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub chunks: Vec<SceneChunk>,
    pub bind_group: wgpu::BindGroup,
}

pub(crate) struct GroundResources {
    pub pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pub binding: Option<GroundBinding>,
}

impl GroundResources {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
        shadow_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain_ground_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain_shader"),
            source: wgpu::ShaderSource::Wgsl(terrain_shader_source().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(&bind_group_layout), Some(shadow_bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<SceneVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &crate::scene_pipeline::VERTEX_ATTRIBUTES,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<crate::scene_resources::SceneInstance>()
                            as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &crate::scene_pipeline::INSTANCE_ATTRIBUTES,
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::offscreen::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: sample_count, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain_ground_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self { pipeline, bind_group_layout, sampler, binding: None }
    }
}

/// Full mip-chain depth for a square texture: log2(size) + 1. The splat and macro maps shipped
/// for months at `mip_level_count: 1` while a terrain shader comment claimed the far field
/// "rides a mipmapped texture … gains no shimmer" — the far ground was aliasing raw texels,
/// speckling bright between grass and straw. The chain makes the claim true.
fn mip_level_count(size: u32) -> u32 {
    32 - size.max(1).leading_zeros()
}

/// One box-filter mip step: average each 2x2 block (odd edges clamp). Plain and CPU-side on
/// purpose — the maps upload once per battlefield, and a box average preserves what the two
/// consumers need: splat weight sums stay normalized, and the packed macro normal renormalizes
/// in the shader anyway.
fn downsample_rgba8(bytes: &[u8], size: u32) -> Vec<u8> {
    let next = (size / 2).max(1);
    let mut out = Vec::with_capacity((next * next * 4) as usize);
    let texel = |x: u32, y: u32, c: u32| -> u32 {
        let x = x.min(size - 1);
        let y = y.min(size - 1);
        bytes[((y * size + x) * 4 + c) as usize] as u32
    };
    for y in 0..next {
        for x in 0..next {
            for c in 0..4 {
                let sum = texel(x * 2, y * 2, c)
                    + texel(x * 2 + 1, y * 2, c)
                    + texel(x * 2, y * 2 + 1, c)
                    + texel(x * 2 + 1, y * 2 + 1, c);
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    out
}

fn upload_rgba8(ctx: &GpuContext, label: &str, size: u32, bytes: &[u8]) -> wgpu::TextureView {
    let mip_count = mip_level_count(size);
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut level_bytes = bytes.to_vec();
    let mut level_size = size;
    for level in 0..mip_count {
        if level > 0 {
            level_bytes = downsample_rgba8(&level_bytes, level_size);
            level_size = (level_size / 2).max(1);
        }
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(level_size * 4),
                rows_per_image: Some(level_size),
            },
            wgpu::Extent3d { width: level_size, height: level_size, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

impl SceneRenderer {
    /// Bind the battlefield ground: the heightfield mesh (chunked like every static slot) plus
    /// its baked splat/macro-normal maps and the map's material set. Cleared automatically by
    /// [`Self::set_terrain`] on a scene swap (the garage has no splat ground).
    pub fn set_battlefield_ground(
        &mut self,
        ctx: &GpuContext,
        vertices: &[SceneVertex],
        indices: &[u32],
        maps: &TerrainGroundMaps,
        materials: &TerrainMaterialSet,
    ) {
        debug_assert!(maps.is_well_formed(), "ground maps must be well-formed");
        let (reordered, chunks) = chunk_scene_indices(vertices, indices, TERRAIN_CHUNK_SIZE_M);
        let vertex_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_ground_v"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_ground_i"),
            contents: bytemuck::cast_slice(&reordered),
            usage: wgpu::BufferUsages::INDEX,
        });
        let splat_view = upload_rgba8(ctx, "terrain_splat", maps.size, &maps.splat);
        let macro_view = upload_rgba8(ctx, "terrain_macro_normal", maps.size, &maps.macro_normal);
        let material_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_materials"),
            contents: bytemuck::cast_slice(&encode_materials(materials, maps.extent_m)),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain_ground_bg"),
            layout: &self.ground.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&splat_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&macro_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.ground.sampler),
                },
                wgpu::BindGroupEntry { binding: 3, resource: material_buffer.as_entire_binding() },
            ],
        });
        self.ground.binding = Some(GroundBinding {
            vertices: vertex_buffer,
            indices: index_buffer,
            chunks,
            bind_group,
        });
    }

    /// Replace only the ground GEOMETRY (true deformation, protocol v31): a fresh crater
    /// re-meshes the heightfield while the baked splat/macro maps and material set stay bound —
    /// the ground's dress never depends on the ledger, so a shell hole costs two buffers,
    /// never a 1024^2 rebake. No-op with no ground bound (garage, interiors).
    pub fn update_battlefield_ground_geometry(
        &mut self,
        ctx: &GpuContext,
        vertices: &[SceneVertex],
        indices: &[u32],
    ) {
        let Some(binding) = self.ground.binding.as_mut() else {
            return;
        };
        let (reordered, chunks) = chunk_scene_indices(vertices, indices, TERRAIN_CHUNK_SIZE_M);
        binding.vertices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_ground_v"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        binding.indices = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_ground_i"),
            contents: bytemuck::cast_slice(&reordered),
            usage: wgpu::BufferUsages::INDEX,
        });
        binding.chunks = chunks;
    }

    /// Drop the ground binding (scene swap to an interior).
    pub fn clear_battlefield_ground(&mut self) {
        self.ground.binding = None;
    }

    /// Diagnostic: how many ground chunks a camera at `view_proj` would draw.
    pub fn visible_ground_chunks(&self, view_proj: &[[f32; 4]; 4]) -> usize {
        let frustum = Frustum::from_view_proj(view_proj);
        self.ground
            .binding
            .as_ref()
            .map(|b| b.chunks.iter().filter(|c| frustum.intersects_aabb(&c.aabb)).count())
            .unwrap_or(0)
    }

    /// Issue the ground draws for one depth/colour pass: bind the ground buffers and draw the
    /// chunks `frustum` sees. The caller has already set the pass pipeline and bind groups.
    pub(super) fn draw_visible_ground(
        &self,
        pass: &mut crate::pass_recorder::CountedPass<'_, '_>,
        frustum: &Frustum,
    ) {
        let Some(binding) = self.ground.binding.as_ref() else {
            return;
        };
        pass.set_vertex_buffer(0, binding.vertices.slice(..));
        pass.set_index_buffer(binding.indices.slice(..), wgpu::IndexFormat::Uint32);
        for chunk in &binding.chunks {
            if frustum.intersects_aabb(&chunk.aabb) {
                pass.draw_indexed(
                    chunk.index_start..chunk.index_start + chunk.index_count,
                    0,
                    0..1,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{downsample_rgba8, mip_level_count};

    /// The lock for D3's structural cousin: the ground maps upload a FULL mip chain, so the
    /// far field filters instead of aliasing bright between grass and straw texels. If the
    /// chain math regresses to a single level, this fails before any golden does.
    #[test]
    fn the_ground_maps_carry_a_full_mip_chain() {
        assert_eq!(mip_level_count(1024), 11);
        assert_eq!(mip_level_count(512), 10);
        assert_eq!(mip_level_count(2), 2);
        assert_eq!(mip_level_count(1), 1);
        // The chain walks down to exactly 1x1: sizes halve (floor), never stall above 1.
        let mut size = 1024u32;
        for _ in 1..mip_level_count(1024) {
            size = (size / 2).max(1);
        }
        assert_eq!(size, 1);
    }

    #[test]
    fn a_mip_step_is_a_2x2_box_average() {
        // One 2x2 texture, four distinct texels per channel: the single output texel is their
        // rounded mean.
        #[rustfmt::skip]
        let bytes = vec![
            10, 0, 100, 255,   20, 0, 100, 255,
            30, 0, 100, 255,  100, 0, 100, 255,
        ];
        let out = downsample_rgba8(&bytes, 2);
        assert_eq!(out, vec![40, 0, 100, 255]);
    }

    #[test]
    fn a_mip_step_keeps_splat_weights_normalized() {
        // Splat texels are weights summing to 255 across RGBA. A box average is linear, so the
        // sum survives every level within rounding — the shader's renormalization never has to
        // fight the chain.
        let texels: [[u8; 4]; 4] =
            [[255, 0, 0, 0], [0, 255, 0, 0], [128, 127, 0, 0], [0, 64, 64, 127]];
        let bytes: Vec<u8> = texels.iter().flatten().copied().collect();
        let out = downsample_rgba8(&bytes, 2);
        let sum: u32 = out.iter().map(|&c| c as u32).sum();
        assert!((253..=257).contains(&sum), "weight sum drifted: {sum}");
    }
}
