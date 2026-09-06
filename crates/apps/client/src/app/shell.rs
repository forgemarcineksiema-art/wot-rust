//! The shell's pages over the battle (interface program P6, P8): the settings page and the
//! key bindings page, opened from the escape menu, driven by the keys of `Context::Shell`
//! and the mouse; every change is applied at once and written to its file (`settings.json`,
//! `keybinds.json`); Esc goes back to the menu the page came from.

use ui_kit::rect::Rect;
use ui_kit::theme::Palette;
use winit::keyboard::PhysicalKey;

use super::ClientApp;
use super::keybinds::{Action, Context, KeyBindings, is_bindable, key_label};
use super::settings::{
    GAIN_STEP, SENSITIVITY_RANGE, SENSITIVITY_STEP, UI_SCALE_RANGE, UI_SCALE_STEP,
};
use crate::hud::elements::ShellPart;
use crate::hud::layout::Preset;
use crate::hud::shell::{
    KEY_ROWS_VISIBLE, KeyRow, KeybindsScreenModel, SettingsRow, SettingsScreenModel, SettingsView,
    ShellModel, shell_row_index,
};
use crate::ui_strings::battle as words;

/// Which page is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellPage {
    Settings,
    /// The key bindings of one context; row 0 is the context row.
    Keybinds(Context),
}

/// The page's live state: the selected row, the window's first action (the key bindings
/// page scrolls), the row listening for a key, the part under the cursor, and the parts of
/// the last built frame for the mouse.
#[derive(Debug, Clone)]
pub(crate) struct ShellState {
    page: ShellPage,
    selected: usize,
    first_visible: usize,
    listening: Option<Action>,
    hovered: Option<ShellPart>,
    hits: Vec<(ShellPart, Rect)>,
}

impl ShellState {
    fn open(page: ShellPage) -> Self {
        Self {
            page,
            selected: 0,
            first_visible: 0,
            listening: None,
            hovered: None,
            hits: Vec::new(),
        }
    }
}

/// The footer of the settings page: the table's keys for the shell's words, never literals.
pub(crate) fn shell_footer(keybinds: &KeyBindings) -> String {
    let first = |action: Action| first_key_label(keybinds, action);
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

/// The footer of the key bindings page.
pub(crate) fn keybinds_footer(keybinds: &KeyBindings) -> String {
    let first = |action: Action| first_key_label(keybinds, action);
    format!(
        "{}/{} {} \u{b7} {} {} \u{b7} {} {} \u{b7} {}/{} {} \u{b7} {} {}",
        first(Action::MenuUp),
        first(Action::MenuDown),
        words::FOOTER_SELECT,
        first(Action::MenuAccept),
        words::FOOTER_REBIND,
        first(Action::MenuReset),
        words::FOOTER_RESET,
        first(Action::MenuLeft),
        first(Action::MenuRight),
        words::FOOTER_CONTEXT,
        first(Action::MenuBack),
        words::FOOTER_BACK
    )
}

fn first_key_label(keybinds: &KeyBindings, action: Action) -> String {
    keybinds.keys(action).first().map_or_else(|| "-".to_string(), |key| key_label(*key))
}

/// The actions of one context, in the table's order.
pub(crate) fn actions_of(context: Context) -> Vec<Action> {
    Action::ALL.into_iter().filter(|action| action.context() == context).collect()
}

/// The key bindings page for one context off the table: every action with its keys, the
/// other action it shares a key with, the listening row.
pub(crate) fn keybinds_page_model(
    keybinds: &KeyBindings,
    context: Context,
    selected: usize,
    first_visible: usize,
    listening: Option<Action>,
    hovered: Option<ShellPart>,
) -> KeybindsScreenModel {
    let conflicts = keybinds.conflicts();
    let rows = actions_of(context)
        .into_iter()
        .map(|action| {
            let keys = keybinds.keys(action).iter().map(|key| key_label(*key)).collect::<Vec<_>>();
            let conflict = conflicts
                .iter()
                .filter(|conflict| {
                    conflict.context == context && conflict.actions.contains(&action)
                })
                .map(|conflict| {
                    let other = if conflict.actions[0] == action {
                        conflict.actions[1]
                    } else {
                        conflict.actions[0]
                    };
                    other.name()
                })
                .next();
            KeyRow {
                action,
                name: action.name(),
                keys: keys.join(" / "),
                conflict,
                listening: listening == Some(action),
                changed: keybinds.keys(action) != action.default_keys(),
            }
        })
        .collect();
    KeybindsScreenModel {
        context,
        rows,
        selected,
        first_visible,
        hovered,
        footer: keybinds_footer(keybinds),
    }
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
        self.open_shell_page(ShellPage::Settings);
    }

    /// KEY BINDINGS on the escape menu: the battle's keys first.
    pub(in crate::app) fn open_keybinds_page(&mut self) {
        self.open_shell_page(ShellPage::Keybinds(Context::Battle));
    }

    fn open_shell_page(&mut self, page: ShellPage) {
        self.pause_menu = None;
        self.command_wheel = None;
        self.input.release_driving();
        self.shell = Some(ShellState::open(page));
        self.set_cursor_captured(false);
    }

    /// The row listening for its next key, if any: while it listens, every key is its.
    pub(in crate::app) fn shell_listening(&self) -> Option<Action> {
        self.shell.as_ref().and_then(|shell| shell.listening)
    }

    /// The key a listening row takes: the table's alphabet binds and is written; the
    /// menu's BACK key cancels; anything else is refused.
    pub(in crate::app) fn shell_take_key(&mut self, key: PhysicalKey) {
        let Some(action) = self.shell_listening() else { return };
        if self.keybinds.action(Context::Shell, key) == Some(Action::MenuBack) {
            self.stop_listening();
            self.queue_audio(audio::AudioEvent::UiClick { accent: false });
            return;
        }
        match key {
            PhysicalKey::Code(code) if is_bindable(code) => {
                self.rebind(action, code);
                self.stop_listening();
                self.queue_audio(audio::AudioEvent::UiClick { accent: true });
            }
            _ => self.queue_audio(audio::AudioEvent::UiReject),
        }
    }

    fn stop_listening(&mut self) {
        if let Some(shell) = &mut self.shell {
            shell.listening = None;
        }
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
        Some(match shell.page {
            ShellPage::Settings => ShellModel::Settings(SettingsScreenModel {
                view: self.settings_view(),
                selected: shell.selected,
                hovered: shell.hovered,
                footer: shell_footer(&self.keybinds),
            }),
            ShellPage::Keybinds(context) => ShellModel::Keybinds(keybinds_page_model(
                &self.keybinds,
                context,
                shell.selected,
                shell.first_visible,
                shell.listening,
                shell.hovered,
            )),
        })
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
        let Some(page) = self.shell.as_ref().map(|shell| shell.page) else { return };
        match page {
            ShellPage::Settings => match action {
                Action::MenuUp => self.select_row(-1, SettingsRow::ALL.len()),
                Action::MenuDown => self.select_row(1, SettingsRow::ALL.len()),
                Action::MenuLeft => self.adjust_selected_setting(-1),
                Action::MenuRight | Action::MenuAccept => self.adjust_selected_setting(1),
                Action::MenuBack => self.close_shell(),
                _ => {}
            },
            ShellPage::Keybinds(context) => {
                let rows = 1 + actions_of(context).len();
                match action {
                    Action::MenuUp => self.select_row(-1, rows),
                    Action::MenuDown => self.select_row(1, rows),
                    Action::MenuLeft if self.selected_row() == 0 => self.step_key_context(-1),
                    Action::MenuRight | Action::MenuAccept if self.selected_row() == 0 => {
                        self.step_key_context(1)
                    }
                    Action::MenuAccept => self.listen_on_selected_row(),
                    Action::MenuReset => self.reset_selected_row(),
                    Action::MenuBack => self.close_shell(),
                    _ => {}
                }
            }
        }
    }

    fn selected_row(&self) -> usize {
        self.shell.as_ref().map_or(0, |shell| shell.selected)
    }

    /// The selection one row along, wrapping; the key bindings page's window follows it.
    fn select_row(&mut self, dir: i32, rows: usize) {
        let Some(shell) = &mut self.shell else { return };
        shell.selected = (shell.selected as i32 + dir).rem_euclid(rows as i32) as usize;
        shell.listening = None;
        if shell.selected == 0 {
            shell.first_visible = 0;
        } else {
            let action = shell.selected - 1;
            if action < shell.first_visible {
                shell.first_visible = action;
            } else if action >= shell.first_visible + KEY_ROWS_VISIBLE {
                shell.first_visible = action + 1 - KEY_ROWS_VISIBLE;
            }
        }
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// The context row's arrows: the next context's keys, from its top.
    fn step_key_context(&mut self, dir: i32) {
        let Some(shell) = &mut self.shell else { return };
        let ShellPage::Keybinds(context) = shell.page else { return };
        shell.page = ShellPage::Keybinds(ring_step(&Context::ALL, context, dir));
        shell.selected = 0;
        shell.first_visible = 0;
        shell.listening = None;
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    fn selected_key_action(&self) -> Option<Action> {
        let shell = self.shell.as_ref()?;
        let ShellPage::Keybinds(context) = shell.page else { return None };
        actions_of(context).get(shell.selected.checked_sub(1)?).copied()
    }

    fn listen_on_selected_row(&mut self) {
        let Some(action) = self.selected_key_action() else { return };
        if let Some(shell) = &mut self.shell {
            shell.listening = Some(action);
        }
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    fn reset_selected_row(&mut self) {
        let Some(action) = self.selected_key_action() else { return };
        self.reset_binding(action);
        self.stop_listening();
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

    /// A click on the page: on the settings page a row's arrow steps that row and anything
    /// else of a row selects it; on the key bindings page the context row's arrows step the
    /// context, a row's label selects it and its keys cell starts it listening.
    pub(in crate::app) fn shell_press(&mut self) {
        let Some(shell) = self.shell.as_ref() else { return };
        let Some(part) = shell.hovered else { return };
        let Some(row) = shell_row_index(part) else { return };
        let page = shell.page;
        let first_visible = shell.first_visible;
        match page {
            ShellPage::Settings => {
                if let Some(shell) = &mut self.shell {
                    shell.selected = row as usize;
                }
                match part {
                    ShellPart::RowDec(_) => self.adjust_selected_setting(-1),
                    ShellPart::RowInc(_) => self.adjust_selected_setting(1),
                    _ => self.queue_audio(audio::AudioEvent::UiClick { accent: false }),
                }
            }
            ShellPage::Keybinds(_) => {
                if row == 0 {
                    match part {
                        ShellPart::RowDec(_) => self.step_key_context(-1),
                        ShellPart::RowInc(_) => self.step_key_context(1),
                        _ => {
                            if let Some(shell) = &mut self.shell {
                                shell.selected = 0;
                            }
                            self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                        }
                    }
                    return;
                }
                if let Some(shell) = &mut self.shell {
                    shell.selected = first_visible + row as usize;
                    shell.listening = None;
                }
                match part {
                    ShellPart::RowGlass(_) | ShellPart::RowValue(_) => {
                        self.listen_on_selected_row()
                    }
                    _ => self.queue_audio(audio::AudioEvent::UiClick { accent: false }),
                }
            }
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
