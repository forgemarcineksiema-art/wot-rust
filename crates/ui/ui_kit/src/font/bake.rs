//! One-time atlas baking (interface program F3): every face of the pair, every code point of
//! the charset and every icon, rasterised once and turned into a SIGNED DISTANCE FIELD in one
//! R8 atlas.
//!
//! A distance field instead of coverage because the interface no longer has one size: the same
//! glyph is set at 11 u in a caption and 40 u in a banner, on a 720p laptop and a 4K monitor,
//! and a coverage bitmap is crisp at exactly one of those. The field is thresholded in the HUD
//! shader with an anti-aliasing width taken from screen derivatives, so every size gets an edge
//! one pixel wide — and the same field gives the outline and the glow the lamp wants later.
//!
//! The bake is deterministic: the rasteriser is `ab_glyph`'s, the transform is integer, and
//! nothing reads the clock or the machine.

use std::collections::HashMap;

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};

use super::manifest::{FONT_FILES, Style};
use super::{BakedFace, FontAtlas, Glyph};
use crate::icons::{HudIcon, ICON_PX, raster as raster_icon};

/// Em size every face is rasterised at. The field is sampled at other sizes; this is only the
/// resolution its edges were measured at.
pub(super) const RASTER_PX: f32 = 48.0;
/// How far the field reaches outside (and inside) the outline, in bake pixels. Half of the
/// atlas value's range maps to this many pixels of distance.
pub(super) const SPREAD_PX: u32 = 6;
const ATLAS_WIDTH: u32 = 2048;
const CELL_PADDING: u32 = 1;

/// The code points every face bakes: printable ASCII, the Latin-1 supplement, Latin Extended-A.
pub(super) const CHARSET_RANGES: [(u32, u32); 3] = [(0x20, 0x7E), (0xA0, 0xFF), (0x100, 0x17F)];

/// Every code point of the charset, in order.
pub(super) fn charset() -> impl Iterator<Item = char> {
    CHARSET_RANGES.iter().flat_map(|&(lo, hi)| (lo..=hi).filter_map(char::from_u32))
}

/// One face's cells before packing: its glyph cells by character, its tofu cell, its ascent.
struct FaceCells {
    glyphs: Vec<(char, usize)>,
    tofu: usize,
    ascent: f32,
}

/// One raster pass result before packing: the field plus its metrics.
struct Cell {
    field: Vec<u8>,
    width: u32,
    height: u32,
    advance: f32,
    bearing_x: f32,
    bearing_y: f32,
}

pub(super) fn bake() -> FontAtlas {
    let scale = PxScale::from(RASTER_PX);
    // Every cell of every face, then the icons, in one list so one packer places them all.
    let mut cells: Vec<Cell> = Vec::new();
    let mut face_glyphs: Vec<FaceCells> = Vec::new();
    for file in &FONT_FILES {
        let font = FontRef::try_from_slice(file.bytes)
            .unwrap_or_else(|_| panic!("{} is a valid TrueType file", file.file));
        let scaled = font.as_scaled(scale);
        let mut glyphs = Vec::new();
        for ch in charset() {
            let id = font.glyph_id(ch);
            if id.0 == 0 {
                // No glyph: the tofu cell answers for it at layout time.
                continue;
            }
            cells.push(raster_cell(&font, scale, id));
            glyphs.push((ch, cells.len() - 1));
        }
        // The tofu: the face's own `.notdef` box, or a drawn box when the face has none.
        let notdef = raster_cell(&font, scale, GlyphId(0));
        let tofu = if notdef.width > 0 && notdef.height > 0 { notdef } else { drawn_tofu(&scaled) };
        cells.push(tofu);
        face_glyphs.push(FaceCells { glyphs, tofu: cells.len() - 1, ascent: scaled.ascent() });
    }
    let icon_cells: Vec<(HudIcon, usize)> = HudIcon::ALL
        .into_iter()
        .map(|icon| {
            cells.push(icon_cell(&raster_icon(icon)));
            (icon, cells.len() - 1)
        })
        .collect();

    // Shelf-pack every cell left to right, wrapping to a new row when the shelf fills.
    let mut cx = CELL_PADDING;
    let mut cy = CELL_PADDING;
    let mut shelf = 0u32;
    let placed: Vec<(u32, u32)> =
        cells.iter().map(|c| place(c.width, c.height, &mut cx, &mut cy, &mut shelf)).collect();
    let atlas_height = cy + shelf + CELL_PADDING;

    let mut coverage = vec![0u8; (ATLAS_WIDTH * atlas_height) as usize];
    for (cell, &(x, y)) in cells.iter().zip(&placed) {
        blit(&mut coverage, x, y, &cell.field, cell.width, cell.height);
    }
    let glyph_of = |index: usize| {
        let cell = &cells[index];
        let (x, y) = placed[index];
        Glyph {
            advance_px: cell.advance,
            atlas_x: x,
            atlas_y: y,
            width_px: cell.width,
            height_px: cell.height,
            bearing_x_px: cell.bearing_x,
            bearing_y_px: cell.bearing_y,
        }
    };

    let faces: Vec<BakedFace> = face_glyphs
        .iter()
        .map(|face| BakedFace {
            glyphs: face.glyphs.iter().map(|&(ch, index)| (ch, glyph_of(index))).collect(),
            tofu: glyph_of(face.tofu),
            ascent_px: face.ascent,
        })
        .collect();
    let icons: HashMap<HudIcon, Glyph> =
        icon_cells.iter().map(|&(icon, index)| (icon, glyph_of(index))).collect();
    debug_assert_eq!(faces.len(), Style::ALL.len());

    FontAtlas {
        width: ATLAS_WIDTH,
        height: atlas_height,
        coverage,
        faces,
        icons,
        raster_px: RASTER_PX,
    }
}

/// Reserve a `w`x`h` slot on the current shelf, wrapping to a new row when it would overflow.
fn place(w: u32, h: u32, cx: &mut u32, cy: &mut u32, shelf: &mut u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (0, 0);
    }
    if *cx + w + CELL_PADDING > ATLAS_WIDTH {
        *cx = CELL_PADDING;
        *cy += *shelf + CELL_PADDING;
        *shelf = 0;
    }
    let placement = (*cx, *cy);
    *shelf = (*shelf).max(h);
    *cx += w + CELL_PADDING;
    placement
}

/// Copy a `w`x`h` block into the atlas at `(x, y)`.
fn blit(dst: &mut [u8], x: u32, y: u32, src: &[u8], w: u32, h: u32) {
    for row in 0..h {
        let s = (row * w) as usize;
        let d = ((y + row) * ATLAS_WIDTH + x) as usize;
        dst[d..d + w as usize].copy_from_slice(&src[s..s + w as usize]);
    }
}

/// Rasterise one glyph into coverage, pad it by the spread, and turn it into a distance field.
fn raster_cell(font: &FontRef<'_>, scale: PxScale, id: GlyphId) -> Cell {
    let advance = font.as_scaled(scale).h_advance(id);
    let Some(outlined) = font.outline_glyph(id.with_scale(scale)) else {
        return Cell {
            field: Vec::new(),
            width: 0,
            height: 0,
            advance,
            bearing_x: 0.0,
            bearing_y: 0.0,
        };
    };
    let bounds = outlined.px_bounds();
    let inner_w = (bounds.max.x - bounds.min.x).ceil().max(0.0) as u32;
    let inner_h = (bounds.max.y - bounds.min.y).ceil().max(0.0) as u32;
    if inner_w == 0 || inner_h == 0 {
        return Cell {
            field: Vec::new(),
            width: 0,
            height: 0,
            advance,
            bearing_x: 0.0,
            bearing_y: 0.0,
        };
    }
    let width = inner_w + 2 * SPREAD_PX;
    let height = inner_h + 2 * SPREAD_PX;
    let mut coverage = vec![0u8; (width * height) as usize];
    outlined.draw(|x, y, c| {
        if x < inner_w && y < inner_h {
            let at = ((y + SPREAD_PX) * width + x + SPREAD_PX) as usize;
            coverage[at] = (c * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    });
    Cell {
        field: signed_distance_field(&coverage, width, height),
        width,
        height,
        advance,
        bearing_x: bounds.min.x - SPREAD_PX as f32,
        bearing_y: bounds.min.y - SPREAD_PX as f32,
    }
}

/// A box for a face that has no `.notdef` outline: half an em wide, seven tenths tall, a
/// three-pixel stroke — visibly a missing glyph, never a gap.
fn drawn_tofu(scaled: &ab_glyph::PxScaleFont<&FontRef<'_>>) -> Cell {
    let inner_w = (RASTER_PX * 0.5) as u32;
    let inner_h = (RASTER_PX * 0.7) as u32;
    let width = inner_w + 2 * SPREAD_PX;
    let height = inner_h + 2 * SPREAD_PX;
    let mut coverage = vec![0u8; (width * height) as usize];
    for y in 0..inner_h {
        for x in 0..inner_w {
            let on_stroke = x < 3 || y < 3 || x + 3 >= inner_w || y + 3 >= inner_h;
            if on_stroke {
                coverage[((y + SPREAD_PX) * width + x + SPREAD_PX) as usize] = 255;
            }
        }
    }
    Cell {
        field: signed_distance_field(&coverage, width, height),
        width,
        height,
        advance: inner_w as f32 + RASTER_PX * 0.08,
        bearing_x: RASTER_PX * 0.04 - SPREAD_PX as f32,
        bearing_y: -scaled.ascent() * 0.72 - SPREAD_PX as f32,
    }
}

/// An icon mask (hard-edged, `ICON_PX` square) padded and fielded like a glyph, so icons scale
/// and anti-alias the way text does.
fn icon_cell(mask: &[u8]) -> Cell {
    let width = ICON_PX + 2 * SPREAD_PX;
    let height = ICON_PX + 2 * SPREAD_PX;
    let mut coverage = vec![0u8; (width * height) as usize];
    for y in 0..ICON_PX {
        for x in 0..ICON_PX {
            coverage[((y + SPREAD_PX) * width + x + SPREAD_PX) as usize] =
                mask[(y * ICON_PX + x) as usize];
        }
    }
    Cell {
        field: signed_distance_field(&coverage, width, height),
        width,
        height,
        advance: 0.0,
        bearing_x: -(SPREAD_PX as f32),
        bearing_y: -(SPREAD_PX as f32),
    }
}

/// Coverage to a signed distance field, encoded so that one half is the outline, above it is
/// inside, and the spread maps to the byte's range. The distance comes from an exact Euclidean
/// transform of the binarised coverage (8SSEDT, two passes); on the boundary pixels themselves
/// the anti-aliased coverage is the better estimate of the sub-pixel distance and replaces it.
pub(super) fn signed_distance_field(coverage: &[u8], width: u32, height: u32) -> Vec<u8> {
    let inside: Vec<bool> = coverage.iter().map(|&c| c >= 128).collect();
    let outside: Vec<bool> = inside.iter().map(|&i| !i).collect();
    let to_inside = euclidean_distance(&inside, width, height);
    let to_outside = euclidean_distance(&outside, width, height);
    let spread = SPREAD_PX as f32;
    coverage
        .iter()
        .enumerate()
        .map(|(at, &c)| {
            let signed = if c > 0 && c < 255 {
                0.5 - f32::from(c) / 255.0
            } else if inside[at] {
                -to_outside[at]
            } else {
                to_inside[at]
            };
            let value = 0.5 - signed / (2.0 * spread);
            (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        })
        .collect()
}

/// Distance from every pixel to the nearest `true` pixel (zero on the `true` pixels), by the
/// eight-neighbour sequential Euclidean distance transform.
fn euclidean_distance(mask: &[bool], width: u32, height: u32) -> Vec<f32> {
    const FAR: i32 = 1 << 14;
    let (w, h) = (width as i32, height as i32);
    let mut grid: Vec<(i32, i32)> =
        mask.iter().map(|&m| if m { (0, 0) } else { (FAR, FAR) }).collect();
    let d2 = |p: (i32, i32)| i64::from(p.0) * i64::from(p.0) + i64::from(p.1) * i64::from(p.1);
    let compare = |grid: &mut Vec<(i32, i32)>, x: i32, y: i32, ox: i32, oy: i32| {
        let (nx, ny) = (x + ox, y + oy);
        if nx < 0 || ny < 0 || nx >= w || ny >= h {
            return;
        }
        let mut other = grid[(ny * w + nx) as usize];
        other.0 += ox;
        other.1 += oy;
        let here = (y * w + x) as usize;
        if d2(other) < d2(grid[here]) {
            grid[here] = other;
        }
    };
    for y in 0..h {
        for x in 0..w {
            compare(&mut grid, x, y, -1, 0);
            compare(&mut grid, x, y, 0, -1);
            compare(&mut grid, x, y, -1, -1);
            compare(&mut grid, x, y, 1, -1);
        }
        for x in (0..w).rev() {
            compare(&mut grid, x, y, 1, 0);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            compare(&mut grid, x, y, 1, 0);
            compare(&mut grid, x, y, 0, 1);
            compare(&mut grid, x, y, -1, 1);
            compare(&mut grid, x, y, 1, 1);
        }
        for x in 0..w {
            compare(&mut grid, x, y, -1, 0);
        }
    }
    grid.iter().map(|&p| (d2(p) as f32).sqrt()).collect()
}
