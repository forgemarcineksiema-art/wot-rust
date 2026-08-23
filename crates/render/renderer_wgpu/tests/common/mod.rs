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
