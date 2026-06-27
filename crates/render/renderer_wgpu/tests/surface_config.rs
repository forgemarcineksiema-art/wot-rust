use renderer_wgpu::select_present_mode;

#[test]
fn present_mode_prefers_vsync_over_adapter_order() {
    let selected = select_present_mode(&[
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Mailbox,
        wgpu::PresentMode::Fifo,
    ]);

    assert_eq!(selected, wgpu::PresentMode::Fifo);
}

#[test]
fn present_mode_uses_fifo_even_when_auto_vsync_is_available() {
    let selected = select_present_mode(&[
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::AutoVsync,
        wgpu::PresentMode::Fifo,
    ]);

    assert_eq!(selected, wgpu::PresentMode::Fifo);
}
