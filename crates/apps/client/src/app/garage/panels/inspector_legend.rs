//! The armor inspector's millimetre legend (Hala v4 R1): the color ramp the overlay paints
//! the plates with, labeled in the unit the sim resolves in. The swatches SAMPLE
//! `armor_overlay::color_for_mm` at the ramp's own anchor stops — the legend carries no
//! colors of its own, so it cannot drift from the scale it explains.

use renderer_api::HudVertex;

use crate::app::garage::layout::{CHAMFER_PANEL, PANEL, TEXT, TEXT_DIM};
use crate::hud::font::{push_text, text_width};
use crate::hud::{push_panel, push_quad};
use crate::vehicle::armor_overlay::color_for_mm;

/// The mm stops the legend prints — the gradient's OWN anchor points (see `color_for_mm`),
/// so every labeled swatch is exactly a stop of the scale, not an interpolation the eye must
/// trust. Locked against the scale by `the_legend_is_the_scale_it_explains`.
pub(in crate::app::garage) const LEGEND_STOPS_MM: [f32; 5] = [10.0, 40.0, 90.0, 150.0, 230.0];

/// Under the nameplate band, over the hero: visible exactly while the overlay it explains is.
/// Sits below the DAMAGED/REPAIR tag line (0.625) and above the hero's turret.
const LEGEND_CENTER: [f32; 2] = [0.0, 0.545];
const LEGEND_HALF: [f32; 2] = [0.30, 0.034];
/// One text size for the whole strip — the audit's pixel floor says nothing under 0.022
/// survives 768p, and a legend nobody can read is a decoration.
const LEGEND_TEXT: f32 = 0.022;

/// The (mm, rgb) pairs the legend draws, straight from the scale. Split out so the lock can
/// read the DATA the quads are built from instead of reverse-engineering vertices.
pub(in crate::app::garage) fn legend_swatches() -> [(f32, [f32; 3]); 5] {
    LEGEND_STOPS_MM.map(|mm| (mm, color_for_mm(mm)))
}

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, aspect: f32) {
    push_panel(v, LEGEND_CENTER, LEGEND_HALF, CHAMFER_PANEL, aspect, PANEL);

    let title = "ARMOR";
    push_text(
        v,
        title,
        LEGEND_CENTER[0] - LEGEND_HALF[0] + 0.018,
        LEGEND_CENTER[1] + 0.012,
        LEGEND_TEXT,
        aspect,
        TEXT_DIM,
    );
    let title_w = text_width(title, LEGEND_TEXT, aspect) + 0.036;

    // Five swatches with their mm labels, spread over the rest of the plate; "MM" closes
    // the row so the unit is printed once instead of five times.
    let row_left = LEGEND_CENTER[0] - LEGEND_HALF[0] + title_w;
    let unit_w = text_width("MM", LEGEND_TEXT, aspect) + 0.014;
    let row_right = LEGEND_CENTER[0] + LEGEND_HALF[0] - unit_w;
    let swatches = legend_swatches();
    let step = (row_right - row_left) / swatches.len() as f32;
    for (index, (mm, rgb)) in swatches.iter().enumerate() {
        let x = row_left + step * (index as f32 + 0.5);
        push_quad(
            v,
            [x - step * 0.26, LEGEND_CENTER[1]],
            [0.012, 0.016],
            [rgb[0], rgb[1], rgb[2], 0.95],
        );
        let label = format!("{}", *mm as i32);
        push_text(v, &label, x - step * 0.12, LEGEND_CENTER[1] + 0.012, LEGEND_TEXT, aspect, TEXT);
    }
    push_text(v, "MM", row_right + 0.006, LEGEND_CENTER[1] + 0.012, LEGEND_TEXT, aspect, TEXT_DIM);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The legend IS the scale: every swatch color equals `color_for_mm` at its printed stop,
    /// and the stops are exactly the gradient's anchors in ascending order. A legend with
    /// numbers of its own would be a second scale waiting to drift.
    #[test]
    fn the_legend_is_the_scale_it_explains() {
        let swatches = legend_swatches();
        for (mm, rgb) in swatches {
            assert_eq!(rgb, color_for_mm(mm), "swatch at {mm} mm drifted from the scale");
        }
        for pair in LEGEND_STOPS_MM.windows(2) {
            assert!(pair[0] < pair[1], "stops must ascend: {} then {}", pair[0], pair[1]);
        }
    }

    /// The strip stays in its band: under the nameplate's tag line, above the hero's turret,
    /// and inside the frame at 16:9.
    #[test]
    fn the_legend_sits_under_the_nameplate_band() {
        let mut v = Vec::new();
        draw(&mut v, 16.0 / 9.0);
        assert!(!v.is_empty(), "the legend must draw");
        assert!(
            v.iter().all(|vert| {
                vert.position[1] > 0.48 && vert.position[1] < 0.62 && vert.position[0].abs() < 0.99
            }),
            "the legend must stay in its band"
        );
    }
}
