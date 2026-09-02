//! The sniper scope surround: a dark vignette outside a circular sight window, a thin rim ring,
//! and four short stadia ticks. Pure dressing in the "military instrument" direction — it frames
//! the magnified picture as *optics* instead of a naked zoomed viewport, without covering any of
//! the sight picture the reticle owns.
//!
//! The surround rides the camera's mode-transition clock (`fade` 0..1): the housing irises in
//! and darkens as the view travels into the scope, and lifts away as it leaves. At `fade` 1 the
//! window sits at its resting radius; below 1 it is wider and more transparent, so entering
//! sniper reads as bringing the eye to the eyepiece instead of a hard cut.

use renderer_api::HudVertex;

use super::primitives::push_arc;
use super::theme;

/// Radius of the settled circular sight window, in clip-X units (y is aspect-corrected, so the
/// window is a circle on screen). Measured in the frame's own aspect (Inny Poziom A7): the old
/// radius was 0.94 of the HEIGHT, which on a 16:9 frame reached only 0.53 of the width and put
/// 61 % of the picture under near-black housing — the enemy the scope exists to read sat in a
/// porthole. A circle 0.94 of the WIDTH fits the frame; only its corners darken.
const WINDOW_RADIUS: f32 = 0.94;
/// Window radius at the start of the iris (fade 0): mostly off-screen, barely a shadow.
const WINDOW_RADIUS_OPEN: f32 = 1.6;
/// Segments of the vignette ring; each spans window edge -> far outside the screen.
const SEGMENTS: u32 = 64;
/// The settled vignette shade: near-black, high alpha — optics housing, not a tint.
pub(crate) const VIGNETTE_COLOR: [f32; 4] = [0.015, 0.017, 0.018, 0.87];

/// Draw the scope surround at `fade` (0 = absent, 1 = settled). Painted before the reticle so
/// every live marker stays on top. Callers skip the call entirely at fade ~0.
pub(crate) fn push_scope_overlay(vertices: &mut Vec<HudVertex>, aspect: f32, fade: f32) {
    let aspect = aspect.max(0.01);
    let fade = fade.clamp(0.0, 1.0);
    let window_radius = WINDOW_RADIUS_OPEN + (WINDOW_RADIUS - WINDOW_RADIUS_OPEN) * fade;
    // `tagged` SETS alpha, so scale the housing's own resting alpha by the fade.
    let vignette = theme::tagged(VIGNETTE_COLOR, VIGNETTE_COLOR[3] * fade);
    // The vignette: an annulus from the window edge to well past the screen corners, built as a
    // triangle strip of quads (two triangles per segment).
    let outer = 3.0;
    // Radius in clip-x; the y leg is scaled UP by the aspect so the window stays circular on a
    // wide frame instead of shrinking to the height.
    let point = |radius: f32, angle: f32| -> [f32; 2] {
        [angle.cos() * radius, angle.sin() * radius * aspect]
    };
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (i0, i1) = (point(window_radius, a0), point(window_radius, a1));
        let (o0, o1) = (point(outer, a0), point(outer, a1));
        for corner in [i0, o0, i1, i1, o0, o1] {
            vertices.push(HudVertex::new(corner, vignette));
        }
    }
    // The rim: one hairline ring where the housing meets the glass.
    push_arc(
        vertices,
        [0.0, 0.0],
        window_radius,
        0.0,
        std::f32::consts::TAU,
        64,
        aspect,
        theme::tagged(theme::color::HAIRLINE, 0.35 * fade),
    );
    // Stadia ticks at 3/6/9/12 o'clock: short, dim, well outside any realistic dispersion ring.
    let tick_color = theme::tagged(theme::color::TEXT_DIM, 0.4 * fade);
    let (inner_r, outer_r) = (window_radius * 0.62, window_radius * 0.72);
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
        push_scope_overlay(&mut vertices, 16.0 / 9.0, 1.0);

        let vignette = vertices.iter().filter(|v| v.color == VIGNETTE_COLOR).count();
        assert_eq!(vignette as u32, SEGMENTS * 6, "full annulus of vignette quads");
        assert!(
            vertices.len() > vignette,
            "the rim ring and the stadia ticks draw on top of the vignette"
        );
        // Nothing may intrude into the sight window: every vignette vertex sits on or outside
        // the window radius (x-normalized: undo the aspect scaling on y).
        let aspect = 16.0f32 / 9.0;
        for vertex in vertices.iter().filter(|v| v.color == VIGNETTE_COLOR) {
            let r = (vertex.position[0].powi(2) + (vertex.position[1] / aspect).powi(2)).sqrt();
            assert!(r >= WINDOW_RADIUS - 1.0e-4, "vignette must stay outside the window: {r}");
        }
    }

    /// Inny Poziom A7: the housing is measured in the frame's aspect. On a 16:9 frame the
    /// settled window spans 0.94 of the WIDTH, so the vignette covers only the corners — well
    /// under a fifth of the picture — where the height-based window buried 61 % of it.
    #[test]
    fn the_settled_housing_leaves_most_of_a_wide_frame_to_the_picture() {
        let aspect = 16.0f32 / 9.0;
        let mut vertices = Vec::new();
        push_scope_overlay(&mut vertices, aspect, 1.0);
        // The window edge is a circle of `WINDOW_RADIUS` in clip-x; a clip point is under the
        // housing when its x-normalized radius exceeds it. Sample the frame on a grid.
        let steps = 200;
        let mut covered = 0usize;
        for iy in 0..steps {
            for ix in 0..steps {
                let x = -1.0 + (ix as f32 + 0.5) / steps as f32 * 2.0;
                let y = -1.0 + (iy as f32 + 0.5) / steps as f32 * 2.0;
                if (x * x + (y / aspect) * (y / aspect)).sqrt() > WINDOW_RADIUS {
                    covered += 1;
                }
            }
        }
        let share = covered as f32 / (steps * steps) as f32;
        assert!(share < 0.20, "the housing covers {:.1}% of a 16:9 frame", share * 100.0);
    }

    /// Mid-transition the housing is wider (iris still opening onto the eyepiece) and more
    /// transparent than the settled scope — the surround travels WITH the camera blend instead
    /// of popping.
    #[test]
    fn a_partial_fade_is_wider_and_more_transparent_than_the_settled_scope() {
        let aspect = 16.0f32 / 9.0;
        let inner_radius = |fade: f32| {
            let mut vertices = Vec::new();
            push_scope_overlay(&mut vertices, aspect, fade);
            vertices
                .iter()
                .map(|v| (v.position[0].powi(2) + (v.position[1] / aspect).powi(2)).sqrt())
                .fold(f32::MAX, f32::min)
        };
        assert!(
            inner_radius(0.3) > inner_radius(1.0) + 0.2,
            "the window irises shut as the fade settles"
        );

        let max_alpha = |fade: f32| {
            let mut vertices = Vec::new();
            push_scope_overlay(&mut vertices, aspect, fade);
            vertices.iter().map(|v| v.color[3]).fold(0.0f32, f32::max)
        };
        assert!(max_alpha(0.3) < max_alpha(1.0) * 0.5, "the housing darkens as the fade settles");
    }
}
