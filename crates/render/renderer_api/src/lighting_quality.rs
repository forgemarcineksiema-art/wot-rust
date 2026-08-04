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
    /// Whether the ground modulates the sun by the wandering cloud layer — one repeat-sampled
    /// tap of the baked coverage tile per fragment (`renderer_wgpu`'s `cloud_map.rs`).
    pub cloud_shadows: bool,
    /// Bloom mip-chain depth in the central post pass: 0 disables the chain entirely, 3 is the
    /// integrated-GPU budget (half/quarter/eighth res), 5 the full dual-Kawase ladder.
    pub bloom_mips: u32,
    /// Whether the water surface refracts the scene behind it (a mid-frame grab of the resolved
    /// opaque HDR + a second transparent pass). It costs an extra full-frame resolve and pass, so
    /// the weakest adapters keep the analytic water instead. Off on integrated/software.
    pub refraction: bool,
    /// Per-feature shader detail (Żywy Step P0). The old monolithic `full_shader_detail`
    /// flag split into individually purchasable bits, so the buy-back program can measure
    /// and enable each feature on the min-spec separately. Rides the same spare camera lane
    /// (`time_params.w`) as a small integer; `NONE` and `FULL` reproduce the old off/on.
    pub shader_detail: ShaderDetailMask,
}

/// Which per-pixel detail features run (see `LightingQuality::shader_detail`). Each bit is one
/// feature the shaders test independently; the composition of the LOOK never depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderDetailMask(pub u32);

impl ShaderDetailMask {
    /// Terrain detail-noise normal bend (grain catches light).
    pub const TERRAIN_NORMAL_BEND: u32 = 1 << 0;
    /// Terrain third "micro" octave near the eye.
    pub const TERRAIN_MICRO_OCTAVE: u32 = 1 << 1;
    /// Statics' three-sample detail normal.
    pub const SCENE_DETAIL_NORMAL: u32 = 1 << 2;
    /// Sky cloud FBM at five octaves instead of three.
    pub const SKY_FIVE_OCTAVES: u32 = 1 << 3;
    /// 3x3 near-cascade PCF instead of the 2x2 core.
    pub const PCF_WIDE: u32 = 1 << 4;
    /// The crepuscular sun march (god rays).
    pub const GOD_RAYS: u32 = 1 << 5;
    /// Ploughed-field furrows: per-plot anisotropic stripes on the terrain (teren B1).
    pub const TERRAIN_FURROWS: u32 = 1 << 6;

    pub const NONE: Self = Self(0);
    pub const FULL: Self = Self(0b111_1111);

    pub fn has(self, bit: u32) -> bool {
        self.0 & bit != 0
    }
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
            // Bought back for the ship (D21): the coverage field is baked into a tiling
            // texture at renderer init (`cloud_map.rs`), so the per-fragment cost fell from
            // ~6 lattice evaluations (the measured ~5 ms on open maps that kept the feature
            // out) to one repeat-sampled texture tap.
            cloud_shadows: true,
            bloom_mips: 0,
            refraction: false,
            // Żywy Step P1: the ground's micro-relief is bought back — pure per-pixel ALU
            // (value noise, no texture taps), measured on the min-spec before shipping. The
            // remaining bits stay in the buy-back pool.
            // Teren B1: the ploughed-field furrows bought back through the same protocol —
            // detail_cost_probe, cold GPU, both orders: mask 3 vs 67 read 11.008 vs
            // 11.002 ms and 9.947 vs 9.998 ms — a ±0.05 ms delta inside the noise. (The
            // first measurement session showed +3 ms and was WRONG: a laptop thermal ramp
            // climbing monotonically across runs regardless of mask. Interleave and cool.)
            shader_detail: ShaderDetailMask(
                ShaderDetailMask::TERRAIN_NORMAL_BEND
                    | ShaderDetailMask::TERRAIN_MICRO_OCTAVE
                    | ShaderDetailMask::TERRAIN_FURROWS,
            ),
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
            shader_detail: ShaderDetailMask::FULL,
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
        // Cloud shade ships: the baked-tile rework (D21) collapsed its cost to one texture
        // tap, so the wandering shade is part of the one look on every adapter.
        assert!(canonical.cloud_shadows && canonical.bloom_mips == 0 && !canonical.refraction);
        assert_eq!(
            canonical.shader_detail,
            ShaderDetailMask(
                ShaderDetailMask::TERRAIN_NORMAL_BEND
                    | ShaderDetailMask::TERRAIN_MICRO_OCTAVE
                    | ShaderDetailMask::TERRAIN_FURROWS
            )
        );
        // Dev-only rich profile is a strict superset for captures, never the shipped look.
        let rich = LightingQuality::rich();
        assert_eq!(rich.shader_detail, ShaderDetailMask::FULL);
        assert!(rich.cloud_shadows && rich.bloom_mips > 0);
    }
}

#[cfg(test)]
mod mask_tests {
    use super::*;

    /// The mask rides a float camera lane: every legal mask must survive the f32 round-trip
    /// the shaders decode with `u32(w + 0.5)`.
    #[test]
    fn the_detail_mask_survives_the_float_lane() {
        for mask in 0..=ShaderDetailMask::FULL.0 {
            let lane = mask as f32;
            assert_eq!((lane + 0.5) as u32, mask, "mask {mask} corrupted by the lane");
        }
    }

    #[test]
    fn full_covers_every_defined_bit_and_has_reads_them() {
        let full = ShaderDetailMask::FULL;
        for bit in [
            ShaderDetailMask::TERRAIN_NORMAL_BEND,
            ShaderDetailMask::TERRAIN_MICRO_OCTAVE,
            ShaderDetailMask::SCENE_DETAIL_NORMAL,
            ShaderDetailMask::SKY_FIVE_OCTAVES,
            ShaderDetailMask::PCF_WIDE,
            ShaderDetailMask::GOD_RAYS,
            ShaderDetailMask::TERRAIN_FURROWS,
        ] {
            assert!(full.has(bit));
            assert!(!ShaderDetailMask::NONE.has(bit));
        }
    }
}
