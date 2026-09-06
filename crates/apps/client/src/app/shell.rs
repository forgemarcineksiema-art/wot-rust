//! The shell's pages over the battle (interface program P6): the settings page, opened from
//! the escape menu, driven by the keys of `Context::Shell` and the mouse; every change is
//! applied at once (`edit_settings`) and written to `settings.json`; Esc goes back to the
//! menu it came from.

use ui_kit::rect::Rect;
use ui_kit::theme::Palette;

use super::ClientApp;
use super::keybinds::{Action, KeyBindings, key_label};
use super::settings::{
    GAIN_STEP, SENSITIVITY_RANGE, SENSITIVITY_STEP, UI_SCALE_RANGE, UI_SCALE_STEP,
};
use crate::hud::elements::ShellPart;
use crate::hud::layout::Preset;
use crate::hud::shell::{
    SettingsRow, SettingsScreenModel, SettingsView, ShellModel, shell_row_index,
};
use crate::ui_strings::battle as words;

/// The page's live state: the selected row, the part under the cursor, and the parts of the
/// last built frame for the mouse.
#[derive(Debug, Clone, Default)]
pub(crate) struct ShellState {
    selected: usize,
    hovered: Option<ShellPart>,
    hits: Vec<(ShellPart, Rect)>,
}

/// The footer of a shell page: the table's keys for the shell's words, never literals.
pub(crate) fn shell_footer(keybinds: &KeyBindings) -> String {
    let first = |action: Action| {
        keybinds.keys(action).first().map_or_else(|| "-".to_string(), |key| key_label(*key))
    };
    format!(
        "{}/{} {} \u{b7} {}/{} {} \u{b7} {} {}",
        first(Action::MenuUp),
        first(Action::MenuDown),
        words::FOOTER_SELECT,
        first(Action::MenuLeft),
        first(Action::MenuRight),
        words::FOOTER_CHANGE,
        first(Action::MenuBack),
        words::FOOTER_BACK
    )
}

/// A number one step along its range on the step's grid; `None` when it has nowhere to go.
fn stepped_value(value: f32, step: f32, range: [f32; 2], dir: i32) -> Option<f32> {
    let next = (((value / step).round() + dir as f32) * step).clamp(range[0], range[1]);
    ((next - value).abs() > 1e-4).then_some(next)
}

/// The next item on a ring, either way round.
fn ring_step<T: Copy + PartialEq>(all: &[T], current: T, dir: i32) -> T {
    let at = all.iter().position(|item| *item == current).unwrap_or(0) as i32;
    all[(at + dir).rem_euclid(all.len() as i32) as usize]
}

impl ClientApp {
    /// SETTINGS on the escape menu: the page over the battle, the cursor free, the driving
    /// keys released — the battle does not pause.
    pub(in crate::app) fn open_settings_page(&mut self) {
        self.pause_menu = None;
        self.command_wheel = None;
        self.input.release_driving();
        self.shell = Some(ShellState::default());
        self.set_cursor_captured(false);
    }

    /// Esc: back to the escape menu the page came from.
    pub(in crate::app) fn close_shell(&mut self) {
        if self.shell.take().is_some() {
            self.pause_menu = Some(super::PauseMenuState::opened());
        }
    }

    pub(crate) fn shell_open(&self) -> bool {
        self.shell.is_some()
    }

    fn settings_view(&self) -> SettingsView {
        let settings = &self.settings;
        SettingsView {
            palette: settings.palette(),
            master_gain: settings.master_gain,
            sensitivity: settings.sensitivity,
            ui_scale: settings.ui_scale,
            sniper_toggle: settings.sniper_toggle,
            borderless: settings.borderless,
            daylight: self.garage.daylight_override(),
            preset: self.hud_layout().preset,
        }
    }

    /// The page for the HUD, if one is open.
    pub(in crate::app) fn shell_model(&self) -> Option<ShellModel> {
        let shell = self.shell.as_ref()?;
        Some(ShellModel::Settings(SettingsScreenModel {
            view: self.settings_view(),
            selected: shell.selected,
            hovered: shell.hovered,
            footer: shell_footer(&self.keybinds),
        }))
    }

    /// The parts the last frame drew, for the mouse.
    pub(in crate::app) fn remember_shell_hits(&mut self, hits: Vec<(ShellPart, Rect)>) {
        if let Some(shell) = &mut self.shell {
            shell.hits = hits;
        }
    }

    /// The shell's keys (P7: the table's word for the key).
    pub(in crate::app) fn shell_action(&mut self, action: Action, pressed: bool) {
        if !pressed {
            return;
        }
        match action {
            Action::MenuUp => self.select_settings_row(-1),
            Action::MenuDown => self.select_settings_row(1),
            Action::MenuLeft => self.adjust_selected_setting(-1),
            Action::MenuRight | Action::MenuAccept => self.adjust_selected_setting(1),
            Action::MenuBack => self.close_shell(),
            _ => {}
        }
    }

    fn select_settings_row(&mut self, dir: i32) {
        let Some(shell) = &mut self.shell else { return };
        let rows = SettingsRow::ALL.len() as i32;
        shell.selected = (shell.selected as i32 + dir).rem_euclid(rows) as usize;
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// The cursor over the page: the topmost part under it.
    pub(in crate::app) fn shell_cursor(&mut self, cursor_px: [f32; 2]) {
        let Some(shell) = &mut self.shell else { return };
        shell.hovered = shell
            .hits
            .iter()
            .rev()
            .find(|(_, rect)| rect.contains(cursor_px))
            .map(|(part, _)| *part);
    }

    /// A click on the page: a row's arrow steps that row; anything else of a row selects it.
    pub(in crate::app) fn shell_press(&mut self) {
        let Some(part) = self.shell.as_ref().and_then(|shell| shell.hovered) else { return };
        let Some(row) = shell_row_index(part) else { return };
        if let Some(shell) = &mut self.shell {
            shell.selected = row as usize;
        }
        match part {
            ShellPart::RowDec(_) => self.adjust_selected_setting(-1),
            ShellPart::RowInc(_) => self.adjust_selected_setting(1),
            _ => self.queue_audio(audio::AudioEvent::UiClick { accent: false }),
        }
    }

    fn adjust_selected_setting(&mut self, dir: i32) {
        let Some(shell) = &self.shell else { return };
        let row = SettingsRow::ALL[shell.selected.min(SettingsRow::ALL.len() - 1)];
        let changed = self.adjust_setting(row, dir);
        self.queue_audio(if changed {
            audio::AudioEvent::UiClick { accent: false }
        } else {
            audio::AudioEvent::UiReject
        });
    }

    /// One step of one row, applied and written; `false` when the value had nowhere to go.
    pub(in crate::app) fn adjust_setting(&mut self, row: SettingsRow, dir: i32) -> bool {
        match row {
            SettingsRow::Palette => {
                let next = ring_step(&Palette::ALL, self.settings.palette(), dir);
                self.edit_settings(|settings| settings.set_palette(next));
                true
            }
            SettingsRow::MasterVolume => {
                let Some(next) =
                    stepped_value(self.settings.master_gain, GAIN_STEP, [0.0, 1.0], dir)
                else {
                    return false;
                };
                self.edit_settings(|settings| settings.master_gain = next);
                true
            }
            SettingsRow::UiScale => {
                let Some(next) =
                    stepped_value(self.settings.ui_scale, UI_SCALE_STEP, UI_SCALE_RANGE, dir)
                else {
                    return false;
                };
                self.edit_settings(|settings| settings.ui_scale = next);
                true
            }
            SettingsRow::SniperKey => {
                self.edit_settings(|settings| settings.sniper_toggle = !settings.sniper_toggle);
                true
            }
            SettingsRow::Fullscreen => {
                self.toggle_fullscreen();
                true
            }
            SettingsRow::Daylight => {
                use scene_build::hangar::HangarLight;
                const RING: [Option<HangarLight>; 4] = [
                    None,
                    Some(HangarLight::Morning),
                    Some(HangarLight::Day),
                    Some(HangarLight::Evening),
                ];
                let next = ring_step(&RING, self.garage.daylight_override(), dir);
                self.garage.set_daylight_override(next);
                self.edit_settings(|settings| settings.set_daylight(next));
                true
            }
            SettingsRow::HudPreset => {
                let next = ring_step(&Preset::ALL, self.hud_layout().preset, dir);
                self.set_preset(next);
                true
            }
            sensitivity => {
                let step = sensitivity.zoom_step().unwrap_or(0);
                let Some(next) = stepped_value(
                    self.settings.sensitivity[step],
                    SENSITIVITY_STEP,
                    SENSITIVITY_RANGE,
                    dir,
                ) else {
                    return false;
                };
                self.edit_settings(|settings| settings.sensitivity[step] = next);
                true
            }
        }
    }
}
