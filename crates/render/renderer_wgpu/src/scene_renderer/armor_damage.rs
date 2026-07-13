//! Bounded GPU representation of analytical armor openings.

use renderer_api::ArmorDamageInstance;

use crate::GpuContext;

pub const MAX_DAMAGE_HEADERS: usize = 64;
pub const MAX_DAMAGE_APERTURES: usize = 256;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuDamageHeader {
    pub start: u32,
    pub count: u32,
    pub _padding: [u32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuAperture {
    pub center_major: [f32; 4],
    pub normal_minor: [f32; 4],
    pub tangent_rotation: [f32; 4],
    /// x irregularity, y/z deterministic wave phases, w plane half-depth.
    pub shape: [f32; 4],
}

pub struct ArmorDamageBuffers {
    pub headers: wgpu::Buffer,
    pub apertures: wgpu::Buffer,
}

impl ArmorDamageBuffers {
    pub fn new(device: &wgpu::Device) -> Self {
        let headers = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("armor_damage_headers"),
            size: (MAX_DAMAGE_HEADERS * std::mem::size_of::<GpuDamageHeader>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let apertures = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("armor_damage_apertures"),
            size: (MAX_DAMAGE_APERTURES * std::mem::size_of::<GpuAperture>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { headers, apertures }
    }

    pub fn upload(&self, ctx: &GpuContext, damage: &[ArmorDamageInstance]) {
        let mut headers = vec![GpuDamageHeader::default()];
        let mut apertures = Vec::new();
        for instance in damage.iter().take(MAX_DAMAGE_HEADERS - 1) {
            let start = apertures.len();
            let remaining = MAX_DAMAGE_APERTURES.saturating_sub(start);
            for aperture in instance.apertures.iter().take(remaining) {
                apertures.push(GpuAperture {
                    center_major: [
                        aperture.center[0],
                        aperture.center[1],
                        aperture.center[2],
                        aperture.major_radius_m,
                    ],
                    normal_minor: [
                        aperture.normal[0],
                        aperture.normal[1],
                        aperture.normal[2],
                        aperture.minor_radius_m,
                    ],
                    tangent_rotation: [
                        aperture.tangent[0],
                        aperture.tangent[1],
                        aperture.tangent[2],
                        aperture.rotation_rad,
                    ],
                    shape: [
                        aperture.irregularity,
                        aperture.phase_a,
                        aperture.phase_b,
                        aperture.half_depth_m,
                    ],
                });
            }
            headers.push(GpuDamageHeader {
                start: start as u32,
                count: (apertures.len() - start) as u32,
                _padding: [0; 2],
            });
        }
        if apertures.is_empty() {
            apertures.push(GpuAperture::default());
        }
        ctx.queue.write_buffer(&self.headers, 0, bytemuck::cast_slice(&headers));
        ctx.queue.write_buffer(&self.apertures, 0, bytemuck::cast_slice(&apertures));
    }
}

pub fn storage_layout_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [storage_layout_entry(1), storage_layout_entry(2)]
}

fn storage_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
