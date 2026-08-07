use renderer_api::{DEFAULT_MSAA_SAMPLES, RenderError};

use crate::GpuContext;

/// The sample count REVIEW images are rendered at: golden look frames, studio tiles, and every
/// `*_probe` that produces a picture for a human to judge.
///
/// This is NOT what the game renders. The window resolves to 1x on every adapter
/// ([`resolve_msaa_samples`]), so a review frame is multisampled 4x while the shipped frame is
/// not. That divergence is a KNOWN DEBT, deliberately held here rather than fixed by flipping
/// the goldens: re-recording 24 committed frames is an art-direction decision, and it needs the
/// measured 1x-versus-4x cost per pass before anyone can take it. Until then the debt is at
/// least named, and it is pinned by
/// `the_review_path_is_pinned_and_does_not_claim_to_be_the_shipped_game`.
///
/// Anything that MEASURES the game — frame-time probes — must use [`shipped_sample_count`]
/// instead, so the instrument and the thing it measures are the same picture.
pub(crate) fn review_sample_count() -> u32 {
    u32::from(DEFAULT_MSAA_SAMPLES)
}

/// The sample count the SHIPPED game resolves to, read through the same two knobs the window
/// reads. This is the one place that resolution lives, so an offscreen instrument and the window
/// cannot drift apart — they did, silently, and every frame-time number this project has ever
/// quoted was taken at 4x while the game ran at 1x.
pub(crate) fn shipped_sample_count(requested: u8) -> u32 {
    resolve_msaa_samples(
        requested,
        rich_profile_requested(),
        std::env::var("WOT_MSAA").ok().as_deref(),
    )
}

/// The dev-only `WOT_QUALITY=high` profile, read the same way on every path that asks.
pub(crate) fn rich_profile_requested() -> bool {
    std::env::var("WOT_QUALITY").ok().as_deref().map(str::trim) == Some("high")
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
    use renderer_api::DEFAULT_MSAA_SAMPLES;

    use super::{resolve_msaa_samples, review_sample_count};

    /// The review path is multisampled and the shipped game is not — the divergence that made
    /// every frame-time number this project has quoted describe a picture nobody plays, and made
    /// every golden PNG a cleaner image than the one on screen.
    ///
    /// Both numbers are written down here so the gap is a decision with a size, not a surprise
    /// waiting to be found a third time. When the goldens move to the shipped count, this test
    /// fails on its own `assert_ne!` — that is the signal to delete it together with the debt
    /// note on [`review_sample_count`].
    #[test]
    fn the_review_path_is_pinned_and_does_not_claim_to_be_the_shipped_game() {
        let shipped = resolve_msaa_samples(DEFAULT_MSAA_SAMPLES, false, None);
        assert_eq!(review_sample_count(), 4, "review images stay multisampled for now");
        assert_eq!(shipped, 1, "the shipped window is 1x on every adapter");
        assert_ne!(
            review_sample_count(),
            shipped,
            "the review path and the game agree now — retire this test and the debt it names"
        );
    }

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
