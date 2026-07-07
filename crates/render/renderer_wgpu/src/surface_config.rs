/// Choose the presentation mode. With vsync the FIFO family is right: at or above the panel's
/// refresh it paces perfectly and never tears. Without vsync, prefer the modes that show a
/// finished frame as soon as possible — on a GPU that cannot hold refresh, FIFO parks every
/// ~40 ms frame behind whole vsync periods (each shown for alternating 33/50 ms — the visible
/// judder) and adds up to a frame of latency on top.
pub fn select_present_mode(supported: &[wgpu::PresentMode], vsync: bool) -> wgpu::PresentMode {
    if !vsync {
        let fast = [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::AutoNoVsync,
        ];
        if let Some(mode) = fast.into_iter().find(|mode| supported.contains(mode)) {
            return mode;
        }
    }
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
