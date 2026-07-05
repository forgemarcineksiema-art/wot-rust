//! The sniper scope surround: a dark vignette outside a circular sight window, a thin rim ring,
//! and four short stadia ticks. Pure dressing in the "military instrument" direction — it frames
//! the magnified picture as *optics* instead of a naked zoomed viewport, without covering any of
//! the sight picture the reticle owns.

use renderer_api::HudVertex;

use super::primitives::push_arc;
use super::theme;

/// Radius of the circular sight window, in clip-y units (x is aspect-corrected).
const WINDOW_RADIUS: f32 = 0.94;
/// Segments of the vignette ring; each spans window edge -> far outside the screen.
const SEGMENTS: u32 = 64;
/// The vignette shade: near-black, high alpha — optics housing, not a tint.
pub(crate) const VIGNETTE_COLOR: [f32; 4] = [0.015, 0.017, 0.018, 0.87];

/// Draw the scope surround. Painted before the reticle so every live marker stays on top.
pub(crate) fn push_scope_overlay(vertices: &mut Vec<HudVertex>, aspect: f32) {
    let aspect = aspect.max(0.01);
    // The vignette: an annulus from the window edge to well past the screen corners, built as a
    // triangle strip of quads (two triangles per segment).
    let outer = 3.0;
    let point = |radius: f32, angle: f32| -> [f32; 2] {
        [angle.cos() * radius / aspect, angle.sin() * radius]
    };
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (i0, i1) = (point(WINDOW_RADIUS, a0), point(WINDOW_RADIUS, a1));
        let (o0, o1) = (point(outer, a0), point(outer, a1));
        for corner in [i0, o0, i1, i1, o0, o1] {
            vertices.push(HudVertex::new(corner, VIGNETTE_COLOR));
        }
    }
    // The rim: one hairline ring where the housing meets the glass.
    push_arc(
        vertices,
        [0.0, 0.0],
        WINDOW_RADIUS,
        0.0,
        std::f32::consts::TAU,
        64,
        aspect,
        theme::tagged(theme::color::HAIRLINE, 0.35),
    );
    // Stadia ticks at 3/6/9/12 o'clock: short, dim, well outside any realistic dispersion ring.
    let tick_color = theme::tagged(theme::color::TEXT_DIM, 0.4);
    let (inner_r, outer_r) = (WINDOW_RADIUS * 0.62, WINDOW_RADIUS * 0.72);
    for quarter in 0..4 {
        let angle = quarter as f32 * std::f32::consts::FRAC_PI_2;
        super::primitives::push_segment(
            vertices,
            point(inner_r, angle),
            point(outer_r, angle),
            0.0012,
            tick_color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scope_surround_draws_a_vignette_ring_and_stadia() {
        let mut vertices = Vec::new();
        push_scope_overlay(&mut vertices, 16.0 / 9.0);

        let vignette = vertices.iter().filter(|v| v.color == VIGNETTE_COLOR).count();
        assert_eq!(vignette as u32, SEGMENTS * 6, "full annulus of vignette quads");
        assert!(
            vertices.len() > vignette,
            "the rim ring and the stadia ticks draw on top of the vignette"
        );
        // Nothing may intrude into the sight window: every vignette vertex sits on or outside
        // the window radius (y-normalized: undo the aspect correction on x).
        let aspect = 16.0f32 / 9.0;
        for vertex in vertices.iter().filter(|v| v.color == VIGNETTE_COLOR) {
            let r = ((vertex.position[0] * aspect).powi(2) + vertex.position[1].powi(2)).sqrt();
            assert!(r >= WINDOW_RADIUS - 1.0e-4, "vignette must stay outside the window: {r}");
        }
    }
}
