use renderer_wgpu::select_present_mode;

/// Mailbox is vsync-correct (never tears) AND non-blocking, so it wins under vsync too — it is
/// what keeps a sub-refresh GPU smooth instead of parking every frame behind a whole vsync period
/// (FIFO's 33/50 ms judder).
#[test]
fn with_vsync_prefers_mailbox_over_fifo() {
    let selected = select_present_mode(
        &[wgpu::PresentMode::Immediate, wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox],
        true,
    );

    assert_eq!(selected, wgpu::PresentMode::Mailbox);
}

/// When the surface has no Mailbox, vsync falls back to FIFO (never tears, universally available)
/// rather than a tearing mode — even if a tearing mode is on offer.
#[test]
fn with_vsync_falls_back_to_fifo_without_mailbox() {
    let selected = select_present_mode(
        &[wgpu::PresentMode::Immediate, wgpu::PresentMode::AutoVsync, wgpu::PresentMode::Fifo],
        true,
    );

    assert_eq!(selected, wgpu::PresentMode::Fifo);
}

/// Without vsync a sub-refresh machine escapes FIFO's whole-period quantization: Mailbox is still
/// preferred (fresh frames, no tearing), Immediate is the fallback, and a FIFO-only surface still
/// works.
#[test]
fn without_vsync_mailbox_then_immediate_then_fifo() {
    let all = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo];
    assert_eq!(select_present_mode(&all, false), wgpu::PresentMode::Mailbox);

    let no_mailbox = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Fifo];
    assert_eq!(select_present_mode(&no_mailbox, false), wgpu::PresentMode::Immediate);

    let fifo_only = [wgpu::PresentMode::Fifo];
    assert_eq!(select_present_mode(&fifo_only, false), wgpu::PresentMode::Fifo);
}
