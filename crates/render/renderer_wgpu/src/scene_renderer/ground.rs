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
    /// The detail tiles' sampler: REPEAT (a tile is a period), trilinear, anisotropic — the
    /// ground is seen at grazing angles almost everywhere, and anisotropy is what keeps the
    /// 0.3 m grain from smearing into streaks along the depth axis there.
    detail_sampler: wgpu::Sampler,
    /// The baked ground material (Teren 2.0), uploaded once per renderer on the first ground
    /// bind: the four-layer detail array and the macro tone tile. Map-independent.
    detail_tiles: Option<GroundDetailViews>,
    pub binding: Option<GroundBinding>,
}

struct GroundDetailViews {
    layers: wgpu::TextureView,
    macro_tile: wgpu::TextureView,
}

/// The ground bind group's layout AS DATA: which stage may read each of the seven bindings. A
/// binding the shader reads from a stage the layout did not grant is not a compile error and
/// not a panic — wgpu refuses the whole pipeline at creation, logs one line nobody reads, and
/// the ground simply stops drawing (Q7's first cold sandwich measured a 0.0 ms battle frame for
/// exactly that: the field quilt moved into the vertex stage, the materials stayed
/// fragment-only). `ground_layout_matches_the_shader` pins this table to `terrain.wgsl`.
pub(crate) fn ground_bind_group_layout_entries() -> [wgpu::BindGroupLayoutEntry; 7] {
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let texture_array = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2Array,
        multisampled: false,
    };
    [
        // The splat map: per-pixel layer weights.
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture,
            count: None,
        },
        // The macro normal map.
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture,
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        // The material set: the fragment stage reads the layers, the vertex stage reads the
        // field-quilt strength (Q7 — the quilt is evaluated once per vertex).
        wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        // Teren 2.0: the detail material — one array, four layers in splat channel order.
        wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture_array,
            count: None,
        },
        // The macro tone tile (T3's macro variation fetch).
        wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: texture,
            count: None,
        },
        // The tiles' repeat/anisotropic sampler (the splat sampler clamps — a different one).
        wgpu::BindGroupLayoutEntry {
            binding: 6,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
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
            entries: &ground_bind_group_layout_entries(),
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
        let detail_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain_detail_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: GROUND_DETAIL_ANISOTROPY,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            detail_sampler,
            detail_tiles: None,
            binding: None,
        }
    }
}

/// The detail tiles' anisotropy: 2, the measured landing (2026-09-04 sandwich on the MX330:
/// full-scene p50 B8 26.66 / B4 25.42 / B2 24.38 vs A master 25.66 ms). Eight is what the
/// bark arrays pay, and the ground is more grazing than a trunk — it still cost more than
/// it bought. Two keeps the 0.3 m grain from smearing into streaks without a fill bill.
pub(crate) const GROUND_DETAIL_ANISOTROPY: u16 = 2;

/// The baked material, once per process: the bake is ~50 ms of CPU (release) and every
/// renderer in a test binary would otherwise pay it again.
fn ground_detail_tiles() -> &'static renderer_api::GroundDetailTiles {
    static TILES: std::sync::OnceLock<renderer_api::GroundDetailTiles> = std::sync::OnceLock::new();
    TILES.get_or_init(renderer_api::bake_ground_detail_tiles)
}

/// Upload the detail array (D2Array, four layers, full box mip chains — the normal lane
/// averages toward flat and the shade lane toward 0.5 exactly as a filtered surface should)
/// and the macro tile. Linear formats: normals and shades are data, not colour.
fn upload_ground_detail(ctx: &GpuContext) -> GroundDetailViews {
    let tiles = ground_detail_tiles();
    let chains: Vec<renderer_api::Rgba8MipChain> = tiles
        .layers
        .iter()
        .map(|base| renderer_api::Rgba8MipChain::build(base.clone(), renderer_api::MipMode::Box))
        .collect();
    let levels = chains[0].levels().len() as u32;
    let size = tiles.layers[0].width();
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain_detail_tiles"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: chains.len() as u32,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, chain) in chains.iter().enumerate() {
        for (level, mip) in chain.levels().iter().enumerate() {
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                mip.rgba(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.width() * 4),
                    rows_per_image: Some(mip.height()),
                },
                wgpu::Extent3d {
                    width: mip.width(),
                    height: mip.height(),
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    let layers = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let macro_tile =
        upload_rgba8(ctx, "terrain_macro_tile", tiles.macro_tile.width(), tiles.macro_tile.rgba());
    GroundDetailViews { layers, macro_tile }
}

/// Upload a square map with its FULL box-filtered mip chain. The splat and macro maps shipped
/// for months at `mip_level_count: 1` while a terrain shader comment claimed the far field
/// "rides a mipmapped texture … gains no shimmer" — the far ground was aliasing raw texels,
/// speckling bright between grass and straw. The chain makes the claim true.
///
/// The walk itself lives in `renderer_api::Rgba8MipChain::build` (hoisted for the foliage
/// atlas, Drzewa 3.0 PR2); `MipMode::Box` is bit-identical to the downsampler that used to
/// live here — linear, so splat weight sums stay normalized and the packed macro normal
/// renormalizes in the shader anyway.
fn upload_rgba8(ctx: &GpuContext, label: &str, size: u32, bytes: &[u8]) -> wgpu::TextureView {
    let chain = renderer_api::Rgba8MipChain::build(
        renderer_api::Rgba8MipLevel::new(size, size, bytes.to_vec()),
        renderer_api::MipMode::Box,
    );
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: chain.levels().len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, mip) in chain.levels().iter().enumerate() {
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip.rgba(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip.width() * 4),
                rows_per_image: Some(mip.height()),
            },
            wgpu::Extent3d { width: mip.width(), height: mip.height(), depth_or_array_layers: 1 },
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
        if self.ground.detail_tiles.is_none() {
            self.ground.detail_tiles = Some(upload_ground_detail(ctx));
        }
        let detail = self.ground.detail_tiles.as_ref().expect("uploaded above");
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&detail.layers),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&detail.macro_tile),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.ground.detail_sampler),
                },
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
    use super::{
        GROUND_DETAIL_ANISOTROPY, ground_bind_group_layout_entries, terrain_shader_source,
    };
    use renderer_api::{MipMode, Rgba8MipChain, Rgba8MipLevel};

    /// Anisotropy 2 is the measured landing (T3 sandwich): 8 cost more than it bought.
    #[test]
    fn the_detail_sampler_anisotropy_is_the_measured_landing() {
        assert_eq!(GROUND_DETAIL_ANISOTROPY, 2);
    }

    /// The lock for D3's structural cousin: the ground maps upload a FULL mip chain, so the
    /// far field filters instead of aliasing bright between grass and straw texels. The walk
    /// moved to `renderer_api` with the Drzewa 3.0 hoist; what THIS crate still owes the
    /// terrain is that `upload_rgba8` rides the box mode with the legacy math — the fixture is
    /// the old in-house downsampler's, bit-exact.
    #[test]
    fn the_ground_maps_carry_a_full_box_filtered_mip_chain() {
        let chain = Rgba8MipChain::build(
            Rgba8MipLevel::new(1024, 1024, vec![9; 1024 * 1024 * 4]),
            MipMode::Box,
        );
        assert_eq!(chain.levels().len(), 11, "log2(1024) + 1 levels, down to the 1x1 tail");
        #[rustfmt::skip]
        let bytes = vec![
            10, 0, 100, 255,   20, 0, 100, 255,
            30, 0, 100, 255,  100, 0, 100, 255,
        ];
        let chain = Rgba8MipChain::build(Rgba8MipLevel::new(2, 2, bytes), MipMode::Box);
        assert_eq!(chain.levels()[1].rgba(), &[40, 0, 100, 255], "the 2x2 rounded box mean");
    }

    /// Every binding `terrain.wgsl` reads at group 1 is granted to THAT stage by the ground
    /// layout. wgpu would otherwise refuse the pipeline at creation and log one line — no
    /// panic, no ground, a 0.0 ms battle frame (Q7's first sandwich). The vertex stage does
    /// read the material set now: the field quilt is evaluated per vertex.
    #[test]
    fn ground_layout_matches_the_shader() {
        let entries = ground_bind_group_layout_entries();
        let uses =
            crate::shader_validation::wgsl_stage_binding_uses("terrain", &terrain_shader_source())
                .expect("terrain.wgsl validates");
        let stage_bit = |stage: naga::ShaderStage| match stage {
            naga::ShaderStage::Vertex => wgpu::ShaderStages::VERTEX,
            naga::ShaderStage::Fragment => wgpu::ShaderStages::FRAGMENT,
            _ => wgpu::ShaderStages::COMPUTE,
        };
        let ground_uses: Vec<_> = uses.iter().filter(|item| item.group == 1).collect();
        assert!(!ground_uses.is_empty(), "the ground shader reads its group-1 bindings");
        for item in &ground_uses {
            let entry =
                entries.iter().find(|entry| entry.binding == item.binding).unwrap_or_else(|| {
                    panic!(
                        "{} reads group 1 binding {} ({}) that the layout does not declare",
                        item.entry_point, item.binding, item.name
                    )
                });
            assert!(
                entry.visibility.contains(stage_bit(item.stage)),
                "{} ({:?}) reads `{}` (binding {}) but the layout grants it only {:?}: wgpu \
                 would refuse the pipeline silently",
                item.entry_point,
                item.stage,
                item.name,
                item.binding,
                entry.visibility
            );
        }
        assert!(
            ground_uses
                .iter()
                .any(|item| item.stage == naga::ShaderStage::Vertex && item.binding == 3),
            "the field quilt reads the material set in the vertex stage (Q7)"
        );
    }
}
