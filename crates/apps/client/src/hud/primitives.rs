//! Shared 2D drawing primitives for the HUD and garage overlays: flat quads, fill bars, and the
//! chamfered panels + hairline rules of the instrument art direction (`hud/theme.rs`). Everything
//! works in clip space and emits plain solid-color `HudVertex` triangles.

use renderer_api::HudVertex;

use super::theme;

pub fn push_quad(vertices: &mut Vec<HudVertex>, center: [f32; 2], half: [f32; 2], color: [f32; 4]) {
    let (left, right) = (center[0] - half[0], center[0] + half[0]);
    let (bottom, top) = (center[1] - half[1], center[1] + half[1]);
    for position in
        [[left, bottom], [right, bottom], [right, top], [left, bottom], [right, top], [left, top]]
    {
        vertices.push(HudVertex::new(position, color));
    }
}

/// A left-aligned bar: a dark background plus a colored fill of `frac` width.
/// `left` is the bar's left edge; `half` is the full-bar half-extent.
pub fn push_bar(
    vertices: &mut Vec<HudVertex>,
    left: [f32; 2],
    half: [f32; 2],
    frac: f32,
    color: [f32; 4],
) {
    push_quad(vertices, [left[0] + half[0], left[1]], half, [0.0, 0.0, 0.0, 0.55]);
    let fill = half[0] * frac.clamp(0.0, 1.0);
    push_quad(vertices, [left[0] + fill, left[1]], [fill, half[1]], color);
}

/// A flat panel with 45-degree chamfered corners — the armor-plate cut of the instrument art
/// direction (`hud/theme.rs`). `chamfer` is the corner cut in clip-y units; the x cut is divided
/// by `aspect` so the angle stays 45 degrees on screen. Emits a 6-triangle fan (18 vertices);
/// degenerate chamfers collapse gracefully toward a plain quad.
pub fn push_panel(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    half: [f32; 2],
    chamfer: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let cy = chamfer.clamp(0.0, half[1]);
    let cx = (chamfer / aspect.max(0.01)).clamp(0.0, half[0]);
    let (left, right) = (center[0] - half[0], center[0] + half[0]);
    let (bottom, top) = (center[1] - half[1], center[1] + half[1]);
    let ring = [
        [left + cx, top],
        [right - cx, top],
        [right, top - cy],
        [right, bottom + cy],
        [right - cx, bottom],
        [left + cx, bottom],
        [left, bottom + cy],
        [left, top - cy],
    ];
    for i in 1..ring.len() - 1 {
        vertices.push(HudVertex::new(ring[0], color));
        vertices.push(HudVertex::new(ring[i], color));
        vertices.push(HudVertex::new(ring[i + 1], color));
    }
}

/// A thin quad between two clip-space points (arc segments, marker arms).
pub fn push_segment(
    vertices: &mut Vec<HudVertex>,
    a: [f32; 2],
    b: [f32; 2],
    half_thick: f32,
    color: [f32; 4],
) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1.0e-6);
    let (nx, ny) = (-dy / len * half_thick, dx / len * half_thick);
    let corners = [
        [a[0] + nx, a[1] + ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] + nx, a[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] - nx, a[1] - ny],
    ];
    for position in corners {
        vertices.push(HudVertex::new(position, color));
    }
}

/// An arc of short segments around `center`, radius in clip-y units (x aspect-corrected so the
/// arc stays circular on screen).
#[allow(clippy::too_many_arguments)]
pub fn push_arc(
    vertices: &mut Vec<HudVertex>,
    center: [f32; 2],
    radius: f32,
    start_rad: f32,
    sweep_rad: f32,
    segments: u32,
    aspect: f32,
    color: [f32; 4],
) {
    let point =
        |angle: f32| [center[0] + angle.cos() * radius / aspect, center[1] + angle.sin() * radius];
    for index in 0..segments {
        let a = start_rad + sweep_rad * (index as f32 / segments as f32);
        let b = start_rad + sweep_rad * ((index + 1) as f32 / segments as f32);
        push_segment(vertices, point(a), point(b), 0.0024, color);
    }
}

/// A thin horizontal hairline rule from `left_x` to `right_x` centred on `y` — the instrument
/// panel's engraved divider (`theme::HAIRLINE_THICKNESS` thick).
pub fn push_hairline(
    vertices: &mut Vec<HudVertex>,
    left_x: f32,
    right_x: f32,
    y: f32,
    color: [f32; 4],
) {
    let half_w = (right_x - left_x).abs() / 2.0;
    let center_x = (left_x + right_x) / 2.0;
    push_quad(vertices, [center_x, y], [half_w, theme::HAIRLINE_THICKNESS / 2.0], color);
}
