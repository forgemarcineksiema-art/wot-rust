/// Choose the presentation mode. Mailbox is the best of both worlds — vsync-correct (never tears)
/// yet non-blocking, so a sub-refresh GPU shows its freshest frame immediately instead of parking
/// it behind whole vsync periods the way FIFO does (each ~40 ms frame stretched to an alternating
/// 33/50 ms cadence — the visible judder — plus up to a frame of latency). So Mailbox is preferred
/// in BOTH modes; the `vsync` flag only picks the FALLBACK when Mailbox is unavailable: FIFO (never
/// tears, but judders below refresh) with vsync, Immediate (tears, lowest latency) without.
pub fn select_present_mode(supported: &[wgpu::PresentMode], vsync: bool) -> wgpu::PresentMode {
    let preference: &[wgpu::PresentMode] = if vsync {
        // Never tear: Mailbox first, then the FIFO family (blocks, judders below refresh, but is
        // the one mode wgpu guarantees on every surface).
        &[
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::AutoVsync,
            wgpu::PresentMode::FifoRelaxed,
        ]
    } else {
        // Freshest frame wins, tearing acceptable.
        &[
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::AutoNoVsync,
            wgpu::PresentMode::Fifo,
        ]
    };
    preference
        .iter()
        .copied()
        .find(|mode| supported.contains(mode))
        .unwrap_or_else(|| supported.first().copied().unwrap_or(wgpu::PresentMode::Fifo))
}
