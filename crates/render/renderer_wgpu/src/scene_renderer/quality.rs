//! The one place adapter class + `WOT_*` env overrides become the frame's [`LightingQuality`]:
//! shadow cascade resolution/count and the SSAO render scale, replacing per-feature resolver
//! functions scattered through the passes. The tier table itself is backend-neutral
//! (`renderer_api::LightingQuality`); this module only maps the wgpu adapter type onto it and
//! applies the overrides.
//!
//! Overrides (each wins in both directions; garbage is ignored):
//! - `WOT_SHADOW_RES=512|1024|2048|4096|8192` — near-cascade resolution (the far cascade derives
//!   half of it). A 4096² Depth32Float map is 64 MB cleared+stored every frame and drives 9
//!   comparison taps per pixel in both main shaders — one of the fattest slices of a
//!   shared-memory GPU's frame, which is why integrated adapters default to 2048.
//! - `WOT_SHADOW_CASCADES=1|2` — 1 drops back to the single near box.
//! - `WOT_SSAO=off|half|full` — SSAO render scale (off = strength 0, the capability fallback).

use renderer_api::LightingQuality;

/// The EFFECTIVE quality class for an adapter (Płynność 2.0 / F3): entry-class discrete
/// laptop chips (GeForce MX, GT 7xx/1030, 9xxM) report `DiscreteGpu` to wgpu but perform in
/// the integrated class — keying quality on the raw type handed them the full RTX diet
/// (4096 shadows, 4×MSAA, full-res SSAO, bloom, refraction). `WOT_QUALITY=low|high` overrides
/// the classification outright; garbage falls back to the heuristic.
pub(crate) fn effective_device_class(
    device_type: wgpu::DeviceType,
    adapter_name: &str,
    quality_env: Option<&str>,
) -> wgpu::DeviceType {
    match quality_env.map(str::trim) {
        Some("low") => return wgpu::DeviceType::IntegratedGpu,
        Some("high") => return wgpu::DeviceType::DiscreteGpu,
        _ => {}
    }
    if device_type == wgpu::DeviceType::DiscreteGpu && is_entry_class_discrete(adapter_name) {
        return wgpu::DeviceType::IntegratedGpu;
    }
    device_type
}

/// Name-based entry-discrete detection. Deliberately NARROW: only chip families that are
/// unambiguously integrated-class performers — a miss costs nothing (the user keeps full
/// quality and the WOT_QUALITY knob), while a false positive would silently degrade a real
/// GPU.
fn is_entry_class_discrete(adapter_name: &str) -> bool {
    let name = adapter_name.to_ascii_lowercase();
    // GeForce MX110..MX570: the whole MX line is GT-1030-class silicon.
    let mx_series = name
        .split(" mx")
        .nth(1)
        .is_some_and(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()));
    if mx_series {
        return true;
    }
    // The entry GT line and the old 9xxM mobile chips.
    ["gt 1030", "gt 730", "gt 710", "gt 740", "920m", "930m", "940m"]
        .iter()
        .any(|token| name.contains(token))
}

pub(crate) fn resolve_lighting_quality_with_bloom(
    device_type: wgpu::DeviceType,
    shadow_res_env: Option<&str>,
    cascades_env: Option<&str>,
    ssao_env: Option<&str>,
    bloom_env: Option<&str>,
) -> LightingQuality {
    let mut quality = LightingQuality::for_device_type(crate::map_device_type(device_type));
    if let Some(value) = shadow_res_env.and_then(|value| value.trim().parse::<u32>().ok())
        && matches!(value, 512 | 1024 | 2048 | 4096 | 8192)
    {
        quality.shadow_resolution = value;
    }
    if let Some(value) = cascades_env.and_then(|value| value.trim().parse::<u32>().ok())
        && (1..=2).contains(&value)
    {
        quality.shadow_cascades = value;
    }
    match ssao_env.map(str::trim) {
        Some("off") => quality.ssao_scale = 0.0,
        Some("half") => quality.ssao_scale = 0.5,
        Some("full") => quality.ssao_scale = 1.0,
        _ => {}
    }
    match bloom_env.map(str::trim) {
        Some("off") => quality.bloom_mips = 0,
        Some("low") => quality.bloom_mips = 3,
        Some("full") => quality.bloom_mips = 5,
        _ => {}
    }
    quality
}

/// Apply the `WOT_REFRACTION=on|off` (or `1|0`) override to a resolved quality — water refraction
/// forced on (e.g. to preview it on an integrated adapter) or off (to profile without it). Kept
/// separate from the main resolver so it composes over any tier without disturbing its callers.
/// Garbage is ignored, leaving the tier default.
pub(crate) fn apply_refraction_override(
    mut quality: LightingQuality,
    refraction_env: Option<&str>,
) -> LightingQuality {
    match refraction_env.map(str::trim) {
        Some("on" | "1") => quality.refraction = true,
        Some("off" | "0") => quality.refraction = false,
        _ => {}
    }
    quality
}

/// Apply the `WOT_GPU_DETAIL=full|low` override — force the F2 shader-detail tier either way
/// (preview the fold on a discrete adapter, or claw detail back on a strong iGPU). Garbage is
/// ignored, leaving the tier default.
pub(crate) fn apply_shader_detail_override(
    mut quality: LightingQuality,
    detail_env: Option<&str>,
) -> LightingQuality {
    match detail_env.map(str::trim) {
        Some("full" | "1") => quality.full_shader_detail = true,
        Some("low" | "0") => quality.full_shader_detail = false,
        _ => {}
    }
    quality
}

#[cfg(test)]
mod tests {
    /// F3's contract: an entry-class discrete chip (the GeForce MX line and friends) folds to
    /// the integrated diet, a real discrete GPU keeps full quality, and WOT_QUALITY overrides
    /// the classification in both directions.
    #[test]
    fn entry_class_discrete_folds_to_the_integrated_diet() {
        use wgpu::DeviceType;
        let class = |name: &str| super::effective_device_class(DeviceType::DiscreteGpu, name, None);
        for entry in [
            "NVIDIA GeForce MX150",
            "NVIDIA GeForce MX450",
            "GeForce MX330",
            "NVIDIA GeForce GT 1030",
            "NVIDIA GeForce 940MX",
        ] {
            assert_eq!(class(entry), DeviceType::IntegratedGpu, "{entry} performs integrated");
        }
        for real in [
            "NVIDIA GeForce RTX 3060 Laptop GPU",
            "NVIDIA GeForce GTX 1660 Ti",
            "AMD Radeon RX 6700 XT",
            "Intel Arc A770",
        ] {
            assert_eq!(class(real), DeviceType::DiscreteGpu, "{real} keeps full quality");
        }
        // The user's word beats the heuristic, both ways.
        assert_eq!(
            super::effective_device_class(DeviceType::DiscreteGpu, "RTX 4090", Some("low")),
            DeviceType::IntegratedGpu
        );
        assert_eq!(
            super::effective_device_class(DeviceType::IntegratedGpu, "Intel UHD", Some("high")),
            DeviceType::DiscreteGpu
        );
        // An integrated adapter never accidentally promotes itself.
        assert_eq!(
            super::effective_device_class(DeviceType::IntegratedGpu, "Intel Iris Xe", None),
            DeviceType::IntegratedGpu
        );
    }

    /// F2's contract: the integrated tier truly folds — no bloom chain, no cloud shadows, no
    /// full shader detail — while discrete keeps everything; WOT_GPU_DETAIL overrides both ways.
    #[test]
    fn the_integrated_tier_folds_and_the_override_flips_it() {
        use renderer_api::{GpuDeviceType, LightingQuality};
        let integrated = LightingQuality::for_device_type(GpuDeviceType::IntegratedGpu);
        assert!(!integrated.full_shader_detail, "iGPU folds the per-pixel detail");
        assert_eq!(integrated.bloom_mips, 0, "iGPU skips the bloom bandwidth");
        assert!(!integrated.cloud_shadows, "iGPU skips cloud shade ALU");
        let discrete = LightingQuality::for_device_type(GpuDeviceType::DiscreteGpu);
        assert!(discrete.full_shader_detail && discrete.cloud_shadows);
        assert_eq!(discrete.bloom_mips, 5);

        let forced = super::apply_shader_detail_override(integrated, Some("full"));
        assert!(forced.full_shader_detail, "WOT_GPU_DETAIL=full claws detail back");
        let folded = super::apply_shader_detail_override(discrete, Some("low"));
        assert!(!folded.full_shader_detail, "WOT_GPU_DETAIL=low previews the fold");
        let garbage = super::apply_shader_detail_override(discrete, Some("banana"));
        assert!(garbage.full_shader_detail, "garbage leaves the tier default");
    }

    use super::resolve_lighting_quality_with_bloom;

    #[test]
    fn integrated_adapters_halve_the_shadow_map_and_ssao_and_discrete_keep_them() {
        let integrated = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::IntegratedGpu,
            None,
            None,
            None,
            None,
        );
        assert_eq!(integrated.shadow_resolution, 2048);
        assert_eq!(integrated.shadow_cascades, 2);
        assert_eq!(integrated.ssao_scale, 0.5);

        let discrete = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::DiscreteGpu,
            None,
            None,
            None,
            None,
        );
        assert_eq!(discrete.shadow_resolution, 4096);
        assert_eq!(discrete.shadow_cascades, 2);
        assert_eq!(discrete.ssao_scale, 1.0);

        let cpu =
            resolve_lighting_quality_with_bloom(wgpu::DeviceType::Cpu, None, None, None, None);
        assert_eq!(cpu.shadow_resolution, 2048);
        assert_eq!(cpu.ssao_scale, 0.5);
    }

    #[test]
    fn the_refraction_override_forces_the_flag_both_ways_and_ignores_garbage() {
        use super::apply_refraction_override;
        let discrete = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::DiscreteGpu,
            None,
            None,
            None,
            None,
        );
        assert!(discrete.refraction, "premise: discrete refracts by default");
        assert!(!apply_refraction_override(discrete, Some("off")).refraction);
        assert!(!apply_refraction_override(discrete, Some("0")).refraction);

        let integrated = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::IntegratedGpu,
            None,
            None,
            None,
            None,
        );
        assert!(!integrated.refraction, "premise: integrated is analytic by default");
        assert!(apply_refraction_override(integrated, Some("on")).refraction);
        assert!(apply_refraction_override(integrated, Some("1")).refraction);
        // Garbage leaves the tier default untouched.
        assert!(!apply_refraction_override(integrated, Some("banana")).refraction);
        assert!(!apply_refraction_override(integrated, None).refraction);
    }

    #[test]
    fn env_overrides_win_both_ways_and_garbage_is_ignored() {
        let up = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::IntegratedGpu,
            Some("4096"),
            Some("1"),
            Some("full"),
            None,
        );
        assert_eq!(up.shadow_resolution, 4096);
        assert_eq!(up.shadow_cascades, 1);
        assert_eq!(up.ssao_scale, 1.0);

        let down = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::DiscreteGpu,
            Some("1024"),
            Some("2"),
            Some("half"),
            None,
        );
        assert_eq!(down.shadow_resolution, 1024);
        assert_eq!(down.ssao_scale, 0.5);

        let off = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::DiscreteGpu,
            None,
            None,
            Some("off"),
            None,
        );
        assert_eq!(off.ssao_scale, 0.0);

        let garbage = resolve_lighting_quality_with_bloom(
            wgpu::DeviceType::IntegratedGpu,
            Some("3000"),
            Some("7"),
            Some("x"),
            Some("banana"),
        );
        assert_eq!(garbage.shadow_resolution, 2048);
        assert_eq!(garbage.shadow_cascades, 2);
        assert_eq!(garbage.ssao_scale, 0.5);
    }

    #[test]
    fn lighting_memory_budget_is_locked_per_tier_at_1080p() {
        // The executable lighting-memory budget: both shadow cascades (Depth32Float, 4 B/texel)
        // plus the SSAO chain (a Depth32Float prepass + two R8 AO targets) at 1920×1080 scaled by
        // the tier's ssao_scale. Moving these numbers is a deliberate decision that belongs in
        // the same diff as the quality-table change.
        let tier_bytes = |device_type: wgpu::DeviceType| {
            let q = resolve_lighting_quality_with_bloom(device_type, None, None, None, None);
            let near = u64::from(q.shadow_resolution);
            let far = u64::from(
                (renderer_api::SunShadowParams {
                    resolution: q.shadow_resolution,
                    ..renderer_api::SunShadowParams::default()
                })
                .far_cascade()
                .resolution,
            );
            let shadows = 4 * (near * near + far * far);
            let (w, h) =
                ((1920.0 * q.ssao_scale).round() as u64, (1080.0 * q.ssao_scale).round() as u64);
            // 4 B prepass depth + 1 B raw AO + 1 B blurred AO per SSAO pixel.
            shadows + w * h * 6
        };
        // Integrated: 20 MB of shadows + a half-res SSAO chain (960×540×6 ≈ 3 MB).
        assert_eq!(tier_bytes(wgpu::DeviceType::IntegratedGpu), 24_081_920);
        // Discrete: 80 MB of shadows + a full-res SSAO chain (1920×1080×6 ≈ 12.4 MB).
        assert_eq!(tier_bytes(wgpu::DeviceType::DiscreteGpu), 96_327_680);
    }
}
