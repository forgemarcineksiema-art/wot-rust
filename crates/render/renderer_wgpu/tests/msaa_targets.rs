use renderer_api::DEFAULT_MSAA_SAMPLES;
use renderer_wgpu::{GpuContext, OffscreenTarget, clear_color};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping MSAA target test: {error}");
            None
        }
    }
}

#[test]
fn offscreen_target_defaults_to_four_sample_msaa_with_resolve() {
    let Some(ctx) = headless_context() else {
        return;
    };

    let target = OffscreenTarget::new(&ctx, 64, 64).expect("offscreen target");
    let render_target = target.render_target();

    assert_eq!(target.sample_count(), u32::from(DEFAULT_MSAA_SAMPLES));
    assert_eq!(render_target.sample_count, u32::from(DEFAULT_MSAA_SAMPLES));
    assert!(render_target.resolve_target.is_some());
}

#[test]
fn clear_color_resolves_msaa_target_into_readable_rgba() {
    let Some(ctx) = headless_context() else {
        return;
    };

    let target = OffscreenTarget::new(&ctx, 4, 4).expect("offscreen target");

    clear_color(&ctx, &target, [0.25, 0.50, 0.75, 1.0]).expect("clear color");
    let pixels = target.read_rgba8(&ctx).expect("read rgba");

    assert_eq!(pixels.len(), 4 * 4 * 4);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0));
    assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] > 200));
}
