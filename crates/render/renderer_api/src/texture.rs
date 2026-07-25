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
}
