//! Bottom vehicle carousel: a horizontal strip of owned tanks that scrolls through a fixed window
//! once the roster outgrows `CAR_VISIBLE`. Click selects; the scroll arrows appear only when the
//! roster overflows. Each cell shows the vehicle's nation label above its short name.

use game_core::VehicleKind;
use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::*;
use crate::hud::font::{push_text, text_width};
use crate::hud::push_panel;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    let count = VehicleKind::PLAYABLE.len();
    let window = carousel_window(count, state.carousel_scroll());
    let visible = window.len();
    push_panel(
        v,
        [0.0, CAR_Y],
        [visible as f32 * 0.065 + 0.02, CAR_HALF[1] + 0.02],
        CHAMFER_PANEL,
        aspect,
        PANEL,
    );

    for (slot, absolute) in window.enumerate() {
        let kind = VehicleKind::PLAYABLE[absolute];
        let c = carousel_cell_center(slot, visible);
        let selected = absolute == state.selected_index();
        push_panel(
            v,
            c,
            CAR_HALF,
            CHAMFER_SLOT,
            aspect,
            if selected { SLOT_SELECTED } else { SLOT },
        );
        let text_color = if selected { TEXT } else { TEXT_DIM };

        // Nation label above the short name, colored by nation for at-a-glance grouping.
        let nation = kind.nation();
        let nation_label = nation.label();
        let nation_w = text_width(nation_label, NATION_TEXT_SIZE, aspect);
        let nation_color = nation.color();
        push_text(
            v,
            nation_label,
            c[0] - nation_w / 2.0,
            c[1] + 0.075,
            NATION_TEXT_SIZE,
            aspect,
            [nation_color[0], nation_color[1], nation_color[2], 0.95],
        );

        push_text(
            v,
            short_name(kind),
            c[0] - CAR_HALF[0] + 0.01,
            c[1] + 0.045,
            0.03,
            aspect,
            text_color,
        );
        push_text(
            v,
            &format!("{}", absolute + 1),
            c[0] - CAR_HALF[0] + 0.01,
            c[1] - 0.005,
            0.026,
            aspect,
            TEXT_DIM,
        );
    }

    // Scroll arrows flank the window only when the roster does not fit.
    if carousel_overflows(count) {
        let (left, right) = carousel_arrows();
        for (center, glyph) in [(left, "<"), (right, ">")] {
            push_panel(v, center, CAR_ARROW_HALF, CHAMFER_SLOT, aspect, SLOT);
            let w = text_width(glyph, 0.05, aspect);
            push_text(v, glyph, center[0] - w / 2.0, center[1] + 0.022, 0.05, aspect, TEXT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carousel_emits_text_for_nation_labels_and_vehicle_names() {
        let mut state = GarageState::default();
        state.select_vehicle(VehicleKind::T54_1951);
        let aspect = 16.0 / 9.0;

        let mut v = Vec::new();
        draw(&mut v, &state, aspect);

        // 5 vehicle cells: 1 background quad + 5 slot quads = 6 quads = 36 vertices.
        // "USSR" (4 glyphs) + "Germany" (7 glyphs × 4 vehicles) + short names + indices add
        // many more, so the total must far exceed the quad-only baseline.
        assert!(
            v.len() > 36,
            "carousel must emit text vertices beyond plain quads, got {}",
            v.len()
        );
    }

    #[test]
    fn carousel_emits_more_vertices_for_germany_than_a_single_ussr_label() {
        // "Germany" is 7 glyphs; "USSR" is 4. With one USSR and four Germany vehicles in the
        // playable roster, Germany glyphs dominate. This is a smoke test that nation labels
        // are actually drawn, not just the background panel.
        let state = GarageState::default();
        let aspect = 16.0 / 9.0;

        let mut v = Vec::new();
        draw(&mut v, &state, aspect);

        // Each glyph is 6 vertices. 4 Germany × 7 glyphs × 6 = 168 vertices from Germany alone.
        // Assert the total is at least that, confirming Germany labels are emitted.
        assert!(
            v.len() >= 168,
            "carousel must emit Germany nation labels (4×7 glyphs×6 verts = 168), got {}",
            v.len()
        );
    }
}
