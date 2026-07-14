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
    /// Whether the water surface refracts the scene behind it (a mid-frame grab of the resolved
    /// opaque HDR + a second transparent pass). It costs an extra full-frame resolve and pass, so
    /// the weakest adapters keep the analytic water instead. Off on integrated/software.
    pub refraction: bool,
    /// Full per-pixel shader detail (Płynność 2.0 / F2). `false` folds the heaviest ALU work
    /// down on weak adapters: fewer sky FBM octaves, 2×2 near-shadow PCF instead of 3×3, and
    /// the terrain/scene noise drops its analytic normal-bend gradient. One flag, read by the
    /// shaders from a spare camera-uniform lane — the LOOK's composition stays identical.
    pub full_shader_detail: bool,
}

impl LightingQuality {
    /// One-look policy: every adapter class receives the same [`Self::canonical`] profile. The
    /// signature survives so existing callers compile; the class is deliberately ignored.
    pub fn for_device_type(_device_type: GpuDeviceType) -> Self {
        Self::canonical()
    }

    /// THE game's one look (one-look policy, 2026-07-14): a single canonical profile for every
    /// adapter — the game owns its performance instead of handing the player a settings menu.
    /// Calibrated to hold 60 FPS on the minimum spec (GeForce MX330 / Iris Xe class); a
    /// stronger GPU renders the IDENTICAL picture with headroom. Equal picture, equal game:
    /// nothing that affects visibility may depend on hardware or options.
    pub fn canonical() -> Self {
        Self {
            shadow_resolution: 2048,
            shadow_cascades: 2,
            ssao_scale: 0.5,
            cloud_shadows: false,
            bloom_mips: 0,
            refraction: false,
            full_shader_detail: false,
        }
    }

    /// The rich profile — DEV ONLY (`WOT_QUALITY=high`): devlog captures, look-lock comparison
    /// renders, portfolio shots. Never the shipped look.
    pub fn rich() -> Self {
        Self {
            shadow_resolution: 4096,
            shadow_cascades: 2,
            ssao_scale: 1.0,
            cloud_shadows: true,
            bloom_mips: 5,
            refraction: true,
            full_shader_detail: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one-look policy's lock: every adapter class gets the identical canonical profile
    /// (equal picture, equal game), and the rich profile stays a dev-only superset.
    #[test]
    fn every_adapter_gets_the_one_canonical_look() {
        let canonical = LightingQuality::canonical();
        for device in [
            GpuDeviceType::IntegratedGpu,
            GpuDeviceType::DiscreteGpu,
            GpuDeviceType::Cpu,
            GpuDeviceType::VirtualGpu,
            GpuDeviceType::Other,
        ] {
            assert_eq!(
                LightingQuality::for_device_type(device),
                canonical,
                "{device:?} must render the same picture as everyone else"
            );
        }
        // The canonical numbers are the minimum-spec 60 FPS calibration — locked.
        assert_eq!(canonical.shadow_resolution, 2048);
        assert_eq!(canonical.shadow_cascades, 2);
        assert_eq!(canonical.ssao_scale, 0.5);
        assert!(!canonical.cloud_shadows && canonical.bloom_mips == 0 && !canonical.refraction);
        assert!(!canonical.full_shader_detail);
        // Dev-only rich profile is a strict superset for captures, never the shipped look.
        let rich = LightingQuality::rich();
        assert!(rich.full_shader_detail && rich.cloud_shadows && rich.bloom_mips > 0);
    }
}
