//! The shell's pages as draw lists (interface program P6): the settings page — one brushed
//! plate, a row per setting, the value under glass between its two arrows, a bar where the
//! value sits on a range, the footer naming the keys from the table (P7). Drawn INSTEAD of
//! the battle's instruments while it is open: a page the player opened to read, over a scrim,
//! the battle still moving behind it. The escape menu is the way in and the way out (P8 adds
//! the garage's).

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload, WidgetState};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::{Palette, Theme};
use ui_kit::ui::{Anchor, Ui};

use super::elements::{HudElement, ShellPart};
use super::layout::Preset;
use crate::app::settings::{SENSITIVITY_RANGE, SENSITIVITY_STEPS, UI_SCALE_RANGE};
use crate::ui_strings::battle as words;

/// The page's plate and its rows, in `u`.
pub const PANEL_W_U: f32 = 760.0;
pub const ROW_H_U: f32 = 34.0;
const ROW_GAP_U: f32 = 4.0;
const PAD_U: f32 = 28.0;
const TITLE_H_U: f32 = 40.0;
const RULE_GAP_U: f32 = 10.0;
const FOOTER_GAP_U: f32 = 12.0;
const FOOTER_H_U: f32 = 26.0;
const VALUE_W_U: f32 = 220.0;
const ARROW_W_U: f32 = 30.0;
/// The scrim under the page: the battle sat back, never hidden.
const SCRIM: [f32; 4] = [0.02, 0.025, 0.03, 0.62];

/// The settings, one row each. Append-only: a row's index is its parts' index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsRow {
    Palette,
    MasterVolume,
    SensitivityThirdPerson,
    SensitivityScope1,
    SensitivityScope2,
    SensitivityScope3,
    SensitivityScope4,
    SensitivityScope5,
    UiScale,
    SniperKey,
    Fullscreen,
    Daylight,
    HudPreset,
}

impl SettingsRow {
    pub const ALL: [SettingsRow; 13] = [
        SettingsRow::Palette,
        SettingsRow::MasterVolume,
        SettingsRow::SensitivityThirdPerson,
        SettingsRow::SensitivityScope1,
        SettingsRow::SensitivityScope2,
        SettingsRow::SensitivityScope3,
        SettingsRow::SensitivityScope4,
        SettingsRow::SensitivityScope5,
        SettingsRow::UiScale,
        SettingsRow::SniperKey,
        SettingsRow::Fullscreen,
        SettingsRow::Daylight,
        SettingsRow::HudPreset,
    ];

    /// The zoom step a sensitivity row reads (0 = third person); `None` for the rest.
    pub fn zoom_step(self) -> Option<usize> {
        match self {
            SettingsRow::SensitivityThirdPerson => Some(0),
            SettingsRow::SensitivityScope1 => Some(1),
            SettingsRow::SensitivityScope2 => Some(2),
            SettingsRow::SensitivityScope3 => Some(3),
            SettingsRow::SensitivityScope4 => Some(4),
            SettingsRow::SensitivityScope5 => Some(5),
            _ => None,
        }
    }
}

/// The scope's magnifications, for the sensitivity rows' labels: the third-person view's
/// field over each step's — the number the zoom readout prints.
pub(crate) fn scope_zooms() -> [f32; SENSITIVITY_STEPS - 1] {
    let third = crate::camera::BattleCameraSettings::default().third_person_fov_degrees;
    crate::camera::zoom::SNIPER_FOV_STEPS_DEGREES.map(|fov| third / fov)
}

/// What the page shows: the settings as the app holds them, read here and never written.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsView {
    pub palette: Palette,
    pub master_gain: f32,
    pub sensitivity: [f32; SENSITIVITY_STEPS],
    pub ui_scale: f32,
    pub sniper_toggle: bool,
    pub borderless: bool,
    pub daylight: Option<scene_build::hangar::HangarLight>,
    pub preset: Preset,
}

/// One row as drawn: its label, its value as a word or a number, a bar when the value sits
/// on a range, and whether an arrow has nowhere to go.
#[derive(Debug, Clone, PartialEq)]
pub struct RowView {
    pub row: SettingsRow,
    pub label: String,
    pub value: String,
    pub frac: Option<f32>,
    pub at_min: bool,
    pub at_max: bool,
}

pub fn palette_word(palette: Palette) -> &'static str {
    match palette {
        Palette::Standard => words::WORD_STANDARD,
        Palette::Deuteranopia => words::WORD_DEUTERANOPIA,
        Palette::Protanopia => words::WORD_PROTANOPIA,
        Palette::Tritanopia => words::WORD_TRITANOPIA,
    }
}

pub fn daylight_word(light: Option<scene_build::hangar::HangarLight>) -> &'static str {
    use scene_build::hangar::HangarLight;
    match light {
        None => words::WORD_AUTO,
        Some(HangarLight::Morning) => words::WORD_MORNING,
        Some(HangarLight::Day) => words::WORD_DAY,
        Some(HangarLight::Evening) => words::WORD_EVENING,
    }
}

pub fn preset_word(preset: Preset) -> &'static str {
    match preset {
        Preset::Minimal => words::WORD_MINIMAL,
        Preset::Standard => words::WORD_STANDARD,
        Preset::Full => words::WORD_FULL,
    }
}

/// A value on a range: its fraction and whether it sits at either end.
fn ranged_row(value: f32, range: [f32; 2]) -> (Option<f32>, bool, bool) {
    let frac = ((value - range[0]) / (range[1] - range[0])).clamp(0.0, 1.0);
    (Some(frac), value <= range[0] + 1e-4, value >= range[1] - 1e-4)
}

impl SettingsView {
    pub fn row(&self, row: SettingsRow) -> RowView {
        let word = |label: &str, value: &str| RowView {
            row,
            label: label.to_string(),
            value: value.to_string(),
            frac: None,
            at_min: false,
            at_max: false,
        };
        let number = |label: String, value: String, range: [f32; 2], at: f32| {
            let (frac, at_min, at_max) = ranged_row(at, range);
            RowView { row, label, value, frac, at_min, at_max }
        };
        match row {
            SettingsRow::Palette => word(words::SET_PALETTE, palette_word(self.palette)),
            SettingsRow::MasterVolume => number(
                words::SET_VOLUME.to_string(),
                format!("{:.0} %", self.master_gain * 100.0),
                [0.0, 1.0],
                self.master_gain,
            ),
            SettingsRow::UiScale => number(
                words::SET_UI_SCALE.to_string(),
                format!("{:.0} %", self.ui_scale * 100.0),
                UI_SCALE_RANGE,
                self.ui_scale,
            ),
            SettingsRow::SniperKey => word(
                words::SET_SNIPER_KEY,
                if self.sniper_toggle { words::WORD_TOGGLE } else { words::WORD_HOLD },
            ),
            SettingsRow::Fullscreen => word(
                words::SET_FULLSCREEN,
                if self.borderless { words::WORD_BORDERLESS } else { words::WORD_WINDOWED },
            ),
            SettingsRow::Daylight => word(words::SET_DAYLIGHT, daylight_word(self.daylight)),
            SettingsRow::HudPreset => word(words::SET_HUD_PRESET, preset_word(self.preset)),
            sensitivity => {
                let step = sensitivity.zoom_step().unwrap_or(0);
                let label = match step {
                    0 => words::SET_SENS_THIRD.to_string(),
                    scope => format!(
                        "{} {}{:.1}",
                        words::SET_SENS_SCOPE,
                        words::ZOOM_PREFIX,
                        scope_zooms()[scope - 1]
                    ),
                };
                let value = self.sensitivity[step];
                number(label, format!("{value:.2}"), SENSITIVITY_RANGE, value)
            }
        }
    }

    pub fn rows(&self) -> Vec<RowView> {
        SettingsRow::ALL.iter().map(|row| self.row(*row)).collect()
    }
}

/// The settings page as the HUD draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsScreenModel {
    pub view: SettingsView,
    pub selected: usize,
    pub hovered: Option<ShellPart>,
    /// The keys of the shell's context, named from the table.
    pub footer: String,
}

/// The shell page over the battle, if one is open.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellModel {
    Settings(SettingsScreenModel),
}

/// The row a part belongs to.
pub fn shell_row_index(part: ShellPart) -> Option<u8> {
    match part {
        ShellPart::RowPlate(i)
        | ShellPart::RowLabel(i)
        | ShellPart::RowGlass(i)
        | ShellPart::RowValue(i)
        | ShellPart::RowBar(i)
        | ShellPart::RowDec(i)
        | ShellPart::RowInc(i) => Some(i),
        ShellPart::Scrim
        | ShellPart::Panel
        | ShellPart::Title
        | ShellPart::Rule
        | ShellPart::Footer => None,
    }
}

/// The parts and their rectangles on a built list, for the mouse.
pub(crate) fn shell_hit_rects(list: &DrawList<HudElement>) -> Vec<(ShellPart, Rect)> {
    list.iter()
        .filter_map(|element| match element.id {
            HudElement::Shell(part) => Some((part, element.rect)),
            _ => None,
        })
        .collect()
}

fn shell_text(
    text: &str,
    style: Style,
    size_u: f32,
    align: Align,
    color: [f32; 4],
    digits: DigitMode,
) -> Payload {
    Payload::Text { text: text.to_string(), style, size_u, align, color, digits }
}

fn shell_put(
    list: &mut DrawList<HudElement>,
    z: &mut i16,
    part: ShellPart,
    rect: Rect,
    payload: Payload,
    state: WidgetState,
) {
    list.push(Element::new(HudElement::Shell(part), rect, payload).z(*z).interactive(state));
    *z += 1;
}

/// The page over the scrim.
pub(crate) fn push_shell(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &ShellModel,
    z: &mut i16,
) {
    shell_put(
        list,
        z,
        ShellPart::Scrim,
        ui.viewport(),
        Payload::Bar { frac: 0.0, fill: SCRIM, back: SCRIM },
        WidgetState::Idle,
    );
    match model {
        ShellModel::Settings(page) => push_settings(list, ui, theme, page, z),
    }
}

fn push_settings(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &SettingsScreenModel,
    z: &mut i16,
) {
    let rows = page.view.rows();
    let panel_h = PAD_U
        + TITLE_H_U
        + 3.0
        + RULE_GAP_U
        + rows.len() as f32 * (ROW_H_U + ROW_GAP_U)
        + FOOTER_GAP_U
        + FOOTER_H_U
        + PAD_U;
    let panel = ui.anchor(Anchor::Center, [PANEL_W_U, panel_h], [0.0, 0.0]);
    let brushed = theme.plates.steel_brushed;
    shell_put(
        list,
        z,
        ShellPart::Panel,
        panel,
        Payload::Plate { tile: brushed.tile, radius_u: 6.0, bevel_u: 2.0, color: brushed.color },
        WidgetState::Idle,
    );
    let pad = ui.px(PAD_U);
    let inner_w = panel.w - 2.0 * pad;
    let title = Rect::new(panel.x + pad, panel.y + pad, inner_w, ui.px(TITLE_H_U));
    shell_put(
        list,
        z,
        ShellPart::Title,
        title,
        shell_text(
            words::SETTINGS_TITLE,
            Style::BANNER,
            28.0,
            Align::Left,
            theme.text.label,
            DigitMode::Proportional,
        ),
        WidgetState::Idle,
    );
    let dim = theme.text.label_dim;
    let rule =
        Rect::new(title.x, title.bottom() + ui.px(2.0), inner_w, ui.px(theme.hairline_u).max(1.0));
    shell_put(
        list,
        z,
        ShellPart::Rule,
        rule,
        Payload::Bar { frac: 0.0, fill: dim, back: dim },
        WidgetState::Idle,
    );
    let painted = theme.plates.steel_painted;
    let glass = theme.plates.glass;
    let row_top = rule.bottom() + ui.px(RULE_GAP_U);
    let hovered_row = page.hovered.and_then(shell_row_index);
    for (i, row) in rows.iter().enumerate() {
        let index = i as u8;
        let rect = Rect::new(
            title.x,
            row_top + i as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            inner_w,
            ui.px(ROW_H_U),
        );
        let selected = i == page.selected;
        let state = if selected {
            WidgetState::Focused
        } else if hovered_row == Some(index) {
            WidgetState::Hover
        } else {
            WidgetState::Idle
        };
        shell_put(
            list,
            z,
            ShellPart::RowPlate(index),
            rect,
            Payload::Plate {
                tile: painted.tile,
                radius_u: 2.0,
                bevel_u: 1.0,
                color: painted.color,
            },
            state,
        );
        let ink = if selected { theme.lamp } else { theme.text.label };
        shell_put(
            list,
            z,
            ShellPart::RowLabel(index),
            rect.inset(ui.px(8.0)),
            shell_text(&row.label, Style::LABEL, 18.0, Align::Left, ink, DigitMode::Proportional),
            WidgetState::Idle,
        );
        let inc =
            Rect::new(rect.right() - ui.px(6.0 + ARROW_W_U), rect.y, ui.px(ARROW_W_U), rect.h);
        let value = Rect::new(
            inc.x - ui.px(VALUE_W_U),
            rect.y + ui.px(3.0),
            ui.px(VALUE_W_U),
            rect.h - ui.px(6.0),
        );
        let dec = Rect::new(value.x - ui.px(ARROW_W_U), rect.y, ui.px(ARROW_W_U), rect.h);
        shell_put(
            list,
            z,
            ShellPart::RowGlass(index),
            value,
            Payload::Glass { radius_u: 1.0, phase: 0.3, color: glass.color },
            WidgetState::Idle,
        );
        shell_put(
            list,
            z,
            ShellPart::RowValue(index),
            value,
            shell_text(
                &row.value,
                Style::VALUE,
                18.0,
                Align::Center,
                theme.text.value,
                DigitMode::Tabular,
            ),
            WidgetState::Idle,
        );
        if let Some(frac) = row.frac {
            let bar = Rect::new(
                value.x + ui.px(12.0),
                value.bottom() - ui.px(5.0),
                value.w - ui.px(24.0),
                ui.px(3.0),
            );
            shell_put(
                list,
                z,
                ShellPart::RowBar(index),
                bar,
                Payload::Bar { frac, fill: theme.lamp, back: [dim[0], dim[1], dim[2], 0.35] },
                WidgetState::Idle,
            );
        }
        let arrow_state = |part: ShellPart, stuck: bool| {
            if stuck {
                WidgetState::Disabled
            } else if page.hovered == Some(part) {
                WidgetState::Hover
            } else {
                WidgetState::Idle
            }
        };
        shell_put(
            list,
            z,
            ShellPart::RowDec(index),
            dec,
            shell_text(
                words::ARROW_DEC,
                Style::LABEL,
                18.0,
                Align::Center,
                theme.text.label,
                DigitMode::Proportional,
            ),
            arrow_state(ShellPart::RowDec(index), row.at_min),
        );
        shell_put(
            list,
            z,
            ShellPart::RowInc(index),
            inc,
            shell_text(
                words::ARROW_INC,
                Style::LABEL,
                18.0,
                Align::Center,
                theme.text.label,
                DigitMode::Proportional,
            ),
            arrow_state(ShellPart::RowInc(index), row.at_max),
        );
    }
    let footer =
        Rect::new(title.x, panel.bottom() - pad - ui.px(FOOTER_H_U), inner_w, ui.px(FOOTER_H_U));
    shell_put(
        list,
        z,
        ShellPart::Footer,
        footer,
        shell_text(
            &page.footer,
            Style::LABEL,
            16.0,
            Align::Center,
            theme.lamp,
            DigitMode::Proportional,
        ),
        WidgetState::Idle,
    );
}

/// The page as the golden stages it: the defaults, the volume row selected, its arrow under
/// the cursor.
pub(crate) fn demo_settings_screen() -> ShellModel {
    let view = SettingsView {
        palette: Palette::Standard,
        master_gain: 0.85,
        sensitivity: [1.0, 1.0, 0.9, 0.8, 0.7, 0.6],
        ui_scale: 1.0,
        sniper_toggle: true,
        borderless: false,
        daylight: None,
        preset: Preset::Standard,
    };
    ShellModel::Settings(SettingsScreenModel {
        view,
        selected: 1,
        hovered: Some(ShellPart::RowInc(1)),
        footer: crate::app::shell::shell_footer(&crate::app::keybinds::KeyBindings::default()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::{build_battle_hud_list, demo};

    /// P6: the page prints every setting as a label and a value, lights the selected row as
    /// the focused one, disables an arrow at the end of its range, and replaces the battle's
    /// instruments while it is open.
    #[test]
    fn the_settings_page_prints_every_setting_and_lights_the_selected_row() {
        let ui = Ui::reference();
        let mut model = demo::demo_model(false);
        model.shell = Some(demo_settings_screen());
        let list = build_battle_hud_list(&model, &ui);
        assert!(
            list.iter().all(|element| matches!(element.id, HudElement::Shell(_))),
            "the page replaces the instruments"
        );
        let text_of = |list: &DrawList<HudElement>, part: ShellPart| match &list
            .find(HudElement::Shell(part))
            .expect("part")
            .payload
        {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{part:?} is not text: {other:?}"),
        };
        let state_of = |list: &DrawList<HudElement>, part: ShellPart| {
            list.find(HudElement::Shell(part)).expect("part").state
        };
        for (i, row) in SettingsRow::ALL.iter().enumerate() {
            let label = text_of(&list, ShellPart::RowLabel(i as u8));
            let value = text_of(&list, ShellPart::RowValue(i as u8));
            assert!(!label.is_empty() && !value.is_empty(), "{row:?}: {label:?} = {value:?}");
        }
        assert_eq!(text_of(&list, ShellPart::RowValue(1)), "85 %");
        assert_eq!(text_of(&list, ShellPart::RowValue(9)), words::WORD_TOGGLE);
        assert_eq!(
            text_of(&list, ShellPart::RowLabel(3)),
            format!("{} {}2.7", words::SET_SENS_SCOPE, words::ZOOM_PREFIX)
        );
        assert_eq!(state_of(&list, ShellPart::RowPlate(1)), WidgetState::Focused);
        assert_eq!(state_of(&list, ShellPart::RowPlate(0)), WidgetState::Idle);
        assert_eq!(state_of(&list, ShellPart::RowInc(1)), WidgetState::Hover);
        assert!(list.find(HudElement::Shell(ShellPart::RowBar(1))).is_some(), "a range has a bar");
        assert!(list.find(HudElement::Shell(ShellPart::RowBar(0))).is_none(), "a word has none");
        // The end of a range disables the arrow that has nowhere to go.
        let ShellModel::Settings(mut page) = demo_settings_screen();
        page.view.master_gain = 1.0;
        page.view.sensitivity[0] = SENSITIVITY_RANGE[0];
        model.shell = Some(ShellModel::Settings(page));
        let list = build_battle_hud_list(&model, &ui);
        assert_eq!(state_of(&list, ShellPart::RowInc(1)), WidgetState::Disabled);
        assert_ne!(state_of(&list, ShellPart::RowDec(1)), WidgetState::Disabled);
        assert_eq!(state_of(&list, ShellPart::RowDec(2)), WidgetState::Disabled);
        // Every sensitivity row reads its own step, once.
        let steps: Vec<usize> = SettingsRow::ALL.iter().filter_map(|row| row.zoom_step()).collect();
        assert_eq!(steps, (0..SENSITIVITY_STEPS).collect::<Vec<_>>());
    }
}
