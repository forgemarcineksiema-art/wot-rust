use renderer_wgpu::select_present_mode;

#[test]
fn with_vsync_fifo_wins_regardless_of_adapter_order() {
    let selected = select_present_mode(
        &[wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo],
        true,
    );

    assert_eq!(selected, wgpu::PresentMode::Fifo);
}

#[test]
fn with_vsync_fifo_wins_even_when_auto_vsync_is_available() {
    let selected = select_present_mode(
        &[wgpu::PresentMode::Immediate, wgpu::PresentMode::AutoVsync, wgpu::PresentMode::Fifo],
        true,
    );

    assert_eq!(selected, wgpu::PresentMode::Fifo);
}

/// Without vsync a sub-refresh machine escapes FIFO's whole-period quantization: Mailbox is
/// preferred (fresh frames, no tearing), Immediate is the fallback, and a FIFO-only surface
/// still works.
#[test]
fn without_vsync_mailbox_then_immediate_then_fifo() {
    let all = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo];
    assert_eq!(select_present_mode(&all, false), wgpu::PresentMode::Mailbox);

    let no_mailbox = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Fifo];
    assert_eq!(select_present_mode(&no_mailbox, false), wgpu::PresentMode::Immediate);

    let fifo_only = [wgpu::PresentMode::Fifo];
    assert_eq!(select_present_mode(&fifo_only, false), wgpu::PresentMode::Fifo);
}
