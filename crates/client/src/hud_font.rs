//! Real glyph text for the HUD, baked the way BigWorld did it: rasterize an embedded TrueType
//! font into a single-channel coverage atlas once, then draw each glyph as a textured `HudVertex`
//! quad sampling that atlas. This replaces the old seven-segment digits with smooth, anti-aliased
//! letters and numbers that fit the soft low-poly look.
//!
//! The atlas rides the same single HUD draw call: solid bars/crosshairs keep the `uv` sentinel and
//! ignore the texture, glyph quads carry real `uv`s. The renderer uploads `atlas()` once at startup.
//!
//! [`bake`] builds the atlas; [`layout`] turns strings into glyph quads.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::hud_icons::HudIcon;

mod bake;
mod layout;

pub(crate) use layout::{push_icon, push_text, push_text_right, text_width};

/// One baked glyph: its slot in the atlas (pixels) plus the metrics needed to lay it out on a line.
#[derive(Debug, Clone, Copy)]
struct Glyph {
    advance_px: f32,
    atlas_x: u32,
    atlas_y: u32,
    width_px: u32,
    height_px: u32,
    /// Offset from the pen origin to the glyph bitmap's top-left, in pixels (y down).
    bearing_x_px: f32,
    bearing_y_px: f32,
}

/// The baked font: a coverage atlas (`R8`, row-major) plus a glyph table and line metrics.
pub(crate) struct FontAtlas {
    width: u32,
    height: u32,
    coverage: Vec<u8>,
    glyphs: HashMap<char, Glyph>,
    /// Icon masks baked into the same atlas; values reuse [`Glyph`] for the atlas rect only.
    icons: HashMap<HudIcon, Glyph>,
    raster_px: f32,
    ascent_px: f32,
}

impl FontAtlas {
    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn coverage(&self) -> &[u8] {
        &self.coverage
    }
}

/// The process-wide baked atlas (built on first use).
pub(crate) fn atlas() -> &'static FontAtlas {
    static ATLAS: OnceLock<FontAtlas> = OnceLock::new();
    ATLAS.get_or_init(bake::bake)
}

/// Public view of the baked HUD glyph atlas for renderer upload: `(width, height, R8 coverage)`.
/// Used by the windowed client and the offscreen screenshot example to feed `set_hud_font_atlas`.
pub fn hud_font_atlas() -> (u32, u32, &'static [u8]) {
    let font = atlas();
    (font.width, font.height, &font.coverage)
}
