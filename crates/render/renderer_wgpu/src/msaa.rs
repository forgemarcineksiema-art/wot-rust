use renderer_api::{DEFAULT_MSAA_SAMPLES, RenderError};

use crate::GpuContext;

/// The sample count REVIEW images are rendered at: golden look frames, studio tiles, and every
/// `*_probe` that produces a picture for a human to judge.
///
/// THE SAME COUNT THE GAME SHIPS. It was 4x for the review path against 1x on every player's
/// screen — a named debt, held until re-recording the goldens was taken as a decision. It was
/// taken (2026-08-11): an instrument that renders a cleaner picture than the player ever sees
/// grades every model, every material and every lighting change on evidence the game does not
/// produce. Every visual judgement made through this path — including the audits that shaped
/// the T-54 — was made on the flattering copy. The goldens are re-recorded at the shipped
/// count in the same change, and the divergence pin
/// (`the_review_path_is_pinned_and_does_not_claim_to_be_the_shipped_game`) is retired on its
/// own instruction.
pub(crate) fn review_sample_count() -> u32 {
    resolve_msaa_samples(
        DEFAULT_MSAA_SAMPLES,
        rich_profile_requested(),
        std::env::var("WOT_MSAA").ok().as_deref(),
    )
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

    /// THE INSTRUMENT AND THE GAME ARE THE SAME PICTURE. This replaces the divergence pin that
    /// held the 4x-review / 1x-shipped gap as a named debt: the debt was paid on 2026-08-11 and
    /// that test retired on its own instruction. What stays locked is the agreement — a review
    /// path that quietly drifts back to a cleaner sample count than the player's screen makes
    /// every golden and every audit describe a picture nobody plays.
    #[test]
    fn the_review_path_renders_the_picture_the_game_ships() {
        let shipped = resolve_msaa_samples(DEFAULT_MSAA_SAMPLES, false, None);
        assert_eq!(shipped, 1, "the shipped window is 1x on every adapter");
        // The review path resolves through the same knobs, so absent dev overrides it must land
        // on the shipped count. (Under WOT_QUALITY=high / WOT_MSAA both move together, which is
        // exactly the point: one resolution, one picture.)
        if std::env::var("WOT_QUALITY").is_err() && std::env::var("WOT_MSAA").is_err() {
            assert_eq!(review_sample_count(), shipped, "goldens grade what the player sees");
        }
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
