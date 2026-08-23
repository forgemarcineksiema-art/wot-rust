use std::collections::HashMap;

use renderer_api::{MaterialHandle, VehicleMaterialFamilies, VehicleTextureMap};

use crate::GpuContext;

/// The number of material-role layers stacked into each map's texture array (one per `material_id`).
const LAYERS: u32 = VehicleMaterialFamilies::LAYERS as u32;

/// Owns the vehicle material bind groups. Each registered [`MaterialHandle`] gets a bind group whose
/// four maps are **texture arrays** — one layer per material role — so the vehicle shader selects the
/// role by `material_id` without splitting the mesh. Handles without uploaded maps resolve to a
/// neutral fallback so the pipeline always has valid bindings.
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
        let albedo = solid_array(device, queue, "vehicle_albedo_fallback", [255, 255, 255, 255]);
        let normal = solid_array(device, queue, "vehicle_normal_fallback", [128, 128, 255, 255]);
        let ao = solid_array(device, queue, "vehicle_ao_roughness_fallback", [255, 160, 0, 255]);
        let cavity = solid_array(device, queue, "vehicle_cavity_fallback", [255, 255, 255, 255]);
        let fallback =
            build_bind_group(device, &layout, &sampler, &albedo, &normal, &ao, &cavity, "fallback");
        Self { layout, sampler, fallback, materials: HashMap::new() }
    }

    /// Upload one vehicle's role families and build its bind group. Each map kind becomes a layered
    /// texture array; a family missing a cavity layer falls back to neutral white for that layer.
    pub(super) fn register(
        &mut self,
        ctx: &GpuContext,
        handle: MaterialHandle,
        families: &VehicleMaterialFamilies,
    ) {
        let device = &ctx.device;
        let queue = &ctx.queue;
        let layers = families.families();
        let albedo =
            layer_array(device, queue, "vehicle_albedo", &layer_maps(layers, |m| m.albedo()));
        let normal =
            layer_array(device, queue, "vehicle_normal", &layer_maps(layers, |m| m.normal()));
        let ao = layer_array(
            device,
            queue,
            "vehicle_ao_roughness",
            &layer_maps(layers, |m| m.ao_roughness()),
        );
        let cavity_layers: Vec<VehicleTextureMap> = layers
            .iter()
            .map(|m| m.cavity().cloned().unwrap_or_else(|| solid_map([255, 255, 255, 255])))
            .collect();
        let cavity = layer_array(device, queue, "vehicle_cavity", &cavity_layers);
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

fn layer_maps(
    layers: &[renderer_api::VehicleMaterialMaps],
    pick: impl Fn(&renderer_api::VehicleMaterialMaps) -> &VehicleTextureMap,
) -> Vec<VehicleTextureMap> {
    layers.iter().map(|m| pick(m).clone()).collect()
}

#[expect(clippy::too_many_arguments)]
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

/// Build a `LAYERS`-deep texture array from per-layer maps, uploading each layer. All layers must
/// share dimensions (the baked families are uniform); the first layer's size defines the array.
fn layer_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    maps: &[VehicleTextureMap],
) -> wgpu::TextureView {
    let (width, height) = (maps[0].width(), maps[0].height());
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: LAYERS },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, map) in maps.iter().enumerate() {
        write_layer(queue, &texture, layer as u32, map.width(), map.height(), map.rgba());
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// A `LAYERS`-deep array of a single solid colour — the neutral fallback for every role layer.
fn solid_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    rgba: [u8; 4],
) -> wgpu::TextureView {
    let maps: Vec<VehicleTextureMap> = (0..LAYERS).map(|_| solid_map(rgba)).collect();
    layer_array(device, queue, label, &maps)
}

fn solid_map(rgba: [u8; 4]) -> VehicleTextureMap {
    VehicleTextureMap::new(1, 1, rgba.to_vec())
}

fn write_layer(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    layer: u32,
    width: u32,
    height: u32,
    rgba: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}

fn texture_entry<'a>(binding: u32, view: &'a wgpu::TextureView) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) }
}
