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
pub(crate) fn resolve_msaa_samples(
    requested: u8,
    device_type: wgpu::DeviceType,
    env_override: Option<&str>,
) -> u32 {
    if let Some(value) = env_override.and_then(|value| value.trim().parse::<u32>().ok())
        && matches!(value, 1 | 2 | 4 | 8)
    {
        return value;
    }
    match device_type {
        wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu => 1,
        _ => u32::from(requested),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_msaa_samples;

    #[test]
    fn integrated_adapters_drop_to_no_msaa_and_discrete_keep_the_request() {
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::IntegratedGpu, None), 1);
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::Cpu, None), 1);
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::DiscreteGpu, None), 4);
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::Other, None), 4);
    }

    #[test]
    fn the_env_override_wins_both_ways_and_garbage_is_ignored() {
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::IntegratedGpu, Some("4")), 4);
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::DiscreteGpu, Some("1")), 1);
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::DiscreteGpu, Some("3")), 4);
        assert_eq!(resolve_msaa_samples(4, wgpu::DeviceType::IntegratedGpu, Some("abc")), 1);
    }
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
