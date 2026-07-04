//! Left crew column: the five roles (icon + name) and the shared proficiency control.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::layout::*;
use crate::hud::font::{push_icon, push_text, text_width};
use crate::hud::icons::HudIcon;
use crate::hud::push_quad;

pub(in crate::app::garage) fn draw(v: &mut Vec<HudVertex>, state: &GarageState, aspect: f32) {
    push_quad(v, [CREW_X, 0.46], [CREW_HALF_X + 0.02, 0.34], PANEL);
    let left = CREW_X - CREW_HALF_X;
    push_text(v, crate::ui_strings::garage::CREW, left, 0.80, 0.04, aspect, TEXT_DIM);

    for (i, role) in game_core::Crew::roles().into_iter().enumerate() {
        let y = CREW_TOP - i as f32 * CREW_PITCH;
        push_icon(v, HudIcon::Crew, left, y + 0.035, 0.06, aspect, ICON);
        push_text(v, role.label(), left + 0.075, y + 0.022, 0.038, aspect, TEXT);
    }

    push_text(
        v,
        crate::ui_strings::garage::PROFICIENCY,
        left,
        PROF_Y + 0.06,
        0.034,
        aspect,
        TEXT_DIM,
    );
    let (l, r) = crew_prof_arrows();
    push_arrow(v, l, false, aspect);
    push_arrow(v, r, true, aspect);
    let pct = (state.draft().crew().proficiency() * 100.0).round() as u32;
    let label = format!("{pct}%");
    let w = text_width(&label, 0.045, aspect);
    push_text(v, &label, (l[0] + r[0]) / 2.0 - w / 2.0, PROF_Y + 0.024, 0.045, aspect, VALUE);
}

fn push_arrow(v: &mut Vec<HudVertex>, center: [f32; 2], right: bool, aspect: f32) {
    push_quad(v, center, ARROW_HALF, SLOT);
    let glyph = if right { ">" } else { "<" };
    let w = text_width(glyph, 0.045, aspect);
    push_text(v, glyph, center[0] - w / 2.0, center[1] + 0.022, 0.045, aspect, TEXT);
}
