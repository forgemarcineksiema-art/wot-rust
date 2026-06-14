use std::collections::HashMap;

use renderer_api::{MaterialHandle, VehicleMaterialMaps, VehicleTextureMap};

use crate::GpuContext;

/// Owns the vehicle material bind groups. Each registered [`MaterialHandle`] gets its own bind
/// group sampling the uploaded baked maps; handles without uploaded maps resolve to a neutral
/// fallback bind group so the vehicle pipeline always has valid bindings (the "falls back cleanly
/// if a debug texture is missing" contract from the Forge plan).
pub(super) struct VehicleMaterialRegistry {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    fallback: wgpu::BindGroup,
    materials: HashMap<u32, wgpu::BindGroup>,
}

impl VehicleMaterialRegistry {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: wgpu::BindGroupLayout,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vehicle_material_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let albedo = solid_view(device, queue, "vehicle_albedo_fallback", [255, 255, 255, 255]);
        let normal = solid_view(device, queue, "vehicle_normal_fallback", [128, 128, 255, 255]);
        let ao = solid_view(device, queue, "vehicle_ao_roughness_fallback", [255, 160, 0, 255]);
        let cavity = solid_view(device, queue, "vehicle_cavity_fallback", [255, 255, 255, 255]);
        let fallback =
            build_bind_group(device, &layout, &sampler, &albedo, &normal, &ao, &cavity, "fallback");
        Self { layout, sampler, fallback, materials: HashMap::new() }
    }

    /// Uploads one vehicle's baked maps and builds its bind group. A missing cavity map reuses a
    /// neutral white texture, matching the shader's "no occlusion" default.
    pub(super) fn register(
        &mut self,
        ctx: &GpuContext,
        handle: MaterialHandle,
        maps: &VehicleMaterialMaps,
    ) {
        let device = &ctx.device;
        let queue = &ctx.queue;
        let albedo = map_view(device, queue, "vehicle_albedo", maps.albedo());
        let normal = map_view(device, queue, "vehicle_normal", maps.normal());
        let ao = map_view(device, queue, "vehicle_ao_roughness", maps.ao_roughness());
        let cavity = match maps.cavity() {
            Some(map) => map_view(device, queue, "vehicle_cavity", map),
            None => solid_view(device, queue, "vehicle_cavity_default", [255, 255, 255, 255]),
        };
        let bind_group = build_bind_group(
            device,
            &self.layout,
            &self.sampler,
            &albedo,
            &normal,
            &ao,
            &cavity,
            "uploaded",
        );
        self.materials.insert(handle.0, bind_group);
    }

    pub(super) fn bind_group(&self, handle: MaterialHandle) -> &wgpu::BindGroup {
        self.materials.get(&handle.0).unwrap_or(&self.fallback)
    }
}

#[allow(clippy::too_many_arguments)]
fn build_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    albedo: &wgpu::TextureView,
    normal: &wgpu::TextureView,
    ao: &wgpu::TextureView,
    cavity: &wgpu::TextureView,
    suffix: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("vehicle_material_bg_{suffix}")),
        layout,
        entries: &[
            texture_entry(0, albedo),
            texture_entry(1, normal),
            texture_entry(2, ao),
            texture_entry(3, cavity),
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn map_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    map: &VehicleTextureMap,
) -> wgpu::TextureView {
    upload_view(device, queue, label, map.width(), map.height(), map.rgba())
}

fn solid_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    rgba: [u8; 4],
) -> wgpu::TextureView {
    upload_view(device, queue, label, 1, 1, &rgba)
}

fn upload_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::TextureView {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn texture_entry<'a>(binding: u32, view: &'a wgpu::TextureView) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) }
}
