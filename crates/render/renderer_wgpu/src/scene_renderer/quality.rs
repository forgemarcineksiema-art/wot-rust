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

#[cfg(test)]
mod tests {
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
