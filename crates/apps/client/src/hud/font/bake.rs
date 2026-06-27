//! One-time atlas baking: rasterize the embedded font into a coverage image and a glyph table.

use std::collections::HashMap;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

use super::{FontAtlas, Glyph};
use crate::hud::icons::{HudIcon, ICON_PX, raster as raster_icon};

/// Embedded font (SIL OFL 1.1, see `assets/fonts/OFL.txt`). Rajdhani's squarish, technical letters
/// read well at small sizes and suit a gunnery HUD without clashing with the flat-shaded scene.
const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/Rajdhani-SemiBold.ttf"
));

/// Pixel size each glyph is rasterized at. Generous vs the on-screen size so the linear-filtered
/// downscale stays crisp; the atlas is tiny regardless (one shelf-packed image, built once).
const RASTER_PX: f32 = 64.0;
const ATLAS_WIDTH: u32 = 512;
const GLYPH_PADDING: u32 = 1;

/// One raster pass result before packing: coverage bitmap plus metrics.
struct Raster {
    ch: char,
    coverage: Vec<u8>,
    width: u32,
    height: u32,
    advance: f32,
    bearing_x: f32,
    bearing_y: f32,
}

pub(super) fn bake() -> FontAtlas {
    let font = FontRef::try_from_slice(FONT_BYTES).expect("embedded Rajdhani TTF is valid");
    let scale = PxScale::from(RASTER_PX);
    let scaled = font.as_scaled(scale);

    // Printable ASCII covers digits, both letter cases, and the punctuation the HUD needs.
    let rasters: Vec<Raster> =
        (0x20u8..=0x7E).map(|code| raster_glyph(&font, scale, code)).collect();

    // Icons share the atlas (one coverage image, one texture upload) — baked like glyphs.
    let icon_masks: Vec<(HudIcon, Vec<u8>)> =
        HudIcon::ALL.into_iter().map(|icon| (icon, raster_icon(icon))).collect();

    // Shelf-pack glyphs, then icons, left-to-right, wrapping to a new row when the shelf fills.
    let mut cx = GLYPH_PADDING;
    let mut cy = GLYPH_PADDING;
    let mut shelf = 0u32;
    let glyph_at: Vec<(u32, u32)> =
        rasters.iter().map(|r| place(r.width, r.height, &mut cx, &mut cy, &mut shelf)).collect();
    let icon_at: Vec<(u32, u32)> =
        icon_masks.iter().map(|_| place(ICON_PX, ICON_PX, &mut cx, &mut cy, &mut shelf)).collect();
    let atlas_height = cy + shelf + GLYPH_PADDING;

    let mut coverage = vec![0u8; (ATLAS_WIDTH * atlas_height) as usize];
    let mut glyphs = HashMap::with_capacity(rasters.len());
    for (raster, &(x, y)) in rasters.iter().zip(&glyph_at) {
        blit(&mut coverage, x, y, &raster.coverage, raster.width, raster.height);
        glyphs.insert(
            raster.ch,
            Glyph {
                advance_px: raster.advance,
                atlas_x: x,
                atlas_y: y,
                width_px: raster.width,
                height_px: raster.height,
                bearing_x_px: raster.bearing_x,
                bearing_y_px: raster.bearing_y,
            },
        );
    }
    let mut icons = HashMap::with_capacity(icon_masks.len());
    for ((icon, mask), &(x, y)) in icon_masks.iter().zip(&icon_at) {
        blit(&mut coverage, x, y, mask, ICON_PX, ICON_PX);
        icons.insert(
            *icon,
            Glyph {
                advance_px: 0.0,
                atlas_x: x,
                atlas_y: y,
                width_px: ICON_PX,
                height_px: ICON_PX,
                bearing_x_px: 0.0,
                bearing_y_px: 0.0,
            },
        );
    }

    FontAtlas {
        width: ATLAS_WIDTH,
        height: atlas_height,
        coverage,
        glyphs,
        icons,
        raster_px: RASTER_PX,
        ascent_px: scaled.ascent(),
    }
}

/// Reserve a `w`x`h` slot on the current shelf, wrapping to a new row when it would overflow.
fn place(w: u32, h: u32, cx: &mut u32, cy: &mut u32, shelf: &mut u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (0, 0);
    }
    if *cx + w + GLYPH_PADDING > ATLAS_WIDTH {
        *cx = GLYPH_PADDING;
        *cy += *shelf + GLYPH_PADDING;
        *shelf = 0;
    }
    let placement = (*cx, *cy);
    *shelf = (*shelf).max(h);
    *cx += w + GLYPH_PADDING;
    placement
}

/// Copy a `w`x`h` coverage block into the atlas at `(x, y)`.
fn blit(dst: &mut [u8], x: u32, y: u32, src: &[u8], w: u32, h: u32) {
    for row in 0..h {
        let s = (row * w) as usize;
        let d = ((y + row) * ATLAS_WIDTH + x) as usize;
        dst[d..d + w as usize].copy_from_slice(&src[s..s + w as usize]);
    }
}

/// Rasterize one ASCII code point into a coverage bitmap plus its layout metrics.
fn raster_glyph(font: &FontRef<'_>, scale: PxScale, code: u8) -> Raster {
    let ch = code as char;
    let id = font.glyph_id(ch);
    let advance = font.as_scaled(scale).h_advance(id);
    let mut raster = Raster {
        ch,
        coverage: Vec::new(),
        width: 0,
        height: 0,
        advance,
        bearing_x: 0.0,
        bearing_y: 0.0,
    };
    if let Some(outlined) = font.outline_glyph(id.with_scale(scale)) {
        let bounds = outlined.px_bounds();
        let width = (bounds.max.x - bounds.min.x).ceil().max(0.0) as u32;
        let height = (bounds.max.y - bounds.min.y).ceil().max(0.0) as u32;
        let mut coverage = vec![0u8; (width * height) as usize];
        outlined.draw(|x, y, c| {
            if x < width && y < height {
                coverage[(y * width + x) as usize] = (c * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        });
        raster.coverage = coverage;
        raster.width = width;
        raster.height = height;
        raster.bearing_x = bounds.min.x;
        raster.bearing_y = bounds.min.y;
    }
    raster
}
