//! The module option list: a small floating popup the player opens by clicking a swappable loadout
//! slot. It lists every option for that slot with its catalogue name and headline stat, plus a
//! green/red delta against the installed option — so a swap is an informed choice instead of a blind
//! cycle. Rows stack upward out of the clicked slot; the installed option reads bright/selected.

use renderer_api::HudVertex;

use crate::app::garage::GarageState;
use crate::app::garage::draft::FitSlot;
use crate::app::garage::layout::*;
use crate::hud::font::{push_text, push_text_right, text_width};
use crate::hud::push_panel;

pub(in crate::app::garage) fn draw(
    v: &mut Vec<HudVertex>,
    state: &GarageState,
    slot: FitSlot,
    aspect: f32,
) {
    let options = state.draft().module_options(slot);
    // The list only opens for slots with a real choice, but guard anyway — a one-option popup would
    // just be noise.
    if options.len() < 2 {
        return;
    }
    let si = slot.index();
    let n = options.len();
    let bottom = option_row_center(si, 0);
    let top = option_row_center(si, n - 1);
    let header_y = top[1] + OPTION_ROW_HALF[1] + 0.045;

    // Background panel behind the header and all rows.
    let low = bottom[1] - OPTION_ROW_HALF[1] - 0.018;
    let center = [bottom[0], (low + header_y) / 2.0];
    let half = [OPTION_ROW_HALF[0] + 0.02, (header_y - low) / 2.0];
    push_panel(v, center, half, CHAMFER_PANEL, aspect, PANEL);

    // Header: the slot name, centred over the column.
    let label = slot_label(slot);
    let lw = text_width(label, 0.028, aspect);
    push_text(v, label, bottom[0] - lw / 2.0, header_y, 0.028, aspect, TEXT_DIM);

    let installed_stat = options.iter().find(|o| o.installed).map_or(0.0, |o| o.stat);
    for (i, opt) in options.iter().enumerate() {
        let c = option_row_center(si, i);
        let bg = if opt.installed { SLOT_SELECTED } else { SLOT };
        push_panel(v, c, OPTION_ROW_HALF, CHAMFER_SLOT, aspect, bg);

        let name_color = if opt.installed { TEXT } else { TEXT_DIM };
        push_text(
            v,
            &opt.name,
            c[0] - OPTION_ROW_HALF[0] + 0.014,
            c[1] + 0.014,
            0.028,
            aspect,
            name_color,
        );

        let stat = format!("{} {}", fmt_stat(opt.stat), opt.unit);
        push_text_right(
            v,
            &stat,
            c[0] + OPTION_ROW_HALF[0] - 0.014,
            c[1] + 0.015,
            0.026,
            aspect,
            VALUE,
        );

        // Delta chip under the stat for every non-installed option, coloured by whether the swap is
        // an improvement on the slot's headline stat.
        if !opt.installed {
            let delta = opt.stat - installed_stat;
            if delta.abs() > 1.0e-3 {
                let better = (delta > 0.0) == opt.higher_is_better;
                let sign = if delta > 0.0 { "+" } else { "" };
                let chip = format!("{sign}{}", fmt_stat(delta));
                let color = if better { DELTA_GOOD } else { DELTA_BAD };
                push_text_right(
                    v,
                    &chip,
                    c[0] + OPTION_ROW_HALF[0] - 0.014,
                    c[1] - 0.014,
                    0.022,
                    aspect,
                    color,
                );
            }
        }
    }
}

/// Compact stat formatting: whole numbers for big/round values, one decimal for small fractional
/// ones (e.g. a reload of `6.9 s`).
fn fmt_stat(x: f32) -> String {
    if x.abs() >= 100.0 || x.fract().abs() < 0.05 {
        format!("{}", x.round() as i32)
    } else {
        format!("{x:.1}")
    }
}
