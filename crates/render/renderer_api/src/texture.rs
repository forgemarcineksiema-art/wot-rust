//! Renderer-neutral texture upload payloads.
//!
//! Asset builders own filtering and packing policy; backends only validate and upload the
//! resulting complete chain. Keeping the bytes here avoids a dependency from world building
//! into a concrete renderer.

/// One tightly packed RGBA8 mip level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8MipLevel {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Rgba8MipLevel {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert!(width > 0 && height > 0, "a texture mip cannot be empty");
        assert_eq!(rgba.len(), (width * height * 4) as usize, "tight RGBA8 mip data");
        Self { width, height, rgba }
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// A complete RGBA8 mip chain, including the 1x1 tail.
///
/// `max_sampled_level` may stop minification before packing regions become sub-texel; the
/// complete tail is still present for backend portability and deterministic inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8MipChain {
    levels: Vec<Rgba8MipLevel>,
    max_sampled_level: u32,
}

impl Rgba8MipChain {
    pub fn new(levels: Vec<Rgba8MipLevel>, max_sampled_level: u32) -> Self {
        assert!(!levels.is_empty(), "a mip chain needs a base level");
        for pair in levels.windows(2) {
            assert_eq!(pair[1].width, (pair[0].width / 2).max(1), "complete mip widths");
            assert_eq!(pair[1].height, (pair[0].height / 2).max(1), "complete mip heights");
        }
        let last = levels.last().expect("non-empty");
        assert_eq!((last.width, last.height), (1, 1), "a complete chain ends at 1x1");
        assert!((max_sampled_level as usize) < levels.len(), "sampled mip is in the chain");
        Self { levels, max_sampled_level }
    }

    pub fn levels(&self) -> &[Rgba8MipLevel] {
        &self.levels
    }

    pub const fn max_sampled_level(&self) -> u32 {
        self.max_sampled_level
    }

    /// Build the complete chain down to 1x1 from one base level. Every level is sampleable
    /// (`max_sampled_level` = the tail), and the filter is CPU-side and deterministic on
    /// purpose: these chains are golden-hashed and upload once per scene.
    ///
    /// Hoisted from the ground-map uploader in `renderer_wgpu` (Drzewa 3.0 PR2) so the foliage
    /// atlas can share the walk without pasting the body — with [`MipMode::Box`] the math is
    /// bit-identical to what the splat/macro maps always shipped.
    pub fn build(base: Rgba8MipLevel, mode: MipMode) -> Self {
        // The coverage reference is the BASE level: whatever area the authored mask shows at
        // the 0.5 cutout is the area every mip must keep showing.
        let base_coverage = cutout_coverage(&base.rgba);
        let mut levels = vec![base];
        loop {
            let previous = levels.last().expect("non-empty");
            if (previous.width, previous.height) == (1, 1) {
                break;
            }
            let next = downsample_box(previous, mode);
            levels.push(next);
        }
        if let MipMode::AlphaCoveragePreserving = mode {
            for level in levels.iter_mut().skip(1) {
                preserve_cutout_coverage(&mut level.rgba, base_coverage);
            }
        }
        let max_sampled_level = levels.len() as u32 - 1;
        Self::new(levels, max_sampled_level)
    }
}

/// How [`Rgba8MipChain::build`] filters a mip step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MipMode {
    /// Plain 2x2 box average per channel (odd edges clamp) — the ground-map filter. Linear, so
    /// splat weight sums survive every level; wrong for a cutout mask, whose 0.5-threshold
    /// coverage melts a little at every step until a distant crown thins into sticks.
    Box,
    /// The cutout-honest filter (Castano): color averages weighted by alpha so transparent
    /// texels never bleed their (meaningless) color into the rim, and each level's alpha is
    /// rescaled so the area above [`ALPHA_CUTOUT`] matches the base level's. This is what a
    /// leaf mask must ride — the 55–150 m band samples deep mips, and a crown that keeps its
    /// coverage there is the difference between foliage and twigs.
    AlphaCoveragePreserving,
}

/// The alpha-cutout threshold the shaders discard under (`alpha < 0.5`), in u8 texels. The
/// coverage-preserving mip mode holds area at THIS threshold; if a shader ever moves its
/// discard, this constant is the one place the chain has to follow.
pub const ALPHA_CUTOUT: u8 = 128;

/// Fraction of texels at or above the cutout threshold.
fn cutout_coverage(rgba: &[u8]) -> f32 {
    let texels = rgba.len() / 4;
    if texels == 0 {
        return 0.0;
    }
    let covered = rgba.chunks_exact(4).filter(|texel| texel[3] >= ALPHA_CUTOUT).count();
    covered as f32 / texels as f32
}

/// One 2x2 box step (odd edges clamp). `Box` averages every channel alike; the coverage mode
/// weights color by alpha (a premultiplied average) and boxes only the alpha plane.
fn downsample_box(level: &Rgba8MipLevel, mode: MipMode) -> Rgba8MipLevel {
    let (width, height) = (level.width, level.height);
    let (next_w, next_h) = ((width / 2).max(1), (height / 2).max(1));
    let texel = |x: u32, y: u32, c: u32| -> u32 {
        let x = x.min(width - 1);
        let y = y.min(height - 1);
        level.rgba[((y * width + x) * 4 + c) as usize] as u32
    };
    let mut out = Vec::with_capacity((next_w * next_h * 4) as usize);
    for y in 0..next_h {
        for x in 0..next_w {
            let corners =
                [(x * 2, y * 2), (x * 2 + 1, y * 2), (x * 2, y * 2 + 1), (x * 2 + 1, y * 2 + 1)];
            let alpha_sum: u32 = corners.iter().map(|&(cx, cy)| texel(cx, cy, 3)).sum();
            for c in 0..3 {
                let value = match mode {
                    MipMode::Box => {
                        let sum: u32 = corners.iter().map(|&(cx, cy)| texel(cx, cy, c)).sum();
                        (sum + 2) / 4
                    }
                    MipMode::AlphaCoveragePreserving => {
                        let weighted: u32 = corners
                            .iter()
                            .map(|&(cx, cy)| texel(cx, cy, c) * texel(cx, cy, 3))
                            .sum();
                        // A fully transparent block has no alpha to weight by — fall back to
                        // the plain mean (its color is invisible either way).
                        (weighted + alpha_sum / 2).checked_div(alpha_sum).unwrap_or_else(|| {
                            let sum: u32 = corners.iter().map(|&(cx, cy)| texel(cx, cy, c)).sum();
                            (sum + 2) / 4
                        })
                    }
                };
                out.push(value as u8);
            }
            out.push(((alpha_sum + 2) / 4) as u8);
        }
    }
    Rgba8MipLevel::new(next_w, next_h, out)
}

/// Rescale a level's alpha plane so its cutout coverage matches the base level's (Castano's
/// alpha-test mipmapping): find the threshold that WOULD reproduce the base coverage on this
/// level, then scale every alpha by `ALPHA_CUTOUT / threshold` so that area lands on the real
/// cutout. A fully empty or fully solid plane passes through untouched.
fn preserve_cutout_coverage(rgba: &mut [u8], base_coverage: f32) {
    let texels = rgba.len() / 4;
    if texels == 0 || base_coverage <= 0.0 {
        return;
    }
    let mut alphas: Vec<u8> = rgba.chunks_exact(4).map(|texel| texel[3]).collect();
    alphas.sort_unstable_by(|a, b| b.cmp(a));
    // The alpha value the base coverage reaches down to when this level's texels are ranked
    // brightest-first — the threshold Castano's search would find.
    let rank = ((base_coverage * texels as f32).round() as usize).clamp(1, texels);
    let threshold = alphas[rank - 1];
    if threshold == 0 || threshold == ALPHA_CUTOUT {
        return;
    }
    let scale = ALPHA_CUTOUT as f32 / threshold as f32;
    for texel in rgba.chunks_exact_mut(4) {
        // A fully solid texel stays fully solid: a downscale (threshold above the cutout)
        // must never erode saturated interiors — the reserved white slot's no-op contract
        // rides on 255 staying 255 through every level.
        if texel[3] == 255 {
            continue;
        }
        texel[3] = (texel[3] as f32 * scale).round().min(255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_chain_contract_accepts_the_one_pixel_tail() {
        let chain = Rgba8MipChain::new(
            vec![Rgba8MipLevel::new(2, 2, vec![255; 16]), Rgba8MipLevel::new(1, 1, vec![255; 4])],
            1,
        );
        assert_eq!(chain.levels().len(), 2);
        assert_eq!(chain.levels()[1].rgba(), &[255; 4]);
    }

    /// The hoist keeps the ground maps' math: one box step is the 2x2 rounded mean, bit-exact
    /// with what `renderer_wgpu`'s private downsampler always produced (its old fixture).
    #[test]
    fn a_box_step_is_the_ground_maps_legacy_average() {
        #[rustfmt::skip]
        let bytes = vec![
            10, 0, 100, 255,   20, 0, 100, 255,
            30, 0, 100, 255,  100, 0, 100, 255,
        ];
        let chain = Rgba8MipChain::build(Rgba8MipLevel::new(2, 2, bytes), MipMode::Box);
        assert_eq!(chain.levels()[1].rgba(), &[40, 0, 100, 255]);
    }

    /// Splat texels are weights summing to 255 across RGBA. The box average is linear, so the
    /// sum survives every level within rounding — the terrain shader's renormalization never
    /// has to fight the chain. (Moved with the hoist; the ground maps ride this promise.)
    #[test]
    fn a_box_step_keeps_splat_weights_normalized() {
        let texels: [[u8; 4]; 4] =
            [[255, 0, 0, 0], [0, 255, 0, 0], [128, 127, 0, 0], [0, 64, 64, 127]];
        let bytes: Vec<u8> = texels.iter().flatten().copied().collect();
        let chain = Rgba8MipChain::build(Rgba8MipLevel::new(2, 2, bytes), MipMode::Box);
        let sum: u32 = chain.levels()[1].rgba().iter().map(|&c| c as u32).sum();
        assert!((253..=257).contains(&sum), "weight sum drifted: {sum}");
    }

    /// `build` walks all the way to the 1x1 tail — square or not — and every level is
    /// sampleable. The full-chain depth is what keeps the far field filtering instead of
    /// aliasing (the ground-map lesson: `mip_level_count: 1` shipped for months).
    #[test]
    fn build_completes_the_chain_to_one_texel_even_off_square() {
        let square =
            Rgba8MipChain::build(Rgba8MipLevel::new(64, 64, vec![7; 64 * 64 * 4]), MipMode::Box);
        assert_eq!(square.levels().len(), 7, "log2(64) + 1 levels");
        assert_eq!(square.max_sampled_level(), 6);
        let wide = Rgba8MipChain::build(Rgba8MipLevel::new(8, 2, vec![7; 8 * 2 * 4]), MipMode::Box);
        let sizes: Vec<(u32, u32)> =
            wide.levels().iter().map(|level| (level.width(), level.height())).collect();
        assert_eq!(sizes, [(8, 2), (4, 1), (2, 1), (1, 1)]);
    }

    /// THE lock the leaf atlas rides (Drzewa 3.0): a sparse cutout mask melts to nothing under
    /// a plain box chain — every step halves the area above the 0.5 threshold until a distant
    /// crown is sticks. The coverage-preserving mode holds the cutout area within ±10% of the
    /// base level at EVERY mip.
    #[test]
    fn coverage_preserving_mips_hold_the_cutout_area() {
        // 16x16 with four soft-rimmed leaf blobs of DIFFERENT radii plus a deterministic
        // per-texel jitter — the shape of a real antialiased mask (a graded rim breaks the
        // alpha ties a synthetic uniform grid would create, which is what the quantile rescale
        // rides on).
        let centers =
            [(3.5f32, 3.5f32, 0.6f32), (11.5, 3.5, 0.9), (3.5, 11.5, 1.2), (11.5, 11.5, 1.5)];
        let mut rgba = vec![0u8; 16 * 16 * 4];
        for y in 0..16u32 {
            for x in 0..16u32 {
                let alpha = centers
                    .iter()
                    .map(|&(cx, cy, solid)| {
                        let d =
                            ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                        ((solid + 0.8 - d) / 0.8 * 255.0).clamp(0.0, 255.0) as u32
                    })
                    .max()
                    .unwrap_or(0) as u8;
                let jitter = ((x * 7 + y * 13) % 5) as u8;
                let i = ((y * 16 + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[0, 200, 0, alpha.saturating_sub(jitter)]);
            }
        }
        let coverage = |level: &Rgba8MipLevel| {
            let texels = level.rgba().chunks_exact(4);
            let total = texels.len() as f32;
            texels.filter(|t| t[3] >= ALPHA_CUTOUT).count() as f32 / total
        };
        let base = Rgba8MipLevel::new(16, 16, rgba);
        let base_coverage = coverage(&base);
        assert!(
            (0.03..=0.25).contains(&base_coverage),
            "fixture sanity: a sparse but present mask, got {base_coverage}"
        );
        let boxed = Rgba8MipChain::build(base.clone(), MipMode::Box);
        assert!(
            coverage(&boxed.levels()[2]) <= base_coverage / 2.0,
            "the fixture must demonstrate the melt: box mip2 keeps {} of a {} base",
            coverage(&boxed.levels()[2]),
            base_coverage
        );
        let held = Rgba8MipChain::build(base, MipMode::AlphaCoveragePreserving);
        for (index, level) in held.levels().iter().enumerate() {
            // The tail's few texels can only quantize the area coarsely; the drift lock
            // applies while the level can still express it at all.
            if level.width() * level.height() < 16 {
                break;
            }
            let drift = coverage(level) - base_coverage;
            assert!(
                drift.abs() <= 0.10,
                "mip {index} cutout coverage drifted by {drift:+.3} from the base {base_coverage:.3}"
            );
        }
    }

    /// The premultiplied half of the cutout filter: a transparent texel's color is meaningless
    /// and must not bleed into the rim. Box turns a white leaf on empty space grey; the
    /// coverage mode keeps the rim the leaf's own color.
    #[test]
    fn transparent_texels_do_not_bleed_color_into_the_cutout_rim() {
        // One opaque white texel among three transparent BLACK ones.
        #[rustfmt::skip]
        let bytes = vec![
            255, 255, 255, 255,   0, 0, 0, 0,
            0, 0, 0, 0,           0, 0, 0, 0,
        ];
        let boxed = Rgba8MipChain::build(Rgba8MipLevel::new(2, 2, bytes.clone()), MipMode::Box);
        assert_eq!(boxed.levels()[1].rgba()[0], 64, "box drags the rim toward black");
        let held =
            Rgba8MipChain::build(Rgba8MipLevel::new(2, 2, bytes), MipMode::AlphaCoveragePreserving);
        let rim = held.levels()[1].rgba();
        assert_eq!(&rim[0..3], &[255, 255, 255], "the rim keeps the leaf's color");
        assert!(rim[3] >= ALPHA_CUTOUT, "a quarter-covered base keeps its one-texel tail lit");
    }
}
