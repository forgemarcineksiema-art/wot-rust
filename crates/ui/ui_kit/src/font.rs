//! Real glyph text for the HUD (interface program F3): the pair of OFL faces in
//! [`manifest`] baked ONCE into one signed-distance atlas, then every run of text drawn as
//! textured `HudVertex` quads sampling that atlas in the single HUD draw call.
//!
//! The atlas is a distance field rather than coverage so the same glyph is crisp at a caption's
//! 11 u and a banner's 40 u, on a laptop and a 4K monitor: the HUD shader thresholds it with an
//! anti-aliasing width from screen derivatives. Solid bars and crosshairs keep the `uv` sentinel
//! and ignore the texture; glyph and icon quads carry real `uv`s. The renderer uploads `atlas()`
//! once at startup.
//!
//! [`bake`] builds the atlas; [`layout`] turns strings into glyph quads; [`manifest`] names the
//! fonts, their licences and their hashes.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::icons::HudIcon;

mod bake;
pub mod layout;
pub mod manifest;

pub use layout::{
    push_icon, push_text, push_text_right, push_text_right_styled, push_text_styled, text_width,
    text_width_styled,
};
pub use manifest::{FONT_FILES, Face, FontFile, POLISH_LETTERS, Style, Weight};

/// One baked cell: its slot in the atlas (pixels) plus the metrics needed to lay it out on a
/// line. The cell includes the field's spread on every side; the bearings account for it.
#[derive(Debug, Clone, Copy)]
struct Glyph {
    advance_px: f32,
    atlas_x: u32,
    atlas_y: u32,
    width_px: u32,
    height_px: u32,
    /// Offset from the pen origin to the cell's top-left, in pixels (y down).
    bearing_x_px: f32,
    bearing_y_px: f32,
}

/// One face at one weight, baked: its glyph table, its tofu and its line metrics.
struct BakedFace {
    glyphs: HashMap<char, Glyph>,
    /// The cell an unknown code point draws — visibly a box, never a gap.
    tofu: Glyph,
    ascent_px: f32,
}

/// The baked pair: a distance-field atlas (`R8`, row-major) plus a glyph table per style.
pub struct FontAtlas {
    width: u32,
    height: u32,
    coverage: Vec<u8>,
    /// One entry per [`Style::ALL`] element, in that order.
    faces: Vec<BakedFace>,
    /// Icon fields baked into the same atlas; values reuse [`Glyph`] for the atlas rect only.
    icons: HashMap<HudIcon, Glyph>,
    raster_px: f32,
}

impl FontAtlas {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The atlas texels: a signed distance field, one half on the outline, above it inside.
    pub fn coverage(&self) -> &[u8] {
        &self.coverage
    }

    /// Whether `style` has a glyph of its own for `ch` (a tofu box is not "covered").
    pub fn covers(&self, style: Style, ch: char) -> bool {
        self.face(style).glyphs.contains_key(&ch)
    }

    /// The em size the faces were baked at.
    pub fn raster_px(&self) -> f32 {
        self.raster_px
    }

    fn face(&self, style: Style) -> &BakedFace {
        &self.faces[style.index()]
    }
}

/// The process-wide baked atlas (built on first use).
pub fn atlas() -> &'static FontAtlas {
    static ATLAS: OnceLock<FontAtlas> = OnceLock::new();
    ATLAS.get_or_init(bake::bake)
}

/// Public view of the baked HUD atlas for renderer upload: `(width, height, R8 field)`.
/// Used by the windowed client and the offscreen probes to feed `set_hud_font_atlas`.
pub fn hud_font_atlas() -> (u32, u32, &'static [u8]) {
    let font = atlas();
    (font.width, font.height, &font.coverage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use manifest::{LATIN_EXTENDED_A_EXEMPT, POLISH_LETTERS};

    /// Every face bakes printable ASCII, every Polish letter, and Latin Extended-A but for the
    /// four code points no string of ours sets. A gap here is a gap on the screen.
    #[test]
    fn the_pair_covers_latin_extended_a() {
        let font = atlas();
        let mut examined = 0usize;
        for style in Style::ALL {
            for code in 0x20u32..=0x7E {
                let ch = char::from_u32(code).expect("ascii");
                assert!(font.covers(style, ch), "{style:?} lacks ASCII {ch:?}");
                examined += 1;
            }
            for ch in POLISH_LETTERS.chars() {
                assert!(font.covers(style, ch), "{style:?} lacks the Polish letter {ch:?}");
                examined += 1;
            }
            for code in 0x100u32..=0x17F {
                let ch = char::from_u32(code).expect("latin extended-a");
                if LATIN_EXTENDED_A_EXEMPT.contains(&ch) {
                    continue;
                }
                assert!(font.covers(style, ch), "{style:?} lacks U+{code:04X} {ch:?}");
                examined += 1;
            }
        }
        assert!(examined > 5 * 200, "the lock examined {examined} code points — too few");
    }

    /// The atlas is a distance field: inside a stroke the value sits above one half, at the
    /// cell's padded rim it sits below, and the further inside the stroke the higher it climbs
    /// — a black M's stem carries a deeper interior than a regular I's.
    #[test]
    fn the_atlas_is_a_signed_distance_field() {
        let font = atlas();
        let probe = |style: Style, ch: char| {
            let glyph = font.face(style).glyphs.get(&ch).expect("a probe glyph");
            let mut max = 0u8;
            let mut rim_max = 0u8;
            for y in 0..glyph.height_px {
                for x in 0..glyph.width_px {
                    let at = ((glyph.atlas_y + y) * font.width + glyph.atlas_x + x) as usize;
                    let v = font.coverage[at];
                    max = max.max(v);
                    let on_rim =
                        x == 0 || y == 0 || x + 1 == glyph.width_px || y + 1 == glyph.height_px;
                    if on_rim {
                        rim_max = rim_max.max(v);
                    }
                }
            }
            (max, rim_max)
        };
        let (thin, thin_rim) = probe(Style::VALUE, 'I');
        let (bold, bold_rim) = probe(Style::BANNER, 'M');
        assert!(thin > 140, "a regular stem must read above one half inside, got {thin}");
        assert!(bold > 185, "a black stem must read deep inside, got {bold}");
        assert!(bold > thin, "the black stem is deeper than the regular one");
        assert!(
            thin_rim < 128 && bold_rim < 128,
            "the padded rim reads outside: {thin_rim}, {bold_rim}"
        );
    }

    #[test]
    fn every_style_has_its_own_face_and_a_tofu() {
        let font = atlas();
        assert_eq!(font.faces.len(), Style::ALL.len());
        for style in Style::ALL {
            let face = font.face(style);
            assert!(face.tofu.width_px > 0 && face.tofu.height_px > 0, "{style:?} has no tofu");
            assert!(face.ascent_px > 0.0);
            assert!(!face.glyphs.contains_key(&'\u{4E2D}'), "{style:?} claims a CJK glyph");
        }
        assert_eq!(font.coverage.len(), (font.width * font.height) as usize);
        assert!(font.icons.len() == HudIcon::ALL.len(), "every icon is in the atlas");
    }
}
