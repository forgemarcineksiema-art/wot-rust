//! The one lighting-quality table: how the shadow cascades, SSAO and cloud shadows scale per
//! adapter class. Backend-neutral — `renderer_wgpu` maps its adapter type onto this and applies
//! the `WOT_*` env overrides in one place, replacing per-feature resolver functions scattered
//! through the passes. A settings menu later drives the same struct.

use crate::GpuDeviceType;

/// Per-tier lighting knobs. The struct is plain data so a capability probe, an env override or a
/// future settings menu can all produce one and hand it to the renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightingQuality {
    /// Near shadow cascade resolution in texels (the far cascade derives half of it — see
    /// `SunShadowParams::far_cascade`).
    pub shadow_resolution: u32,
    /// Sun-shadow cascades: 2 = near + far (the default everywhere), 1 = the single near box.
    pub shadow_cascades: u32,
    /// SSAO render scale relative to the frame: 1.0 = full resolution, 0.5 = half (quarter the
    /// pixels — including the depth prepass rasterization, the real cost on shared-memory GPUs).
    pub ssao_scale: f32,
    /// Whether the terrain modulates the sun by the procedural cloud layer (a ~10 ALU/px cost the
    /// weakest adapters skip).
    pub cloud_shadows: bool,
    /// Bloom mip-chain depth in the central post pass: 0 disables the chain entirely, 3 is the
    /// integrated-GPU budget (half/quarter/eighth res), 5 the full dual-Kawase ladder.
    pub bloom_mips: u32,
}

impl LightingQuality {
    /// The calibrated tier table. Integrated/software adapters halve the shadow map and run SSAO
    /// at half resolution — a 20-30 FPS laptop never resolves the detail either buys — while
    /// discrete GPUs keep everything at full quality. Both classes keep both shadow cascades: the
    /// far pass is terrain-only at half resolution and nearly free.
    pub fn for_device_type(device_type: GpuDeviceType) -> Self {
        match device_type {
            GpuDeviceType::IntegratedGpu => Self {
                shadow_resolution: 2048,
                shadow_cascades: 2,
                ssao_scale: 0.5,
                cloud_shadows: true,
                bloom_mips: 3,
            },
            GpuDeviceType::Cpu => Self {
                shadow_resolution: 2048,
                shadow_cascades: 2,
                ssao_scale: 0.5,
                cloud_shadows: false,
                bloom_mips: 0,
            },
            GpuDeviceType::DiscreteGpu | GpuDeviceType::VirtualGpu | GpuDeviceType::Other => Self {
                shadow_resolution: 4096,
                shadow_cascades: 2,
                ssao_scale: 1.0,
                cloud_shadows: true,
                bloom_mips: 5,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tier_table_is_locked() {
        let integrated = LightingQuality::for_device_type(GpuDeviceType::IntegratedGpu);
        assert_eq!(integrated.shadow_resolution, 2048);
        assert_eq!(integrated.shadow_cascades, 2);
        assert_eq!(integrated.ssao_scale, 0.5);
        assert!(integrated.cloud_shadows);

        let discrete = LightingQuality::for_device_type(GpuDeviceType::DiscreteGpu);
        assert_eq!(discrete.shadow_resolution, 4096);
        assert_eq!(discrete.shadow_cascades, 2);
        assert_eq!(discrete.ssao_scale, 1.0);
        assert!(discrete.cloud_shadows);

        // Software rasterizer: the one class that also drops the cloud-shadow ALU.
        let cpu = LightingQuality::for_device_type(GpuDeviceType::Cpu);
        assert_eq!(cpu.ssao_scale, 0.5);
        assert!(!cpu.cloud_shadows);
    }
}
