pub fn select_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else if supported.contains(&wgpu::PresentMode::AutoVsync) {
        wgpu::PresentMode::AutoVsync
    } else if supported.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else if supported.contains(&wgpu::PresentMode::FifoRelaxed) {
        wgpu::PresentMode::FifoRelaxed
    } else {
        supported.first().copied().unwrap_or(wgpu::PresentMode::Fifo)
    }
}
