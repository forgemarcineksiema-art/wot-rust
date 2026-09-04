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
//! - `WOT_CLOUD_SHADOWS=on|off` — the terrain's cloud-shade slice. Its own knob so the buy-back
//!   can be measured as ONE variable on the min-spec: `WOT_QUALITY=high` moves five things at
//!   once and answers nothing about this one.

use renderer_api::LightingQuality;

/// The base profile before the per-knob env overrides (one-look policy): everyone ships the
/// canonical look; `WOT_QUALITY=high` is the DEV-ONLY rich profile for captures and look-lock
/// comparison renders. There is no player-facing quality selection by design — the game owns
/// its performance instead of handing the player a settings menu.
pub(crate) fn resolve_base_profile(quality_env: Option<&str>) -> LightingQuality {
    match quality_env.map(str::trim) {
        Some("high") => LightingQuality::rich(),
        _ => LightingQuality::canonical(),
    }
}

pub(crate) fn resolve_lighting_quality_with_bloom(
    base: LightingQuality,
    shadow_res_env: Option<&str>,
    cascades_env: Option<&str>,
    ssao_env: Option<&str>,
    bloom_env: Option<&str>,
) -> LightingQuality {
    let mut quality = base;
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

/// Apply the `WOT_CLOUD_SHADOWS=on|off` (or `1|0`) override. Cloud shade ships in the canonical
/// tier since the baked-tile buy-back (D21); the knob stays so its cost keeps being measurable
/// as ONE variable, per the Żywy Step protocol. Garbage is ignored.
pub(crate) fn apply_cloud_shadow_override(
    mut quality: LightingQuality,
    cloud_env: Option<&str>,
) -> LightingQuality {
    match cloud_env.map(str::trim) {
        Some("on" | "1") => quality.cloud_shadows = true,
        Some("off" | "0") => quality.cloud_shadows = false,
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
        Some("full" | "1") => quality.shader_detail = renderer_api::ShaderDetailMask::FULL,
        Some("low" | "0") => quality.shader_detail = renderer_api::ShaderDetailMask::NONE,
        // A raw number is a bit mask (Żywy Step buy-back protocol): measure one feature at
        // a time on the min-spec, e.g. WOT_GPU_DETAIL=3 = terrain bend + micro octave.
        Some(mask) if mask.parse::<u32>().is_ok() => {
            quality.shader_detail = renderer_api::ShaderDetailMask(
                mask.parse::<u32>().unwrap_or(0) & renderer_api::ShaderDetailMask::FULL.0,
            )
        }
        _ => {}
    }
    quality
}

#[cfg(test)]
mod tests {

    /// One-look policy over F2's folds: the canonical profile ships folded (no bloom, reduced
    /// detail; cloud shade is IN since the baked-tile buy-back) for EVERY adapter, the
    /// dev-only rich profile keeps everything, and WOT_GPU_DETAIL still flips the detail lane
    /// for profiling.
    #[test]
    fn the_canonical_look_folds_and_the_dev_overrides_flip_it() {
        use renderer_api::LightingQuality;
        let canonical = LightingQuality::canonical();
        assert!(
            canonical.shader_detail
                == renderer_api::ShaderDetailMask(
                    renderer_api::ShaderDetailMask::TERRAIN_NORMAL_BEND
                        | renderer_api::ShaderDetailMask::TERRAIN_MICRO_OCTAVE
                )
                && canonical.cloud_shadows
        );
        assert_eq!(canonical.bloom_mips, 0);
        let rich = LightingQuality::rich();
        assert!(
            rich.shader_detail == renderer_api::ShaderDetailMask::FULL
                && rich.cloud_shadows
                && rich.bloom_mips == 5
        );

        assert_eq!(super::resolve_base_profile(Some("high")), rich, "WOT_QUALITY=high = dev rich");
        assert_eq!(super::resolve_base_profile(None), canonical);
        assert_eq!(super::resolve_base_profile(Some("banana")), canonical, "garbage = canonical");

        let forced = super::apply_shader_detail_override(canonical, Some("full"));
        assert_eq!(
            forced.shader_detail,
            renderer_api::ShaderDetailMask::FULL,
            "WOT_GPU_DETAIL=full claws detail back"
        );
        let folded = super::apply_shader_detail_override(rich, Some("low"));
        assert_eq!(
            folded.shader_detail,
            renderer_api::ShaderDetailMask::NONE,
            "WOT_GPU_DETAIL=low previews the fold"
        );
    }

    use super::resolve_lighting_quality_with_bloom;

    #[test]
    fn the_canonical_and_rich_profiles_flow_through_the_env_resolver() {
        let integrated = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::canonical(),
            None,
            None,
            None,
            None,
        );
        assert_eq!(integrated.shadow_resolution, 2048);
        assert_eq!(integrated.shadow_cascades, 2);
        assert_eq!(integrated.ssao_scale, 0.5);

        let discrete = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::rich(),
            None,
            None,
            None,
            None,
        );
        assert_eq!(discrete.shadow_resolution, 4096);
        assert_eq!(discrete.shadow_cascades, 2);
        assert_eq!(discrete.ssao_scale, 1.0);

        let cpu = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::canonical(),
            None,
            None,
            None,
            None,
        );
        assert_eq!(cpu.shadow_resolution, 2048);
        assert_eq!(cpu.ssao_scale, 0.5);
    }

    /// The knob exists so a buy-back can be measured as ONE variable. If it stopped forcing the
    /// flag both ways, a measurement would silently report the tier default and "we measured it"
    /// would become a lie — which is worse than not measuring.
    #[test]
    fn the_cloud_shadow_override_forces_the_flag_both_ways_and_ignores_garbage() {
        use super::apply_cloud_shadow_override;
        let shipped = renderer_api::LightingQuality::canonical();
        assert!(shipped.cloud_shadows, "premise: cloud shade ships since the baked-tile D21");
        assert!(!apply_cloud_shadow_override(shipped, Some("off")).cloud_shadows);
        assert!(!apply_cloud_shadow_override(shipped, Some("0")).cloud_shadows);

        let mut bare = renderer_api::LightingQuality::canonical();
        bare.cloud_shadows = false;
        assert!(apply_cloud_shadow_override(bare, Some("on")).cloud_shadows);
        assert!(apply_cloud_shadow_override(bare, Some("1")).cloud_shadows);

        // Garbage leaves the tier default untouched, in both directions.
        assert!(!apply_cloud_shadow_override(bare, Some("banana")).cloud_shadows);
        assert!(!apply_cloud_shadow_override(bare, None).cloud_shadows);
        assert!(apply_cloud_shadow_override(shipped, Some("banana")).cloud_shadows);
    }

    #[test]
    fn the_refraction_override_forces_the_flag_both_ways_and_ignores_garbage() {
        use super::apply_refraction_override;
        let discrete = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::rich(),
            None,
            None,
            None,
            None,
        );
        assert!(discrete.refraction, "premise: discrete refracts by default");
        assert!(!apply_refraction_override(discrete, Some("off")).refraction);
        assert!(!apply_refraction_override(discrete, Some("0")).refraction);

        let integrated = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::canonical(),
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
            renderer_api::LightingQuality::canonical(),
            Some("4096"),
            Some("1"),
            Some("full"),
            None,
        );
        assert_eq!(up.shadow_resolution, 4096);
        assert_eq!(up.shadow_cascades, 1);
        assert_eq!(up.ssao_scale, 1.0);

        let down = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::rich(),
            Some("1024"),
            Some("2"),
            Some("half"),
            None,
        );
        assert_eq!(down.shadow_resolution, 1024);
        assert_eq!(down.ssao_scale, 0.5);

        let off = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::rich(),
            None,
            None,
            Some("off"),
            None,
        );
        assert_eq!(off.ssao_scale, 0.0);

        let garbage = resolve_lighting_quality_with_bloom(
            renderer_api::LightingQuality::canonical(),
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
        let tier_bytes = |profile: renderer_api::LightingQuality| {
            let q = resolve_lighting_quality_with_bloom(profile, None, None, None, None);
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
        // The ONE shipped look: 20 MB of shadows + a half-res SSAO chain (960×540×6 ≈ 3 MB).
        assert_eq!(tier_bytes(renderer_api::LightingQuality::canonical()), 24_081_920);
        // The dev-only rich profile: 80 MB of shadows + full-res SSAO (capture rigs only).
        assert_eq!(tier_bytes(renderer_api::LightingQuality::rich()), 96_327_680);
    }
}
