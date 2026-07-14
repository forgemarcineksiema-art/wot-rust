use renderer_api::{DEFAULT_MSAA_SAMPLES, RenderError};

use crate::GpuContext;

pub(crate) fn default_sample_count() -> u32 {
    u32::from(DEFAULT_MSAA_SAMPLES)
}

/// The sample count the window renderer actually uses: the caller's request, cut to 1× on
/// integrated/software adapters. Every attachment and every blended pass (water, FX, rain,
/// HUD) pays `sample_count ×` fill bandwidth, and a shared-memory GPU is bandwidth-bound
/// first — 4× MSAA is one of the largest slices of the 20-30 FPS laptop frame. `WOT_MSAA=1|2|4`
/// overrides in both directions (force MSAA back on an iGPU, or drop it on a discrete card).
pub(crate) fn resolve_msaa_samples(requested: u8, rich: bool, env_override: Option<&str>) -> u32 {
    if let Some(value) = env_override.and_then(|value| value.trim().parse::<u32>().ok())
        && matches!(value, 1 | 2 | 4 | 8)
    {
        return value;
    }
    // One-look policy: the canonical picture is 1× on EVERY adapter (the minimum spec cannot
    // afford multisampling, so nobody ships it — equal picture, equal game). The dev-only
    // rich profile (WOT_QUALITY=high) keeps the requested count for captures.
    if rich { u32::from(requested) } else { 1 }
}

pub(crate) fn validate_msaa_support(
    ctx: &GpuContext,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    sample_count: u32,
) -> Result<(), RenderError> {
    validate_sample_count(sample_count)?;
    if sample_count == 1 {
        return Ok(());
    }

    let color_flags = ctx.adapter.get_texture_format_features(color_format).flags;
    if !color_flags.sample_count_supported(sample_count) {
        return Err(RenderError::new(format!(
            "{color_format:?} does not support {sample_count}x MSAA"
        )));
    }
    if !color_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE) {
        return Err(RenderError::new(format!("{color_format:?} does not support MSAA resolve")));
    }

    let depth_flags = ctx.adapter.get_texture_format_features(depth_format).flags;
    if !depth_flags.sample_count_supported(sample_count) {
        return Err(RenderError::new(format!(
            "{depth_format:?} does not support {sample_count}x MSAA"
        )));
    }

    Ok(())
}

fn validate_sample_count(sample_count: u32) -> Result<(), RenderError> {
    match sample_count {
        1 | 2 | 4 | 8 | 16 => Ok(()),
        _ => Err(RenderError::new(format!("unsupported MSAA sample count: {sample_count}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_msaa_samples;

    /// One-look policy: the shipped picture is 1× on EVERY adapter; only the dev-only rich
    /// profile keeps the requested count, and the env override wins over both.
    #[test]
    fn everyone_ships_no_msaa_and_only_the_dev_rich_profile_keeps_the_request() {
        assert_eq!(resolve_msaa_samples(4, false, None), 1, "canonical = 1x for all");
        assert_eq!(resolve_msaa_samples(4, true, None), 4, "rich (dev) keeps the request");
        assert_eq!(resolve_msaa_samples(4, false, Some("4")), 4, "env override wins");
        assert_eq!(resolve_msaa_samples(4, true, Some("1")), 1, "env override wins both ways");
        assert_eq!(resolve_msaa_samples(4, true, Some("3")), 4, "invalid counts fall through");
        assert_eq!(resolve_msaa_samples(4, false, Some("abc")), 1, "garbage is ignored");
    }
}
