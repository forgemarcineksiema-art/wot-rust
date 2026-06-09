//! One-time atlas baking: rasterize the embedded font into a coverage image and a glyph table.

use std::collections::HashMap;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

use super::{FontAtlas, Glyph};

/// Embedded font (SIL OFL 1.1, see `assets/fonts/OFL.txt`). Rajdhani's squarish, technical letters
/// read well at small sizes and suit a gunnery HUD without clashing with the flat-shaded scene.
const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/Rajdhani-SemiBold.ttf"
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

    // Shelf-pack the bitmaps left-to-right, wrapping to a new row when the shelf fills.
    let mut cursor_x = GLYPH_PADDING;
    let mut cursor_y = GLYPH_PADDING;
    let mut shelf_height = 0u32;
    let mut placements = Vec::with_capacity(rasters.len());
    for raster in &rasters {
        if raster.width == 0 || raster.height == 0 {
            placements.push((0u32, 0u32));
            continue;
        }
        if cursor_x + raster.width + GLYPH_PADDING > ATLAS_WIDTH {
            cursor_x = GLYPH_PADDING;
            cursor_y += shelf_height + GLYPH_PADDING;
            shelf_height = 0;
        }
        placements.push((cursor_x, cursor_y));
        shelf_height = shelf_height.max(raster.height);
        cursor_x += raster.width + GLYPH_PADDING;
    }
    let atlas_height = cursor_y + shelf_height + GLYPH_PADDING;

    let mut coverage = vec![0u8; (ATLAS_WIDTH * atlas_height) as usize];
    let mut glyphs = HashMap::with_capacity(rasters.len());
    for (raster, &(x, y)) in rasters.iter().zip(&placements) {
        for row in 0..raster.height {
            let src = (row * raster.width) as usize;
            let dst = ((y + row) * ATLAS_WIDTH + x) as usize;
            coverage[dst..dst + raster.width as usize]
                .copy_from_slice(&raster.coverage[src..src + raster.width as usize]);
        }
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

    FontAtlas {
        width: ATLAS_WIDTH,
        height: atlas_height,
        coverage,
        glyphs,
        raster_px: RASTER_PX,
        ascent_px: scaled.ascent(),
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
