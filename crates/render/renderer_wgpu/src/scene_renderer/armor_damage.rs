//! Bounded GPU representation of analytical armor openings.

use renderer_api::ArmorDamageInstance;

use crate::GpuContext;

pub const MAX_DAMAGE_HEADERS: usize = 64;
/// Fourteen visible tanks, each with 12 physical groups, four pose-frame fragments per group and
/// four union lobes per fragment, plus headroom. The descriptor buffer is still below 192 KiB.
pub const MAX_DAMAGE_APERTURES: usize = 3_072;

pub const fn armor_damage_aperture_budget() -> usize {
    MAX_DAMAGE_APERTURES
}

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
    /// x glow intensity now (CPU-cooled), y glow tightness, z cut flag (1 opens the mesh,
    /// 0 scorches without opening), w reserved.
    pub thermal: [f32; 4],
}

/// Remembers the last-uploaded damage set so an unchanged frame skips rebuilding and re-uploading
/// the GPU buffers. Armor damage changes a few times a MINUTE (a penetration), not every frame — the
/// old code paid two Vec builds and two `write_buffer`s on all ~60 frames a second regardless (~240
/// KiB/s late in a battle with every hull holed). `None` means nothing has been uploaded yet, so the
/// first call always fires and initialises the freshly-created buffers even for an empty set.
#[derive(Default)]
struct DamageUploadGuard {
    last: Option<Vec<ArmorDamageInstance>>,
}

impl DamageUploadGuard {
    /// True (and remembers the new set) when `damage` differs from the last upload — or nothing has
    /// been uploaded yet; false when it is byte-for-byte identical, so the caller skips the GPU
    /// write. Exact by `PartialEq`, so it never skips a real change.
    fn changed(&mut self, damage: &[ArmorDamageInstance]) -> bool {
        if self.last.as_deref() == Some(damage) {
            return false;
        }
        let buffer = self.last.get_or_insert_with(Vec::new);
        buffer.clear();
        buffer.extend_from_slice(damage);
        true
    }
}

pub struct ArmorDamageBuffers {
    pub headers: wgpu::Buffer,
    pub apertures: wgpu::Buffer,
    guard: DamageUploadGuard,
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
        Self { headers, apertures, guard: DamageUploadGuard::default() }
    }

    pub fn upload(&mut self, ctx: &GpuContext, damage: &[ArmorDamageInstance]) {
        // Skip the whole rebuild + two GPU writes when the damage set has not changed since last
        // frame — the common case, since a hull is holed a handful of times a battle, not per frame.
        if !self.guard.changed(damage) {
            return;
        }
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
                    thermal: [
                        aperture.glow,
                        aperture.glow_tightness.max(0.25),
                        if aperture.cut { 1.0 } else { 0.0 },
                        aperture.kind.as_lane(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(id: u64) -> ArmorDamageInstance {
        ArmorDamageInstance { tank_id: game_core::TankId(id), apertures: Vec::new() }
    }

    #[test]
    fn the_damage_upload_guard_fires_first_and_on_change_but_skips_an_unchanged_set() {
        let mut guard = DamageUploadGuard::default();
        // The first call always fires — even an empty set — so the freshly-created buffers are
        // initialised rather than left with undefined contents.
        assert!(guard.changed(&[]), "the first upload initialises the buffers");
        assert!(!guard.changed(&[]), "an unchanged empty set is then skipped");

        let d1 = vec![instance(1)];
        assert!(guard.changed(&d1), "new damage fires");
        assert!(!guard.changed(&d1), "the same damage is skipped");

        let d2 = vec![instance(2)];
        assert!(guard.changed(&d2), "a different set fires again");
        assert!(!guard.changed(&d2), "and is then skipped");

        // Back to empty (every holed hull cleared) fires once more, then settles.
        assert!(guard.changed(&[]), "clearing the damage fires");
        assert!(!guard.changed(&[]));
    }
}
