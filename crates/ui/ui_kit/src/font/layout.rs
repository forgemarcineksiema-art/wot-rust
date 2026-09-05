//! Turn strings into textured `HudVertex` glyph quads using the baked [`super::atlas`].
//!
//! Every function comes in two forms: the styled one names the [`Style`] the text is set in;
//! the legacy one (the name without `_styled`) sets it in [`Style::DEFAULT`] and exists so the
//! seventy call sites of the old kit keep compiling until the H and G waves restyle them one by
//! one. An unknown code point draws the face's tofu box and advances — it never skips.

use renderer_api::HudVertex;

use super::atlas;
use super::manifest::Style;
use crate::icons::HudIcon;

/// Clip-space advance width of `text` set in [`Style::DEFAULT`] at em-height `height`.
pub fn text_width(text: &str, height: f32, aspect: f32) -> f32 {
    text_width_styled(Style::DEFAULT, text, height, aspect)
}

/// Clip-space advance width of `text` set in `style` at em-height `height`. `aspect` squishes x
/// so glyphs stay square on wide viewports — matching `push_text_styled`.
pub fn text_width_styled(style: Style, text: &str, height: f32, aspect: f32) -> f32 {
    let font = atlas();
    let face = font.face(style);
    let clip_per_px = height / font.raster_px;
    let advance: f32 = text
        .chars()
        .map(|ch| face.glyphs.get(&ch).unwrap_or(&face.tofu).advance_px * clip_per_px)
        .sum();
    advance / aspect.max(0.01)
}

/// Draw `text` in [`Style::DEFAULT`] — see [`push_text_styled`].
pub fn push_text(
    vertices: &mut Vec<HudVertex>,
    text: &str,
    left_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    push_text_styled(vertices, Style::DEFAULT, text, left_x, top_y, height, aspect, color);
}

/// Draw `text` set in `style`, left-aligned with its top-left box at (`left_x`, `top_y`), em
/// height `height` clip units, tinted `color`. Each visible glyph becomes two textured
/// `HudVertex` quads: a dark drop-shadow copy offset down-right, then the glyph itself — the
/// shadow is what keeps a marking legible when the field behind it is sunlit grass instead of
/// a dark panel.
#[expect(clippy::too_many_arguments)]
pub fn push_text_styled(
    vertices: &mut Vec<HudVertex>,
    style: Style,
    text: &str,
    left_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let shadow = crate::theme::color::TEXT_SHADOW;
    let offset = height * 0.09;
    push_text_pass(
        vertices,
        style,
        text,
        left_x + offset / aspect.max(0.01),
        top_y - offset,
        height,
        aspect,
        [shadow[0], shadow[1], shadow[2], shadow[3] * color[3]],
    );
    push_text_pass(vertices, style, text, left_x, top_y, height, aspect, color);
}

#[expect(clippy::too_many_arguments)]
fn push_text_pass(
    vertices: &mut Vec<HudVertex>,
    style: Style,
    text: &str,
    left_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let font = atlas();
    let face = font.face(style);
    let clip_per_px = height / font.raster_px;
    let x_scale = clip_per_px / aspect.max(0.01);
    // Place the baseline so the ascent line sits at `top_y`; caps then hang just below it.
    let baseline_y = top_y - face.ascent_px * clip_per_px;
    let (atlas_w, atlas_h) = (font.width as f32, font.height as f32);

    let mut pen_x = left_x;
    for ch in text.chars() {
        let glyph = face.glyphs.get(&ch).unwrap_or(&face.tofu);
        if glyph.width_px > 0 && glyph.height_px > 0 {
            let x0 = pen_x + glyph.bearing_x_px * x_scale;
            let x1 = x0 + glyph.width_px as f32 * x_scale;
            let y_top = baseline_y - glyph.bearing_y_px * clip_per_px;
            let y_bottom = y_top - glyph.height_px as f32 * clip_per_px;
            let u0 = glyph.atlas_x as f32 / atlas_w;
            let u1 = (glyph.atlas_x + glyph.width_px) as f32 / atlas_w;
            let v0 = glyph.atlas_y as f32 / atlas_h;
            let v1 = (glyph.atlas_y + glyph.height_px) as f32 / atlas_h;
            push_glyph_quad(vertices, x0, x1, y_top, y_bottom, u0, v0, u1, v1, color);
        }
        pen_x += glyph.advance_px * x_scale;
    }
}

/// Draw `text` in [`Style::DEFAULT`], right-aligned so its advance ends at `right_x`.
pub fn push_text_right(
    vertices: &mut Vec<HudVertex>,
    text: &str,
    right_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    push_text_right_styled(vertices, Style::DEFAULT, text, right_x, top_y, height, aspect, color);
}

/// Draw `text` set in `style`, right-aligned so its advance ends at `right_x`.
#[expect(clippy::too_many_arguments)]
pub fn push_text_right_styled(
    vertices: &mut Vec<HudVertex>,
    style: Style,
    text: &str,
    right_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let left_x = right_x - text_width_styled(style, text, height, aspect);
    push_text_styled(vertices, style, text, left_x, top_y, height, aspect, color);
}

/// Draw `icon` as a `size`-tall square (x compressed by `aspect` to stay square), top-left at
/// (`left_x`, `top_y`), tinted `color`. Samples the icon's field baked into the shared atlas;
/// the cell's spread rim is drawn too, so the square is a little larger than the icon's ink,
/// the way a glyph cell is.
pub fn push_icon(
    vertices: &mut Vec<HudVertex>,
    icon: HudIcon,
    left_x: f32,
    top_y: f32,
    size: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let font = atlas();
    let Some(g) = font.icons.get(&icon) else {
        return;
    };
    let (atlas_w, atlas_h) = (font.width as f32, font.height as f32);
    // The icon's ink is `ICON_PX` of the cell; the cell carries the spread on each side.
    let ink = crate::icons::ICON_PX as f32;
    let cell_scale = g.height_px as f32 / ink;
    let rim = (g.height_px as f32 - ink) * 0.5 / ink * size;
    let cell = size * cell_scale;
    let width = cell / aspect.max(0.01);
    let left = left_x - rim / aspect.max(0.01);
    let top = top_y + rim;
    let u0 = g.atlas_x as f32 / atlas_w;
    let u1 = (g.atlas_x + g.width_px) as f32 / atlas_w;
    let v0 = g.atlas_y as f32 / atlas_h;
    let v1 = (g.atlas_y + g.height_px) as f32 / atlas_h;
    push_glyph_quad(vertices, left, left + width, top, top - cell, u0, v0, u1, v1, color);
}

#[expect(clippy::too_many_arguments)]
fn push_glyph_quad(
    vertices: &mut Vec<HudVertex>,
    x0: f32,
    x1: f32,
    y_top: f32,
    y_bottom: f32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    color: [f32; 4],
) {
    // Atlas v grows downward (row 0 = top), so the clip-space top edge maps to v0.
    let top_left = HudVertex::textured([x0, y_top], [u0, v0], color);
    let bottom_left = HudVertex::textured([x0, y_bottom], [u0, v1], color);
    let bottom_right = HudVertex::textured([x1, y_bottom], [u1, v1], color);
    let top_right = HudVertex::textured([x1, y_top], [u1, v0], color);
    for vertex in [top_left, bottom_left, bottom_right, top_left, bottom_right, top_right] {
        vertices.push(vertex);
    }
}

/// The width of the cell every digit occupies when text is set tabular: the widest digit's
/// advance, in clip units at em-height `height`. `ab_glyph` shapes nothing, so tabular figures
/// are a rule of the layout engine, not a feature of the font.
pub fn digit_cell_width(style: Style, height: f32, aspect: f32) -> f32 {
    let font = atlas();
    let face = font.face(style);
    let clip_per_px = height / font.raster_px;
    ('0'..='9').map(|d| face.glyphs.get(&d).unwrap_or(&face.tofu).advance_px).fold(0.0f32, f32::max)
        * clip_per_px
        / aspect.max(0.01)
}

/// Advance width of `text` with every digit in the fixed cell.
pub fn text_width_tabular(style: Style, text: &str, height: f32, aspect: f32) -> f32 {
    let cell = digit_cell_width(style, height, aspect);
    let font = atlas();
    let face = font.face(style);
    let clip_per_px = height / font.raster_px / aspect.max(0.01);
    text.chars()
        .map(|ch| {
            if ch.is_ascii_digit() {
                cell
            } else {
                face.glyphs.get(&ch).unwrap_or(&face.tofu).advance_px * clip_per_px
            }
        })
        .sum()
}

/// Draw `text` set in `style` with tabular digits: every digit centred in one fixed cell, so
/// a counter's columns stand still while its value runs.
#[expect(clippy::too_many_arguments)]
pub fn push_text_tabular(
    vertices: &mut Vec<HudVertex>,
    style: Style,
    text: &str,
    left_x: f32,
    top_y: f32,
    height: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let shadow = crate::theme::color::TEXT_SHADOW;
    let offset = height * 0.09;
    let shadow_color = [shadow[0], shadow[1], shadow[2], shadow[3] * color[3]];
    for (dx, dy, tint) in [(offset / aspect.max(0.01), -offset, shadow_color), (0.0, 0.0, color)] {
        let cell = digit_cell_width(style, height, aspect);
        let font = atlas();
        let face = font.face(style);
        let clip_per_px = height / font.raster_px;
        let x_scale = clip_per_px / aspect.max(0.01);
        let mut pen_x = left_x + dx;
        for ch in text.chars() {
            let glyph = face.glyphs.get(&ch).unwrap_or(&face.tofu);
            if ch.is_ascii_digit() {
                let advance = glyph.advance_px * x_scale;
                let centred = pen_x + (cell - advance) * 0.5;
                push_text_pass(
                    vertices,
                    style,
                    &ch.to_string(),
                    centred,
                    top_y + dy,
                    height,
                    aspect,
                    tint,
                );
                pen_x += cell;
            } else {
                push_text_pass(
                    vertices,
                    style,
                    &ch.to_string(),
                    pen_x,
                    top_y + dy,
                    height,
                    aspect,
                    tint,
                );
                pen_x += glyph.advance_px * x_scale;
            }
        }
    }
}

/// Word-wrap `text` to lines no wider than `max_width` (clip units) at em-height `height`. A
/// single word wider than the line is broken by character; lines never come back empty.
pub fn wrap_text(
    style: Style,
    text: &str,
    height: f32,
    aspect: f32,
    max_width: f32,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate =
                if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width_styled(style, &candidate, height, aspect) <= max_width
                || line.is_empty() && word.chars().count() == 1
            {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // The word alone: fits, or is broken by character.
            if text_width_styled(style, word, height, aspect) <= max_width {
                line = word.to_string();
            } else {
                let mut piece = String::new();
                for ch in word.chars() {
                    let next = format!("{piece}{ch}");
                    if !piece.is_empty()
                        && text_width_styled(style, &next, height, aspect) > max_width
                    {
                        lines.push(std::mem::take(&mut piece));
                    }
                    piece.push(ch);
                }
                line = piece;
            }
        }
        lines.push(line);
    }
    lines
}

/// `text` cut to `max_width` with a trailing `...` when it does not fit: the tail goes, the
/// head stays — a truncated name is still recognisable by its start.
pub fn ellipsize(style: Style, text: &str, height: f32, aspect: f32, max_width: f32) -> String {
    if text_width_styled(style, text, height, aspect) <= max_width {
        return text.to_string();
    }
    const ELLIPSIS: &str = "...";
    let mut kept = String::new();
    for ch in text.chars() {
        let next = format!("{kept}{ch}{ELLIPSIS}");
        if text_width_styled(style, &next, height, aspect) > max_width {
            break;
        }
        kept.push(ch);
    }
    format!("{kept}{ELLIPSIS}")
}

#[cfg(test)]
mod measure_tests {
    use super::*;

    #[test]
    fn tabular_digits_share_one_cell() {
        let aspect = 16.0 / 9.0;
        for style in Style::ALL {
            let ones = text_width_tabular(style, "111", 0.05, aspect);
            let eights = text_width_tabular(style, "888", 0.05, aspect);
            assert!((ones - eights).abs() < 1e-6, "{style:?}: 111 and 888 differ in width");
            assert!(ones >= text_width_styled(style, "111", 0.05, aspect) - 1e-6);
        }
        let mut verts = Vec::new();
        push_text_tabular(&mut verts, Style::VALUE, "1:8", -0.5, 0.5, 0.05, aspect, [1.0; 4]);
        assert_eq!(verts.len(), 3 * 12, "shadow and glyph per character");
    }

    #[test]
    fn wrapped_text_never_exceeds_its_width() {
        let aspect = 16.0 / 9.0;
        let max = 0.30;
        let lines = wrap_text(
            Style::VALUE,
            "the quick brown fox jumps over the lazy dog",
            0.05,
            aspect,
            max,
        );
        assert!(lines.len() > 1, "a long sentence wraps");
        for line in &lines {
            assert!(!line.is_empty());
            assert!(
                text_width_styled(Style::VALUE, line, 0.05, aspect) <= max + 1e-6,
                "{line:?} overflows"
            );
        }
        let joined = lines.join(" ");
        assert_eq!(joined, "the quick brown fox jumps over the lazy dog", "no word lost");
        let broken = wrap_text(Style::VALUE, "supercalifragilistic", 0.05, aspect, 0.10);
        assert!(broken.len() > 1, "an unbreakable word breaks by character");
    }

    #[test]
    fn ellipsis_replaces_the_tail_not_the_head() {
        let aspect = 16.0 / 9.0;
        let full = text_width_styled(Style::VALUE, "Panzerkampfwagen VI Tiger", 0.05, aspect);
        let max = full * 0.5;
        let cut = ellipsize(Style::VALUE, "Panzerkampfwagen VI Tiger", 0.05, aspect, max);
        assert!(cut.starts_with("Panzer"), "the head stays: {cut}");
        assert!(cut.ends_with("..."), "the tail is the ellipsis: {cut}");
        assert!(text_width_styled(Style::VALUE, &cut, 0.05, aspect) <= max + 1e-6);
        assert_eq!(
            ellipsize(Style::VALUE, "T-54", 0.05, aspect, 1.0),
            "T-54",
            "what fits is untouched"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_bakes_digits_letters_and_is_nonempty() {
        let font = atlas();
        assert!(font.width() > 0 && font.height() > 0, "atlas has size");
        assert_eq!(
            font.coverage().len(),
            (font.width() * font.height()) as usize,
            "R8 byte per texel"
        );
        for style in Style::ALL {
            let face = font.face(style);
            for ch in ['0', '9', 'A', 'M', 'P', 'k', 'm', '/', 'Ł', 'ż'] {
                let glyph = face.glyphs.get(&ch).unwrap_or_else(|| panic!("{style:?} lacks {ch}"));
                assert!(glyph.advance_px > 0.0, "{ch} advances the pen");
            }
            // The space glyph advances but carves no bitmap.
            let space = face.glyphs.get(&' ').expect("space glyph");
            assert!(space.advance_px > 0.0 && space.width_px == 0);
        }
        assert!(font.coverage().iter().any(|&c| c > 128), "atlas has painted glyph pixels");
    }

    #[test]
    fn text_width_grows_with_more_glyphs_and_emits_shadowed_quads() {
        let aspect = 16.0 / 9.0;
        assert!(text_width("888", 0.05, aspect) > text_width("8", 0.05, aspect));
        assert_eq!(text_width("", 0.05, aspect), 0.0);

        let mut verts = Vec::new();
        push_text(&mut verts, "12", -0.5, 0.5, 0.05, aspect, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(verts.len(), 24, "two glyphs => shadow quad + glyph quad each");
        assert!(verts.iter().all(|v| v.uv[0] >= 0.0), "glyph verts carry real (non-sentinel) uv");
        // The shadow pass comes first (drawn under), darker and offset down-right.
        let (shadow, main) = verts.split_at(12);
        assert!(shadow.iter().all(|v| v.color[0] < 0.1), "shadow is dark");
        assert!(main.iter().all(|v| v.color[0] == 1.0), "main glyphs keep the caller's color");
        assert!(shadow[0].position[0] > main[0].position[0], "shadow offsets right");
        assert!(shadow[0].position[1] < main[0].position[1], "shadow offsets down");
    }

    /// A fading text's shadow fades with it — the shadow alpha scales by the text alpha, so
    /// a nearly-transparent banner does not leave a dark ghost behind.
    #[test]
    fn text_shadow_alpha_follows_the_text() {
        let mut verts = Vec::new();
        push_text(&mut verts, "1", -0.5, 0.5, 0.05, 16.0 / 9.0, [1.0, 1.0, 1.0, 0.1]);
        let shadow_alpha = verts[0].color[3];
        assert!(shadow_alpha < 0.1, "shadow must fade with its text, got {shadow_alpha}");
    }

    #[test]
    fn right_aligned_text_ends_at_the_anchor() {
        let aspect = 16.0 / 9.0;
        let color = [1.0, 1.0, 1.0, 1.0];
        let mut verts = Vec::new();
        push_text_right(&mut verts, "42", 0.8, 0.0, 0.05, aspect, color);
        // Measure the glyphs themselves; the drop shadow legitimately peeks past by its offset,
        // and every cell carries its spread rim past the ink.
        let rim = 0.05 * super::super::bake::SPREAD_PX as f32 / atlas().raster_px / aspect;
        let max_x = verts
            .iter()
            .filter(|v| v.color == color)
            .map(|v| v.position[0])
            .fold(f32::MIN, f32::max);
        assert!(max_x <= 0.8 + rim + 1.0e-4, "right edge should not pass the anchor, got {max_x}");
        assert!(max_x > 0.7, "text should reach close to the anchor, got {max_x}");
    }

    /// An unknown code point renders the tofu box and advances the pen — a missing glyph is
    /// visible on the screen, never a silent gap that shortens the word.
    #[test]
    fn an_unknown_glyph_renders_tofu_never_skips() {
        let aspect = 16.0 / 9.0;
        let mut verts = Vec::new();
        push_text(&mut verts, "\u{4E2D}", -0.5, 0.5, 0.05, aspect, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(verts.len(), 12, "one tofu => shadow quad + glyph quad");
        assert!(verts.iter().all(|v| v.uv[0] >= 0.0), "the tofu samples the atlas");
        assert!(text_width("\u{4E2D}", 0.05, aspect) > 0.0, "the tofu advances the pen");
        for style in Style::ALL {
            assert!(
                text_width_styled(style, "\u{4E2D}\u{4E2D}", 0.05, aspect)
                    > text_width_styled(style, "\u{4E2D}", 0.05, aspect)
            );
        }
    }

    /// The two faces are different faces: the same word measures differently in the stencil
    /// and in the text face, and both draw it.
    #[test]
    fn the_styles_set_the_same_word_differently() {
        let aspect = 16.0 / 9.0;
        let label = text_width_styled(Style::LABEL, "AMMUNITION", 0.05, aspect);
        let value = text_width_styled(Style::VALUE, "AMMUNITION", 0.05, aspect);
        assert!(label > 0.0 && value > 0.0);
        assert!((label - value).abs() > 1.0e-4, "two faces, one advance — the bake read one font");
        let mut verts = Vec::new();
        push_text_styled(
            &mut verts,
            Style::BANNER,
            "ZWYCIĘSTWO",
            -0.5,
            0.5,
            0.08,
            aspect,
            [1.0; 4],
        );
        assert_eq!(verts.len(), 10 * 12, "every Polish letter of the banner drew");
    }
}
