//! Alpha-tested foliage mip generation.
//!
//! Filtering happens in linear light with premultiplied alpha, then returns to straight
//! RGBA8 sRGB for the GPU. Alpha is rescaled after every reduction so the 0.5 cutout keeps
//! the source image's coverage instead of making distant crowns evaporate.

use renderer_api::Rgba8MipLevel;
use std::collections::VecDeque;

const ALPHA_CUTOFF_BYTE: u8 = 128;
const ALPHA_SCALE_ONE: u32 = 1 << 16;
const ALPHA_SCALE_MAX: u32 = 256 << 16;

pub(super) fn build_asset_mip_chain(width: u32, height: u32, rgba: Vec<u8>) -> Vec<Rgba8MipLevel> {
    let base_covered = rgba.chunks_exact(4).filter(|pixel| pixel[3] >= ALPHA_CUTOFF_BYTE).count();
    let base_pixels = (width * height) as usize;
    let mut base = MutableMipLevel { width, height, rgba };
    dilate_cutout_rgb(&mut base);
    let mut levels: Vec<Rgba8MipLevel> = vec![base.into()];
    while levels.last().expect("base").width() > 1 || levels.last().expect("base").height() > 1 {
        let parent = levels.last().expect("base");
        let mut next = downsample_flora_level(parent);
        preserve_flora_coverage(next.rgba_mut(), base_covered, base_pixels);
        dilate_cutout_rgb(&mut next);
        levels.push(next.into());
    }
    levels
}

fn downsample_flora_level(parent: &Rgba8MipLevel) -> MutableMipLevel {
    let width = (parent.width() / 2).max(1);
    let height = (parent.height() / 2).max(1);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let mut alpha_sum = 0.0;
            let mut premul_sum = [0.0f32; 3];
            let mut sample_count = 0.0;
            for source_y in y * 2..(y * 2 + 2).min(parent.height()) {
                for source_x in x * 2..(x * 2 + 2).min(parent.width()) {
                    let offset = ((source_y * parent.width() + source_x) * 4) as usize;
                    let alpha = parent.rgba()[offset + 3] as f32 / 255.0;
                    for (channel, sum) in premul_sum.iter_mut().enumerate() {
                        *sum += flora_srgb_to_linear(parent.rgba()[offset + channel]) * alpha;
                    }
                    alpha_sum += alpha;
                    sample_count += 1.0;
                }
            }
            let alpha = alpha_sum / sample_count;
            let dst = ((y * width + x) * 4) as usize;
            if alpha_sum > 0.0 {
                for (channel, sum) in premul_sum.iter().enumerate() {
                    rgba[dst + channel] = flora_linear_to_srgb(*sum / alpha_sum);
                }
            }
            rgba[dst + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    MutableMipLevel { width, height, rgba }
}

fn preserve_flora_coverage(rgba: &mut [u8], base_covered: usize, base_pixels: usize) {
    let pixels = rgba.len() / 4;
    let desired = ((base_covered as u64 * pixels as u64 + base_pixels as u64 / 2)
        / base_pixels as u64) as usize;
    let mut low = 0u32;
    let mut high = ALPHA_SCALE_MAX;
    while low < high {
        let mid = low + (high - low) / 2;
        if scaled_coverage_count(rgba, mid) < desired {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    let candidates = [low, low.saturating_sub(1), ALPHA_SCALE_ONE];
    let scale = candidates
        .into_iter()
        .min_by_key(|candidate| {
            (
                scaled_coverage_count(rgba, *candidate).abs_diff(desired),
                candidate.abs_diff(ALPHA_SCALE_ONE),
            )
        })
        .expect("three candidates");
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = scaled_flora_alpha(pixel[3], scale);
    }
}

fn dilate_cutout_rgb(level: &mut MutableMipLevel) {
    let pixel_count = (level.width * level.height) as usize;
    let mut reached = vec![false; pixel_count];
    let mut frontier = VecDeque::with_capacity(pixel_count);
    for (pixel, reached_pixel) in reached.iter_mut().enumerate() {
        if level.rgba[pixel * 4 + 3] >= ALPHA_CUTOFF_BYTE {
            *reached_pixel = true;
            frontier.push_back(pixel);
        }
    }
    if frontier.is_empty() {
        for (pixel, reached_pixel) in reached.iter_mut().enumerate() {
            if level.rgba[pixel * 4 + 3] != 0 {
                *reached_pixel = true;
                frontier.push_back(pixel);
            }
        }
    }

    while let Some(pixel) = frontier.pop_front() {
        let x = pixel as u32 % level.width;
        let y = pixel as u32 / level.width;
        let offset = pixel * 4;
        let rgb = [level.rgba[offset], level.rgba[offset + 1], level.rgba[offset + 2]];
        if x > 0 {
            reach_cutout_pixel(&mut level.rgba, &mut reached, &mut frontier, pixel - 1, rgb);
        }
        if x + 1 < level.width {
            reach_cutout_pixel(&mut level.rgba, &mut reached, &mut frontier, pixel + 1, rgb);
        }
        if y > 0 {
            reach_cutout_pixel(
                &mut level.rgba,
                &mut reached,
                &mut frontier,
                pixel - level.width as usize,
                rgb,
            );
        }
        if y + 1 < level.height {
            reach_cutout_pixel(
                &mut level.rgba,
                &mut reached,
                &mut frontier,
                pixel + level.width as usize,
                rgb,
            );
        }
    }
}

fn reach_cutout_pixel(
    rgba: &mut [u8],
    reached: &mut [bool],
    frontier: &mut VecDeque<usize>,
    pixel: usize,
    rgb: [u8; 3],
) {
    if reached[pixel] {
        return;
    }
    let offset = pixel * 4;
    debug_assert!(rgba[offset + 3] < ALPHA_CUTOFF_BYTE);
    rgba[offset..offset + 3].copy_from_slice(&rgb);
    reached[pixel] = true;
    frontier.push_back(pixel);
}

fn scaled_coverage_count(rgba: &[u8], scale: u32) -> usize {
    rgba.chunks_exact(4)
        .filter(|pixel| scaled_flora_alpha(pixel[3], scale) >= ALPHA_CUTOFF_BYTE)
        .count()
}

fn scaled_flora_alpha(alpha: u8, scale: u32) -> u8 {
    (((alpha as u64 * scale as u64 + (ALPHA_SCALE_ONE / 2) as u64) >> 16).min(255)) as u8
}

fn flora_srgb_to_linear(byte: u8) -> f32 {
    let value = byte as f32 / 255.0;
    if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
}

fn flora_linear_to_srgb(linear: f32) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let srgb =
        if linear <= 0.003_130_8 { linear * 12.92 } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

struct MutableMipLevel {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl MutableMipLevel {
    fn rgba_mut(&mut self) -> &mut [u8] {
        &mut self.rgba
    }
}

impl From<MutableMipLevel> for Rgba8MipLevel {
    fn from(level: MutableMipLevel) -> Self {
        Self::new(level.width, level.height, level.rgba)
    }
}
