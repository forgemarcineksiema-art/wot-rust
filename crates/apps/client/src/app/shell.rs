//! The shell's pages over the battle (interface program P6, P8): the settings page and the
//! key bindings page, opened from the escape menu, driven by the keys of `Context::Shell`
//! and the mouse; every change is applied at once and written to its file (`settings.json`,
//! `keybinds.json`); Esc goes back to the menu the page came from.

use ui_kit::rect::Rect;
use ui_kit::theme::Palette;
use winit::keyboard::PhysicalKey;

use super::ClientApp;
use super::history::date_word;
use super::keybinds::{Action, Context, KeyBindings, is_bindable, key_label};
use super::results::{ResultsInputs, results_model};
use super::settings::{
    GAIN_STEP, SENSITIVITY_RANGE, SENSITIVITY_STEP, UI_SCALE_RANGE, UI_SCALE_STEP,
};
use crate::hud::elements::ShellPart;
use crate::hud::layout::Preset;
use crate::hud::shell::{
    BattleRow, BattlesScreenModel, KEY_ROWS_VISIBLE, KeyRow, KeybindsScreenModel, MenuItem,
    MenuKind, MenuScreenModel, ResultsTab, SettingsRow, SettingsScreenModel, SettingsView,
    ShellModel, shell_row_index,
};
use crate::hud::shell::{ReplaysScreenModel, StatRow, StatisticsScreenModel};
use crate::ui_strings::battle as words;

/// Which page is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellPage {
    /// The menu: the way in to the other pages and the way out.
    Menu(MenuKind),
    Settings,
    /// The key bindings of one context; row 0 is the context row.
    Keybinds(Context),
    /// The results (P1, P2), one tab at a time; row 0 is the tab row. `stored` is a battle
    /// off the history (P4) rather than the one just fought.
    Results {
        tab: ResultsTab,
        stored: bool,
    },
    /// The battle history (P4), newest first.
    Battles,
    /// The replays (G10, P10's honesty): no viewer, the recording named.
    Replays,
    /// The crew's own numbers over the stored battles (G10).
    Statistics,
}

/// A battle read back from the history for the results page (P4).
#[derive(Debug, Clone)]
pub(crate) struct StoredBattle {
    pub ledger: super::ledger::BattleLedger,
    pub roster: Vec<net::RosterEntry>,
    pub player_team: game_core::TeamId,
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
    /// The stored battle the results page shows (P4), when it is not the one just fought.
    stored: Option<Box<StoredBattle>>,
    /// Opened from a garage tab (G10): Esc returns to the hall, not to a menu.
    from_tab: bool,
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
            from_tab: false,
            stored: None,
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

pub(crate) use super::results::results_footer;

/// The footer of the BATTLES page.
pub(crate) fn battles_footer(keybinds: &KeyBindings) -> String {
    let first = |action: Action| first_key_label(keybinds, action);
    format!(
        "{}/{} {} \u{b7} {} {} \u{b7} {} {}",
        first(Action::MenuUp),
        first(Action::MenuDown),
        words::FOOTER_SELECT,
        first(Action::MenuAccept),
        words::FOOTER_OPEN,
        first(Action::MenuBack),
        words::FOOTER_BACK
    )
}

/// The footer of a menu.
pub(crate) fn menu_footer(keybinds: &KeyBindings) -> String {
    let first = |action: Action| first_key_label(keybinds, action);
    format!(
        "{}/{} {} \u{b7} {} {} \u{b7} {} {}",
        first(Action::MenuUp),
        first(Action::MenuDown),
        words::FOOTER_SELECT,
        first(Action::MenuAccept),
        words::FOOTER_CHOOSE,
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

pub(crate) fn first_key_label(keybinds: &KeyBindings, action: Action) -> String {
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

    /// KEY BINDINGS on the menu: the battle's keys first.
    pub(in crate::app) fn open_keybinds_page(&mut self) {
        self.open_shell_page(ShellPage::Keybinds(Context::Battle));
    }

    /// ESC: the menu over the battle, or the cold garage's own (P8).
    pub(in crate::app) fn open_menu(&mut self, kind: MenuKind) {
        self.open_shell_page(ShellPage::Menu(kind));
    }

    /// The banner's hand-off (P1): the results, SUMMARY first.
    pub(in crate::app) fn open_results_page(&mut self) {
        self.results_shown = true;
        self.open_shell_page(ShellPage::Results { tab: ResultsTab::Summary, stored: false });
    }

    /// BATTLES on the garage's menu (P4): the history, newest first.
    pub(in crate::app) fn open_battles_page(&mut self) {
        self.open_shell_page(ShellPage::Battles);
    }

    /// A garage tab (G10): the page over the hall, and Esc returns to the hall.
    pub(in crate::app) fn open_shell_page_from_tab(&mut self, page: ShellPage) {
        self.open_shell_page(page);
        if let Some(shell) = &mut self.shell {
            shell.from_tab = true;
        }
    }

    /// The page up, for the locks.
    #[cfg(test)]
    pub(in crate::app) fn shell_page(&self) -> Option<ShellPage> {
        self.shell.as_ref().map(|shell| shell.page)
    }

    /// OPEN on a stored battle: the results page over its ledger.
    fn open_stored_results(&mut self) {
        let Some(shell) = &self.shell else { return };
        let Some(record) = self.history.as_ref().and_then(|history| history.read(shell.selected))
        else {
            self.queue_audio(audio::AudioEvent::UiReject);
            return;
        };
        let stored = StoredBattle {
            ledger: record.ledger(),
            roster: record.roster.clone(),
            player_team: record.player_team(),
        };
        let mut state =
            ShellState::open(ShellPage::Results { tab: ResultsTab::Summary, stored: true });
        state.stored = Some(Box::new(stored));
        self.shell = Some(state);
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// CONTINUE on the results: the garage — or, over a stored battle, back to the history.
    fn continue_from_results(&mut self) {
        let stored = matches!(
            self.shell.as_ref().map(|shell| shell.page),
            Some(ShellPage::Results { stored: true, .. })
        );
        self.shell = None;
        self.queue_audio(audio::AudioEvent::UiClick { accent: !stored });
        if stored {
            self.open_battles_page();
        } else {
            self.open_garage();
        }
    }

    fn step_results_tab(&mut self, dir: i32) {
        let Some(shell) = &mut self.shell else { return };
        let ShellPage::Results { tab, stored } = shell.page else { return };
        shell.page = ShellPage::Results { tab: ring_step(&ResultsTab::ALL, tab, dir), stored };
        shell.first_visible = 0;
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    fn battles_model(&self, shell: &ShellState) -> BattlesScreenModel {
        let rows = self.history.as_ref().map_or_else(Vec::new, |history| {
            history
                .entries()
                .iter()
                .map(|entry| BattleRow {
                    date: date_word(entry.ended_at),
                    map: entry.map.clone(),
                    vehicle: entry.vehicle.clone(),
                    outcome: crate::hud::shell::outcome_word(super::history::outcome_from_slug(
                        &entry.outcome,
                    ))
                    .to_string(),
                    kills: entry.kills,
                    damage_dealt: entry.damage_dealt,
                })
                .collect()
        });
        BattlesScreenModel {
            rows,
            selected: shell.selected,
            first_visible: shell.first_visible,
            hovered: shell.hovered,
            footer: battles_footer(&self.keybinds),
        }
    }

    /// The REPLAYS page (G10): no viewer exists (P10), and the recording the session wrote.
    fn replays_model(&self, shell: &ShellState) -> ReplaysScreenModel {
        ReplaysScreenModel {
            reason: words::REPLAY_REASON.to_string(),
            recording: self.session.recording_path(),
            hovered: shell.hovered,
            footer: shell_footer(&self.keybinds),
        }
    }

    /// The STATISTICS page (G10): the crew's own numbers summed over every stored battle —
    /// the same tally the results page prints, the outcomes counted off the index; no XP,
    /// because there is none.
    fn statistics_model(&self, shell: &ShellState) -> StatisticsScreenModel {
        let stat = |label: &str, value: String| StatRow { label: label.to_string(), value };
        let rows = match self.history.as_ref() {
            Some(history) if !history.entries().is_empty() => {
                let mut tally = super::ledger::OwnTally::default();
                let (mut victories, mut defeats, mut draws) = (0u32, 0u32, 0u32);
                for (index, entry) in history.entries().iter().enumerate() {
                    match super::history::outcome_from_slug(&entry.outcome) {
                        crate::hud::BattleHudOutcome::Victory => victories += 1,
                        crate::hud::BattleHudOutcome::Defeat => defeats += 1,
                        crate::hud::BattleHudOutcome::Draw => draws += 1,
                        _ => {}
                    }
                    if let Some(record) = history.read(index) {
                        let own = record.ledger().own();
                        tally.shots += own.shots;
                        tally.hits += own.hits;
                        tally.penetrations += own.penetrations;
                        tally.damage_dealt += own.damage_dealt;
                        tally.damage_taken += own.damage_taken;
                        tally.kills += own.kills;
                        tally.spotted_spans += own.spotted_spans;
                    }
                }
                let rate = |part: u32, whole: u32| {
                    (part * 100 + whole / 2)
                        .checked_div(whole)
                        .map_or_else(|| "-".to_string(), |percent| format!("{percent} %"))
                };
                vec![
                    stat(words::STAT_BATTLES, history.entries().len().to_string()),
                    stat(words::STAT_VICTORIES, victories.to_string()),
                    stat(words::STAT_DEFEATS, defeats.to_string()),
                    stat(words::STAT_DRAWS, draws.to_string()),
                    stat(words::STAT_SHOTS, tally.shots.to_string()),
                    stat(words::STAT_HITS, tally.hits.to_string()),
                    stat(words::STAT_HIT_RATE, rate(tally.hits, tally.shots)),
                    stat(words::STAT_PENETRATIONS, tally.penetrations.to_string()),
                    stat(words::STAT_PEN_RATE, rate(tally.penetrations, tally.hits)),
                    stat(words::STAT_DAMAGE_DEALT, tally.damage_dealt.to_string()),
                    stat(words::STAT_DAMAGE_TAKEN, tally.damage_taken.to_string()),
                    stat(words::STAT_KILLS, tally.kills.to_string()),
                    stat(words::STAT_SPOTTED, tally.spotted_spans.to_string()),
                ]
            }
            _ => Vec::new(),
        };
        StatisticsScreenModel { rows, hovered: shell.hovered, footer: shell_footer(&self.keybinds) }
    }

    /// The results' window one row along, never past the last full window.
    fn scroll_results(&mut self, dir: i32) {
        let rows = self.shell_model().map_or(0, |model| match model {
            ShellModel::Results(page) => page.scroll_rows(),
            _ => 0,
        });
        let Some(shell) = &mut self.shell else { return };
        let last = rows.saturating_sub(KEY_ROWS_VISIBLE) as i32;
        shell.first_visible = (shell.first_visible as i32 + dir).clamp(0, last) as usize;
    }

    fn open_shell_page(&mut self, page: ShellPage) {
        self.command_wheel = None;
        self.input.release_driving();
        self.shell = Some(ShellState::open(page));
        self.set_cursor_captured(false);
    }

    /// Whether a menu is up (not one of its pages); the locks read it, P1's hand-off next.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn menu_open(&self) -> bool {
        matches!(self.shell.as_ref().map(|shell| shell.page), Some(ShellPage::Menu(_)))
    }

    /// The menu the pages go back to: the garage's, or the battle's.
    ///
    /// The question is where the player is STANDING, so it asks whether a battle is still
    /// running — not whether one was ever started. With `has_started()` here, a garage over a
    /// FINISHED battle raised the battle's menu and offered STAY IN BATTLE for a battle that
    /// no longer existed.
    fn menu_kind_here(&self) -> MenuKind {
        if self.garage.is_open() && !self.in_live_battle() {
            MenuKind::Garage
        } else {
            MenuKind::Battle
        }
    }

    /// ESC on the menu, or STAY: the page goes; in the battle the mouse is the gun again and
    /// the motion collected over the buttons is dropped.
    pub(in crate::app) fn close_menu(&mut self) {
        self.shell = None;
        if !self.garage.is_open() {
            self.input.clear_mouse_look();
            self.set_cursor_captured(true);
        }
    }

    /// A menu entry, chosen by key or by click.
    fn activate_menu_item(&mut self, item: MenuItem) {
        match item {
            MenuItem::Stay => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.close_menu();
            }
            MenuItem::Settings => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.open_settings_page();
            }
            MenuItem::Keybinds => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.open_keybinds_page();
            }
            MenuItem::HudEditor => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.open_hud_editor();
            }
            MenuItem::ExitToGarage => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: true });
                self.shell = None;
                self.open_garage();
            }
            MenuItem::Quit => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: true });
                self.shell = None;
                self.quit_requested = true;
            }
            MenuItem::Battles => {
                self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                self.open_battles_page();
            }
        }
    }

    /// QUIT was chosen: the loop leaves on its next turn (the loop reads the field; the
    /// locks read this).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Whether the page up offers a way out — QUIT or EXIT TO GARAGE (the lock's word).
    #[cfg(test)]
    pub(in crate::app) fn way_out_offered(&self) -> bool {
        match self.shell.as_ref().map(|shell| shell.page) {
            Some(ShellPage::Menu(kind)) => kind.items().iter().any(|item| item.is_way_out()),
            _ => false,
        }
    }

    /// The part under the cursor, for the locks.
    #[cfg(test)]
    pub(in crate::app) fn shell_hovered(&self) -> Option<ShellPart> {
        self.shell.as_ref().and_then(|shell| shell.hovered)
    }

    /// Stage the page's parts as a frame would, for the mouse locks.
    #[cfg(test)]
    pub(in crate::app) fn stage_shell_hits(&mut self) {
        let Some(page) = self.shell_model() else { return };
        let ui = ui_kit::ui::Ui::new(1920, 1080, 1.0);
        let theme = ui_kit::theme::Theme::standard();
        let mut list = ui_kit::draw_list::DrawList::new();
        let mut order: i16 = 0;
        crate::hud::shell::push_shell(&mut list, &ui, &theme, &page, &mut order);
        self.remember_shell_hits(crate::hud::shell::shell_hit_rects(&list));
    }

    /// Click a menu entry as the mouse would: the cursor over its plate, then the press.
    #[cfg(test)]
    pub(in crate::app) fn click_menu_item(&mut self, item: MenuItem) {
        let Some(ShellPage::Menu(kind)) = self.shell.as_ref().map(|shell| shell.page) else {
            panic!("no menu is up")
        };
        let index = kind.items().iter().position(|it| *it == item).expect("the entry") as u8;
        self.stage_shell_hits();
        let rect = self
            .shell
            .as_ref()
            .and_then(|shell| {
                shell
                    .hits
                    .iter()
                    .find(|(part, _)| *part == ShellPart::RowPlate(index))
                    .map(|(_, r)| *r)
            })
            .expect("the entry's plate");
        self.shell_cursor(rect.center());
        self.shell_press();
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

    /// Esc on a page: back to the menu it came from.
    pub(in crate::app) fn close_shell(&mut self) {
        if let Some(shell) = self.shell.take() {
            // G10: a page a tab opened goes back to the hall it was opened over.
            if shell.from_tab {
                return;
            }
            let kind = self.menu_kind_here();
            self.shell = Some(ShellState::open(ShellPage::Menu(kind)));
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
            ShellPage::Menu(kind) => ShellModel::Menu(MenuScreenModel {
                kind,
                selected: shell.selected,
                hovered: shell.hovered,
                footer: menu_footer(&self.keybinds),
            }),
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
            ShellPage::Results { tab, .. } => match &shell.stored {
                Some(stored) => ShellModel::Results(results_model(ResultsInputs {
                    ledger: &stored.ledger,
                    roster: &stored.roster,
                    player_team: stored.player_team,
                    recording: None,
                    tab,
                    first_visible: shell.first_visible,
                    hovered: shell.hovered,
                    footer: results_footer(&self.keybinds),
                })),
                None => {
                    let roster = self.session.roster();
                    ShellModel::Results(results_model(ResultsInputs {
                        ledger: &self.ledger,
                        roster: &roster,
                        player_team: self.player_team(),
                        recording: self.session.recording_path(),
                        tab,
                        first_visible: shell.first_visible,
                        hovered: shell.hovered,
                        footer: results_footer(&self.keybinds),
                    }))
                }
            },
            ShellPage::Battles => ShellModel::Battles(self.battles_model(shell)),
            ShellPage::Replays => ShellModel::Replays(self.replays_model(shell)),
            ShellPage::Statistics => ShellModel::Statistics(self.statistics_model(shell)),
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
            ShellPage::Menu(kind) => match action {
                Action::MenuUp => self.select_row(-1, kind.items().len()),
                Action::MenuDown => self.select_row(1, kind.items().len()),
                Action::MenuAccept => {
                    let item = kind.items()[self.selected_row().min(kind.items().len() - 1)];
                    self.activate_menu_item(item);
                }
                Action::MenuBack => self.close_menu(),
                _ => {}
            },
            ShellPage::Settings => match action {
                Action::MenuUp => self.select_row(-1, SettingsRow::ALL.len()),
                Action::MenuDown => self.select_row(1, SettingsRow::ALL.len()),
                Action::MenuLeft => self.adjust_selected_setting(-1),
                Action::MenuRight | Action::MenuAccept => self.adjust_selected_setting(1),
                Action::MenuBack => self.close_shell(),
                _ => {}
            },
            ShellPage::Battles => {
                let rows = self.history.as_ref().map_or(0, |history| history.entries().len());
                match action {
                    Action::MenuUp if rows > 0 => self.select_row(-1, rows),
                    Action::MenuDown if rows > 0 => self.select_row(1, rows),
                    Action::MenuAccept if rows > 0 => self.open_stored_results(),
                    Action::MenuBack => self.close_shell(),
                    _ => {}
                }
            }
            // G10: pages for reading — Esc is their one key.
            ShellPage::Replays | ShellPage::Statistics => {
                if action == Action::MenuBack {
                    self.close_shell();
                }
            }
            ShellPage::Results { .. } => match action {
                Action::MenuLeft => self.step_results_tab(-1),
                Action::MenuRight => self.step_results_tab(1),
                Action::MenuUp => self.scroll_results(-1),
                Action::MenuDown => self.scroll_results(1),
                Action::MenuAccept => self.continue_from_results(),
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

    /// The selection one row along, wrapping; a scrolling page's window follows it (the key
    /// bindings page keeps its context row at the top; the history's rows start at zero).
    fn select_row(&mut self, dir: i32, rows: usize) {
        let Some(shell) = &mut self.shell else { return };
        shell.selected = (shell.selected as i32 + dir).rem_euclid(rows as i32) as usize;
        shell.listening = None;
        let header = usize::from(!matches!(shell.page, ShellPage::Battles));
        if shell.selected < header {
            shell.first_visible = 0;
        } else {
            let row = shell.selected - header;
            if row < shell.first_visible {
                shell.first_visible = row;
            } else if row >= shell.first_visible + KEY_ROWS_VISIBLE {
                shell.first_visible = row + 1 - KEY_ROWS_VISIBLE;
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
            // A click on an entry chooses it; off every entry it does nothing — a menu that
            // closed on a stray click would drop the player back without an answer.
            ShellPage::Menu(kind) => {
                if let Some(item) = kind.items().get(row as usize) {
                    self.activate_menu_item(*item);
                }
            }
            // The history: a click selects a battle; a second on the selected one opens it.
            ShellPage::Battles => {
                let index = first_visible + row as usize;
                let rows = self.history.as_ref().map_or(0, |history| history.entries().len());
                if index >= rows {
                    return;
                }
                let already = self.shell.as_ref().is_some_and(|shell| shell.selected == index);
                if let Some(shell) = &mut self.shell {
                    shell.selected = index;
                }
                if already {
                    self.open_stored_results();
                } else {
                    self.queue_audio(audio::AudioEvent::UiClick { accent: false });
                }
            }
            // The results: the tab row's arrows step the tab; the rows are for reading.
            ShellPage::Results { .. } => match (row, part) {
                (0, ShellPart::RowDec(_)) => self.step_results_tab(-1),
                (0, ShellPart::RowInc(_)) => self.step_results_tab(1),
                _ => {}
            },
            // G10: nothing on these pages answers a click.
            ShellPage::Replays | ShellPage::Statistics => {}
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
