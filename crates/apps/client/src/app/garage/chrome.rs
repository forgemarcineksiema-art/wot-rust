//! The hangar screen's chrome (interface program G6, G9): the key legend on the top bar, the
//! filter chips beside the carousel, and the tooltip that follows a resting cursor. Laid out
//! in `u` like the rest of `screen.rs`, on the same put/plate/text helpers.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, WidgetState};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

use super::GarageState;
use super::elements::GarageElement as E;
use super::filter::Chip;
use super::hints::hint_lines;
use super::screen::{plate, put, put_control, text};

// The legend: three lines in the top bar's empty left third.
const HINT_LEFT_U: f32 = 30.0;
const HINT_TOP_U: f32 = 20.0;
const HINT_PITCH_U: f32 = 28.0;
const HINT_SIZE_U: [f32; 2] = [440.0, 24.0];
// The chips: a stack in the bottom-left corner, beside the carousel.
const CHIP_LEFT_U: f32 = 20.0;
const CHIP_BOTTOM_U: f32 = 14.0;
const CHIP_PITCH_U: f32 = 50.0;
const CHIP_SIZE_U: [f32; 2] = [340.0, 40.0];
// The tooltip: a small enamel plate under (or over) the control it explains.
const TOOLTIP_H_U: f32 = 34.0;
const TOOLTIP_PAD_U: f32 = 12.0;
const TOOLTIP_GAP_U: f32 = 6.0;
const TOOLTIP_MARGIN_U: f32 = 8.0;

/// The width a run of text takes on this context, in pixels.
pub(super) fn text_px_width(ui: &Ui, style: Style, text: &str, size_u: f32) -> f32 {
    let height = ui.px(size_u) * ui.clip_per_px();
    ui_kit::font::text_width_styled(style, text, height, ui.aspect()) / ui.clip_per_px()
        * ui.aspect()
}

/// The key legend (G6): every key from the table, three lines in the label's ink — a reference,
/// legible (the look pass: dim at 16 u read as noise in the frame).
pub(super) fn push_hint_strip(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    for (i, line) in hint_lines(state.key_labels()).iter().enumerate() {
        let rect = Rect::new(
            ui.px(HINT_LEFT_U),
            ui.px(HINT_TOP_U + i as f32 * HINT_PITCH_U),
            ui.px(HINT_SIZE_U[0]),
            ui.px(HINT_SIZE_U[1]),
        );
        let z = list.len() as i16;
        list.push(
            Element::new(
                E::HintLine(i as u8),
                rect,
                text(
                    line,
                    Style::VALUE_STRONG,
                    18.0,
                    Align::Left,
                    theme.text.label,
                    DigitMode::Proportional,
                ),
            )
            .z(z)
            .clipped(Some(rect)),
        );
    }
}

/// The filter chips (G9): CLASS, NATION, TIER — the label on the left, the value on the right,
/// in the lamp when the chip is set.
pub(super) fn push_chips(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    let viewport = ui.viewport();
    let painted = theme.plates.steel_painted;
    let filter = state.filter();
    for chip in Chip::ALL {
        let index = chip.index();
        let from_bottom =
            CHIP_BOTTOM_U + (Chip::ALL.len() as f32 - f32::from(index)) * CHIP_PITCH_U;
        let rect = Rect::new(
            ui.px(CHIP_LEFT_U),
            viewport.bottom() - ui.px(from_bottom),
            ui.px(CHIP_SIZE_U[0]),
            ui.px(CHIP_SIZE_U[1]),
        );
        put_control(list, E::Chip(index), rect, plate(theme, painted, 2.0), WidgetState::Idle);
        let inner = rect.inset(ui.px(10.0));
        put(
            list,
            E::ChipLabel(index),
            inner,
            text(
                chip.word(),
                Style::VALUE_STRONG,
                18.0,
                Align::Left,
                theme.text.label,
                DigitMode::Proportional,
            ),
        );
        let set = filter.is_set(chip);
        put(
            list,
            E::ChipValue(index),
            inner,
            text(
                &filter.value_word(chip),
                Style::VALUE,
                18.0,
                Align::Right,
                if set { theme.lamp } else { theme.text.label_dim },
                DigitMode::Proportional,
            ),
        );
    }
}

/// The tooltip (G6): under the control it explains, over it when the frame's bottom is near,
/// always inside the frame.
pub(super) fn push_tooltip(
    list: &mut DrawList<E>,
    ui: &Ui,
    theme: &Theme,
    anchor: Rect,
    words: &str,
) {
    let viewport = ui.viewport();
    let margin = ui.px(TOOLTIP_MARGIN_U);
    let w = text_px_width(ui, Style::VALUE, words, 18.0) + 2.0 * ui.px(TOOLTIP_PAD_U);
    let h = ui.px(TOOLTIP_H_U);
    let x =
        (anchor.center()[0] - w * 0.5).clamp(margin, (viewport.right() - margin - w).max(margin));
    let below = anchor.bottom() + ui.px(TOOLTIP_GAP_U);
    let y = if below + h > viewport.bottom() - margin {
        anchor.y - ui.px(TOOLTIP_GAP_U) - h
    } else {
        below
    };
    let rect = Rect::new(x, y.max(margin), w, h);
    put(list, E::TooltipPlate, rect, plate(theme, theme.plates.enamel_black, 2.0));
    put(
        list,
        E::TooltipText,
        rect,
        text(words, Style::VALUE, 18.0, Align::Center, theme.text.value, DigitMode::Proportional),
    );
}
