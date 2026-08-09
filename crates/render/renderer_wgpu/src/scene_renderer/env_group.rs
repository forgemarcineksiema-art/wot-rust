//! The group-2 "environment" bind group both main pipelines consume: the comparison-sampled sun
//! shadow cascades (bindings 0-1 near map + sampler, binding 4 far map), the blurred SSAO
//! target (bindings 2-3), and the baked cloud-coverage tile (bindings 5-6, repeat-sampled —
//! see `cloud_map.rs`). The two cascade maps are separate textures, not one array — they run at
//! different resolutions (the far box needs half the texels), and array layers must share a size.
//! Split from `shadow.rs` to keep each module within the reviewability budget.

/// The screen-environment bind group layout both main pipelines bind at group 2: the
/// comparison-sampled sun shadow map (bindings 0–1), the blurred SSAO target (bindings 2–3),
/// the far shadow cascade (binding 4, sharing the comparison sampler at binding 1), and the
/// cloud-coverage tile with its repeat sampler (bindings 5–6).
pub fn build_shadow_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let depth_texture = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let float_texture = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let filtering_sampler = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow_bgl"),
        entries: &[
            depth_texture(0),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            float_texture(2),
            filtering_sampler(3),
            depth_texture(4),
            float_texture(5),
            filtering_sampler(6),
            // The interior reflection cubemap (Hala 3.0 D1); a 1x1 black cube outside the
            // garage. Shares the filtering sampler at binding 3.
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

/// The full group-2 entries in binding order.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_environment_bind_group(
    device: &wgpu::Device,
    shadow_bgl: &wgpu::BindGroupLayout,
    depth_view: &wgpu::TextureView,
    far_depth_view: &wgpu::TextureView,
    shadow_sampler: &wgpu::Sampler,
    ao_view: &wgpu::TextureView,
    ao_sampler: &wgpu::Sampler,
    cloud_view: &wgpu::TextureView,
    cloud_sampler: &wgpu::Sampler,
    env_cube_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_bg"),
        layout: shadow_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(ao_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(ao_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(far_depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(cloud_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(cloud_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(env_cube_view),
            },
        ],
    })
}
