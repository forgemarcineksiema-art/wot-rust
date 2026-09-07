//! Shared pixel-measure fixtures for the renderer's frame tests.
#![allow(dead_code)]

/// Rec. 601 luma of one RGBA pixel, normalized to [0, 1].
pub fn luma(p: &[u8]) -> f32 {
    (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0
}

/// Rec. 601 luma of one RGBA pixel on the raw 0..255 scale. Not the same measure as [`luma`]:
/// two helpers that shared a name until the burn made the scale difference visible.
pub fn luma_255(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// The identity model matrix.
pub fn identity() -> [[f32; 4]; 4] {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

/// Rec. 709 luma summed over a whole RGBA frame.
pub fn total_luma(pixels: &[u8]) -> f64 {
    pixels
        .chunks_exact(4)
        .map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64)
        .sum()
}

/// A headless GPU context that has the GPU TO ITSELF for as long as it lives (2026-09-07).
///
/// Every GPU test in the `suite` binary used to open its own context on its own thread, and
/// under the full gate they ran side by side on the MX330: a shadow frame came back without
/// its shadow, an MSAA frame came back black, the HUD pass timed 3 ms instead of 0.4 — the
/// same tests green every time alone. A frame is only worth locking when the device is not
/// shared, so the contexts serialise through one process-wide lock; a panicking test poisons
/// nothing (the guard is taken back), and a test that cannot get a device skips as before.
pub struct HeadlessGpu {
    _exclusive: std::sync::MutexGuard<'static, ()>,
    ctx: renderer_wgpu::GpuContext,
}

impl std::ops::Deref for HeadlessGpu {
    type Target = renderer_wgpu::GpuContext;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

static GPU_EXCLUSIVE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The one door to a headless device for the suite: `what` names the test in the skip line.
pub fn headless(what: &str) -> Option<HeadlessGpu> {
    let exclusive = GPU_EXCLUSIVE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match renderer_wgpu::GpuContext::headless() {
        Ok(ctx) => Some(HeadlessGpu { _exclusive: exclusive, ctx }),
        Err(error) => {
            eprintln!("skipping {what}: {error}");
            None
        }
    }
}
