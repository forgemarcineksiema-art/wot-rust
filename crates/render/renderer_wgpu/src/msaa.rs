use renderer_api::{DEFAULT_MSAA_SAMPLES, RenderError};

use crate::GpuContext;

pub(crate) fn default_sample_count() -> u32 {
    u32::from(DEFAULT_MSAA_SAMPLES)
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
