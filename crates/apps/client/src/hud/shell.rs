//! The shell's pages as draw lists (interface program P6, P8): the settings page — one
//! brushed plate, a row per setting, the value under glass between its two arrows, a bar
//! where the value sits on a range — and the key bindings page — one context at a time, an
//! action per row with its keys under glass, a row that LISTENS, a shared key named on both
//! rows — and the menu itself: the escape menu over the battle and the cold garage's own, a
//! column of buttons; the footer of each names the keys from the table (P7). Drawn INSTEAD of
//! the battle's instruments while open: a page the player opened to read, over a scrim, the
//! battle still moving behind it. The menu is the way in and the way out.

use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload, WidgetState};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::{Palette, Theme};
use ui_kit::ui::{Anchor, Ui};

use super::BattleHudOutcome;
use super::elements::{HudElement, ShellPart};
use super::layout::Preset;
use crate::app::keybinds::{Action, Context};
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
/// The key bindings page: the note's width and the rows on one plate (the rest scroll).
const NOTE_W_U: f32 = 210.0;
pub const KEY_ROWS_VISIBLE: usize = 14;
/// The menu: a narrower plate, taller buttons.
const MENU_W_U: f32 = 480.0;
const MENU_ROW_H_U: f32 = 44.0;
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

/// One action on the key bindings page (P8).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyRow {
    pub action: Action,
    pub name: String,
    /// The keys, labelled, `/`-joined.
    pub keys: String,
    /// The other action of this context sharing a key, by name.
    pub conflict: Option<String>,
    pub listening: bool,
    /// Not the default.
    pub changed: bool,
}

/// The key bindings page as the HUD draws it: one context, its actions, a window of
/// `KEY_ROWS_VISIBLE` rows from `first_visible`; row 0 is the context row.
#[derive(Debug, Clone, PartialEq)]
pub struct KeybindsScreenModel {
    pub context: Context,
    pub rows: Vec<KeyRow>,
    pub selected: usize,
    pub first_visible: usize,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

/// A menu's entries (P8). Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuItem {
    Stay,
    Settings,
    Keybinds,
    HudEditor,
    ExitToGarage,
    Quit,
    /// The battle history (P4).
    Battles,
}

impl MenuItem {
    /// Every entry, for the locks: each one sits on some menu.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [MenuItem; 7] = [
        MenuItem::Stay,
        MenuItem::Settings,
        MenuItem::Keybinds,
        MenuItem::HudEditor,
        MenuItem::ExitToGarage,
        MenuItem::Quit,
        MenuItem::Battles,
    ];

    pub fn word(self) -> &'static str {
        match self {
            MenuItem::Stay => words::PAUSE_STAY,
            MenuItem::Settings => words::PAUSE_SETTINGS,
            MenuItem::Keybinds => words::PAUSE_KEYBINDS,
            MenuItem::HudEditor => words::PAUSE_HUD_EDITOR,
            MenuItem::ExitToGarage => words::PAUSE_EXIT_TO_GARAGE,
            MenuItem::Quit => words::MENU_QUIT,
            MenuItem::Battles => words::MENU_BATTLES,
        }
    }

    /// The commits — leaving the battle, leaving the game — wear the one red.
    pub fn is_commit(self) -> bool {
        matches!(self, MenuItem::ExitToGarage | MenuItem::Quit)
    }

    /// A way out of the GAME: the lock's word.
    ///
    /// This used to be `self.is_commit()`, so EXIT TO GARAGE answered it — and that is exactly
    /// why `escape_always_offers_a_way_out` stayed green through the whole 2026-09-09 defect,
    /// when no menu a started session could raise offered QUIT at all. Leaving the battle is not
    /// leaving the game; the policy's promise (`docs/interface-policy.md`) is the latter.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_way_out(self) -> bool {
        matches!(self, MenuItem::Quit)
    }
}

/// Which menu: the escape menu over a live battle, or the cold garage's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuKind {
    Battle,
    Garage,
}

impl MenuKind {
    /// The entries, top to bottom. Nothing destructive sits first: the keyboard's selection
    /// starts on SETTINGS.
    pub fn items(self) -> &'static [MenuItem] {
        match self {
            MenuKind::Battle => &[
                MenuItem::Settings,
                MenuItem::Keybinds,
                MenuItem::HudEditor,
                MenuItem::ExitToGarage,
                // QUIT belongs here too. It used to sit ONLY on the cold garage's menu, and
                // `started` never returns to false once BATTLE is pressed — so after the first
                // battle of a session the player could not leave the game from inside it at
                // all: the battle menu offered no QUIT, and Esc in the garage went back to the
                // battle instead of raising a menu. The window's X was the only way out.
                MenuItem::Quit,
                MenuItem::Stay,
            ],
            MenuKind::Garage => {
                &[MenuItem::Settings, MenuItem::Keybinds, MenuItem::Battles, MenuItem::Quit]
            }
        }
    }
}

/// The menu as the HUD draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuScreenModel {
    pub kind: MenuKind,
    pub selected: usize,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

/// The results page's tabs (P1, P2). Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultsTab {
    Summary,
    Timeline,
    Team,
}

impl ResultsTab {
    pub const ALL: [ResultsTab; 3] = [ResultsTab::Summary, ResultsTab::Timeline, ResultsTab::Team];

    pub fn word(self) -> &'static str {
        match self {
            ResultsTab::Summary => words::TAB_SUMMARY,
            ResultsTab::Timeline => words::TAB_TIMELINE,
            ResultsTab::Team => words::TAB_TEAM,
        }
    }
}

/// One of the crew's own numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct StatRow {
    pub label: String,
    pub value: String,
}

/// What backs a timeline row on the wire (P2, P3): a stamped damage event, a kill, an own
/// shot's shell, an own spotted span, an observer's span from the end word — never nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowSource {
    Damage(game_core::BattleEventId),
    Kill { victim: game_core::TankId, tick: u64 },
    Shot(game_core::ShellId),
    Spotted { from_tick: u64 },
    Observer { observer: game_core::TankId, from_tick: u64 },
}

/// One row of the timeline: the clock, the word, the detail, and what backs it.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineRow {
    pub tick: u64,
    pub clock: String,
    pub word: String,
    pub detail: String,
    pub source: RowSource,
}

/// One hull of the roster on the TEAM tab: its name, its side, its crew, whether it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamRow {
    pub name: String,
    pub ally: bool,
    pub human: bool,
    pub destroyed: bool,
    pub player: bool,
}

/// The REPLAY button's honesty (P10): disabled, with the reason, and the recording's path
/// when the session wrote one.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayNote {
    pub reason: String,
    pub recording: Option<String>,
}

/// The results page as the HUD draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultsScreenModel {
    pub outcome: BattleHudOutcome,
    /// The crew's hull: "T-54 · B".
    pub hull: String,
    pub stats: Vec<StatRow>,
    pub rows: Vec<TimelineRow>,
    pub team: Vec<TeamRow>,
    pub replay: ReplayNote,
    pub tab: ResultsTab,
    pub first_visible: usize,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

impl ResultsScreenModel {
    /// The rows the current tab scrolls through.
    pub fn scroll_rows(&self) -> usize {
        match self.tab {
            ResultsTab::Summary => 0,
            ResultsTab::Timeline => self.rows.len(),
            ResultsTab::Team => self.team.len(),
        }
    }
}

/// The outcome's word on the results page.
pub fn outcome_word(outcome: BattleHudOutcome) -> &'static str {
    match outcome {
        BattleHudOutcome::Victory => words::VICTORY,
        BattleHudOutcome::Defeat => words::DEFEAT,
        BattleHudOutcome::Draw => words::DRAW,
        BattleHudOutcome::ConnectionLost => words::CONNECTION_LOST,
        BattleHudOutcome::BattleOver => words::BATTLE_OVER,
    }
}

/// One stored battle on the BATTLES page (P4): its day, its map, its hull, its outcome, the
/// crew's two numbers off the index.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleRow {
    pub date: String,
    pub map: String,
    pub vehicle: String,
    pub outcome: String,
    pub kills: u32,
    pub damage_dealt: u32,
}

/// The BATTLES page as the HUD draws it: the history newest first, a window of rows.
#[derive(Debug, Clone, PartialEq)]
pub struct BattlesScreenModel {
    pub rows: Vec<BattleRow>,
    pub selected: usize,
    pub first_visible: usize,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

/// The shell page over the battle, if one is open.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellModel {
    Settings(SettingsScreenModel),
    Keybinds(KeybindsScreenModel),
    Menu(MenuScreenModel),
    Results(ResultsScreenModel),
    Battles(BattlesScreenModel),
    Replays(ReplaysScreenModel),
    Statistics(StatisticsScreenModel),
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
        | ShellPart::RowInc(i)
        | ShellPart::RowNote(i) => Some(i),
        ShellPart::Scrim
        | ShellPart::Panel
        | ShellPart::Title
        | ShellPart::Subtitle
        | ShellPart::Rule
        | ShellPart::Footer
        | ShellPart::ScrollBar => None,
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
        ShellModel::Keybinds(page) => push_keybinds(list, ui, theme, page, z),
        ShellModel::Menu(page) => push_menu(list, ui, theme, page, z),
        ShellModel::Results(page) => push_results(list, ui, theme, page, z),
        ShellModel::Battles(page) => push_battles(list, ui, theme, page, z),
        ShellModel::Replays(page) => push_replays(list, ui, theme, page, z),
        ShellModel::Statistics(page) => push_statistics(list, ui, theme, page, z),
    }
}

/// The REPLAYS page (G10, P10's honesty): no viewer exists, and the page says so — and names
/// the recording the session wrote, when it wrote one.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplaysScreenModel {
    pub reason: String,
    pub recording: Option<String>,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

/// The STATISTICS page (G10): the crew's OWN numbers summed over the stored battles (P5) —
/// nothing a ledger did not hold, no XP.
#[derive(Debug, Clone, PartialEq)]
pub struct StatisticsScreenModel {
    pub rows: Vec<StatRow>,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

/// The REPLAYS page: two rows — the viewer that does not exist, with its reason; the
/// recording on disk, or that there is none.
fn push_replays(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &ReplaysScreenModel,
    z: &mut i16,
) {
    let (title, row_top, panel) = push_page_frame(list, ui, theme, words::REPLAYS_TITLE, 2, z);
    let row_rect = |visible: usize| {
        Rect::new(
            title.x,
            row_top + visible as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(ROW_H_U),
        )
    };
    let viewer = row_rect(0);
    push_row_plate(
        list,
        ui,
        theme,
        0,
        viewer,
        WidgetState::Disabled,
        words::REPLAY,
        theme.text.label,
        z,
    );
    push_value_cell(list, ui, theme, 0, viewer, 6.0, &page.reason, theme.text.label, z);
    let recording = row_rect(1);
    let hovered_row = page.hovered.and_then(shell_row_index);
    push_row_plate(
        list,
        ui,
        theme,
        1,
        recording,
        row_state(false, hovered_row == Some(1)),
        words::RECORDED_TO,
        theme.text.label,
        z,
    );
    let note =
        Rect::new(recording.x + ui.px(200.0), recording.y, recording.w - ui.px(208.0), recording.h);
    shell_put(
        list,
        z,
        ShellPart::RowNote(1),
        note,
        shell_text(
            page.recording.as_deref().unwrap_or(words::NO_RECORDING),
            Style::VALUE,
            16.0,
            Align::Right,
            theme.text.value,
            DigitMode::Proportional,
        ),
        WidgetState::Idle,
    );
    push_footer(list, ui, theme, panel, title, &page.footer, z);
}

/// The STATISTICS page: a row per number, label and value; an empty history says so.
fn push_statistics(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &StatisticsScreenModel,
    z: &mut i16,
) {
    let rows = page.rows.len().clamp(1, KEY_ROWS_VISIBLE);
    let (title, row_top, panel) =
        push_page_frame(list, ui, theme, words::STATISTICS_TITLE, rows, z);
    let row_rect = |visible: usize| {
        Rect::new(
            title.x,
            row_top + visible as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(ROW_H_U),
        )
    };
    if page.rows.is_empty() {
        push_row_plate(
            list,
            ui,
            theme,
            0,
            row_rect(0),
            WidgetState::Disabled,
            words::NO_BATTLES,
            theme.text.label,
            z,
        );
    }
    let hovered_row = page.hovered.and_then(shell_row_index);
    for (i, stat) in page.rows.iter().take(KEY_ROWS_VISIBLE).enumerate() {
        let index = i as u8;
        let rect = row_rect(i);
        push_row_plate(
            list,
            ui,
            theme,
            index,
            rect,
            row_state(false, hovered_row == Some(index)),
            &stat.label,
            theme.text.label,
            z,
        );
        push_value_cell(list, ui, theme, index, rect, 6.0, &stat.value, theme.text.value, z);
    }
    push_footer(list, ui, theme, panel, title, &page.footer, z);
}

/// The REPLAYS page as the golden stages it: no viewer, a recording named.
pub(crate) fn demo_replays_screen() -> ShellModel {
    ShellModel::Replays(ReplaysScreenModel {
        reason: words::REPLAY_REASON.to_string(),
        recording: Some("C:/wot/records/2026-09-06_prokhorovka.wotrec".to_string()),
        hovered: None,
        footer: crate::app::shell::shell_footer(&crate::app::keybinds::KeyBindings::default()),
    })
}

/// The STATISTICS page as the golden stages it: a week of battles summed.
pub(crate) fn demo_statistics_screen() -> ShellModel {
    let stat =
        |label: &str, value: &str| StatRow { label: label.to_string(), value: value.to_string() };
    ShellModel::Statistics(StatisticsScreenModel {
        rows: vec![
            stat(words::STAT_BATTLES, "6"),
            stat(words::STAT_VICTORIES, "3"),
            stat(words::STAT_DEFEATS, "2"),
            stat(words::STAT_DRAWS, "1"),
            stat(words::STAT_SHOTS, "142"),
            stat(words::STAT_HITS, "97"),
            stat(words::STAT_HIT_RATE, "68 %"),
            stat(words::STAT_PENETRATIONS, "61"),
            stat(words::STAT_PEN_RATE, "63 %"),
            stat(words::STAT_DAMAGE_DEALT, "8 460"),
            stat(words::STAT_DAMAGE_TAKEN, "6 110"),
            stat(words::STAT_KILLS, "11"),
            stat(words::STAT_SPOTTED, "23"),
        ],
        hovered: None,
        footer: crate::app::shell::shell_footer(&crate::app::keybinds::KeyBindings::default()),
    })
}

/// The BATTLES page: one row per stored battle, newest first; the selected one is lit;
/// an empty history says so.
fn push_battles(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &BattlesScreenModel,
    z: &mut i16,
) {
    let (title, row_top, panel) =
        push_page_frame(list, ui, theme, words::BATTLES_TITLE, KEY_ROWS_VISIBLE, z);
    let row_rect = |visible: usize| {
        Rect::new(
            title.x,
            row_top + visible as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(ROW_H_U),
        )
    };
    let hovered_row = page.hovered.and_then(shell_row_index);
    if page.rows.is_empty() {
        push_row_plate(
            list,
            ui,
            theme,
            0,
            row_rect(0),
            WidgetState::Disabled,
            words::NO_BATTLES,
            theme.text.label,
            z,
        );
    }
    for (visible, row) in
        page.rows.iter().skip(page.first_visible).take(KEY_ROWS_VISIBLE).enumerate()
    {
        let index = visible as u8;
        let rect = row_rect(visible);
        let selected = page.selected == page.first_visible + visible;
        push_row_plate(
            list,
            ui,
            theme,
            index,
            rect,
            row_state(selected, hovered_row == Some(index)),
            &format!("{} \u{b7} {} \u{b7} {}", row.date, row.map, row.vehicle),
            if selected { theme.lamp } else { theme.text.label },
            z,
        );
        push_value_cell(
            list,
            ui,
            theme,
            index,
            rect,
            6.0 + NOTE_W_U,
            &row.outcome,
            theme.text.value,
            z,
        );
        let note = Rect::new(rect.right() - ui.px(NOTE_W_U), rect.y, ui.px(NOTE_W_U - 8.0), rect.h);
        shell_put(
            list,
            z,
            ShellPart::RowNote(index),
            note,
            shell_text(
                &format!(
                    "{} {} \u{b7} {} {}",
                    row.kills,
                    words::STAT_KILLS,
                    row.damage_dealt,
                    words::DMG_UNIT
                ),
                Style::VALUE_STRONG,
                16.0,
                Align::Right,
                theme.text.label,
                DigitMode::Tabular,
            ),
            WidgetState::Idle,
        );
    }
    let track_top = row_rect(0).y;
    let track_h = row_rect(KEY_ROWS_VISIBLE - 1).bottom() - track_top;
    push_thumb(
        list,
        ui,
        theme,
        panel,
        track_top,
        track_h,
        page.first_visible,
        KEY_ROWS_VISIBLE,
        page.rows.len(),
        z,
    );
    push_footer(list, ui, theme, panel, title, &page.footer, z);
}

/// A scroll thumb for a window of `visible` rows over `total`, from `first`.
#[allow(clippy::too_many_arguments)]
fn push_thumb(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    panel: Rect,
    track_top: f32,
    track_h: f32,
    first: usize,
    visible: usize,
    total: usize,
    z: &mut i16,
) {
    if total <= visible {
        return;
    }
    let thumb_h = track_h * visible as f32 / total as f32;
    let thumb_y = track_top + track_h * first as f32 / total as f32;
    let thumb = Rect::new(panel.right() - ui.px(PAD_U * 0.5 + 2.0), thumb_y, ui.px(4.0), thumb_h);
    shell_put(
        list,
        z,
        ShellPart::ScrollBar,
        thumb,
        Payload::Bar { frac: 0.0, fill: theme.lamp, back: theme.lamp },
        WidgetState::Idle,
    );
}

/// The results page: the outcome beside the title, a tab row, and the tab's rows.
fn push_results(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &ResultsScreenModel,
    z: &mut i16,
) {
    let (title, row_top, panel) =
        push_page_frame(list, ui, theme, words::RESULTS_TITLE, 1 + KEY_ROWS_VISIBLE, z);
    shell_put(
        list,
        z,
        ShellPart::Subtitle,
        title,
        shell_text(
            &format!("{} \u{b7} {}", outcome_word(page.outcome), page.hull),
            Style::BANNER,
            24.0,
            Align::Right,
            theme.lamp,
            DigitMode::Proportional,
        ),
        WidgetState::Idle,
    );
    let row_rect = |visible: usize| {
        Rect::new(
            title.x,
            row_top + visible as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(ROW_H_U),
        )
    };
    let hovered_row = page.hovered.and_then(shell_row_index);
    // Row 0: the tab, stepped by its arrows.
    let tab_rect = row_rect(0);
    push_row_plate(
        list,
        ui,
        theme,
        0,
        tab_rect,
        row_state(false, hovered_row == Some(0)),
        words::TAB_WORD,
        theme.text.label,
        z,
    );
    let cell = push_value_cell(
        list,
        ui,
        theme,
        0,
        tab_rect,
        6.0 + ARROW_W_U,
        page.tab.word(),
        theme.text.value,
        z,
    );
    push_arrows(list, ui, theme, 0, cell, tab_rect, page.hovered, (false, false), z);
    match page.tab {
        ResultsTab::Summary => {
            for (i, stat) in page.stats.iter().enumerate().take(KEY_ROWS_VISIBLE - 1) {
                let index = (i + 1) as u8;
                let rect = row_rect(i + 1);
                push_row_plate(
                    list,
                    ui,
                    theme,
                    index,
                    rect,
                    row_state(false, hovered_row == Some(index)),
                    &stat.label,
                    theme.text.label,
                    z,
                );
                push_value_cell(
                    list,
                    ui,
                    theme,
                    index,
                    rect,
                    6.0,
                    &stat.value,
                    theme.text.value,
                    z,
                );
            }
            // P10: the REPLAY button, disabled, with the reason — and the recording's path when
            // the session wrote one.
            let index = (page.stats.len().min(KEY_ROWS_VISIBLE - 1) + 1) as u8;
            let rect = row_rect(index as usize);
            push_row_plate(
                list,
                ui,
                theme,
                index,
                rect,
                WidgetState::Disabled,
                words::REPLAY,
                theme.text.label,
                z,
            );
            let cell = Rect::new(
                rect.right() - ui.px(6.0 + VALUE_W_U),
                rect.y + ui.px(3.0),
                ui.px(VALUE_W_U),
                rect.h - ui.px(6.0),
            );
            shell_put(
                list,
                z,
                ShellPart::RowValue(index),
                cell,
                shell_text(
                    &page.replay.reason,
                    Style::VALUE,
                    18.0,
                    Align::Center,
                    theme.text.value,
                    DigitMode::Proportional,
                ),
                WidgetState::Disabled,
            );
            if let Some(path) = &page.replay.recording {
                let note = Rect::new(
                    rect.x + ui.px(160.0),
                    rect.y,
                    rect.w - ui.px(160.0 + 8.0 + VALUE_W_U),
                    rect.h,
                );
                shell_put(
                    list,
                    z,
                    ShellPart::RowNote(index),
                    note,
                    shell_text(
                        &format!("{} {path}", words::RECORDED_TO),
                        Style::VALUE_STRONG,
                        16.0,
                        Align::Right,
                        theme.lamp,
                        DigitMode::Proportional,
                    ),
                    WidgetState::Idle,
                );
            }
        }
        ResultsTab::Timeline => {
            for (visible, row) in
                page.rows.iter().skip(page.first_visible).take(KEY_ROWS_VISIBLE).enumerate()
            {
                let index = (visible + 1) as u8;
                let rect = row_rect(visible + 1);
                push_row_plate(
                    list,
                    ui,
                    theme,
                    index,
                    rect,
                    row_state(false, hovered_row == Some(index)),
                    &format!("{}  {}", row.clock, row.word),
                    theme.text.label,
                    z,
                );
                let detail =
                    Rect::new(rect.x + ui.px(230.0), rect.y, rect.w - ui.px(230.0 + 8.0), rect.h);
                shell_put(
                    list,
                    z,
                    ShellPart::RowValue(index),
                    detail,
                    shell_text(
                        &row.detail,
                        Style::VALUE,
                        16.0,
                        Align::Left,
                        theme.text.value,
                        DigitMode::Tabular,
                    ),
                    WidgetState::Idle,
                );
            }
            let track_top = row_rect(1).y;
            let track_h = row_rect(KEY_ROWS_VISIBLE).bottom() - track_top;
            push_thumb(
                list,
                ui,
                theme,
                panel,
                track_top,
                track_h,
                page.first_visible,
                KEY_ROWS_VISIBLE,
                page.rows.len(),
                z,
            );
        }
        ResultsTab::Team => {
            for (visible, row) in
                page.team.iter().skip(page.first_visible).take(KEY_ROWS_VISIBLE).enumerate()
            {
                let index = (visible + 1) as u8;
                let rect = row_rect(visible + 1);
                let ink = if row.player { theme.lamp } else { theme.text.label };
                push_row_plate(
                    list,
                    ui,
                    theme,
                    index,
                    rect,
                    row_state(false, hovered_row == Some(index)),
                    &row.name,
                    ink,
                    z,
                );
                let standing = if row.destroyed { words::TL_DESTROYED } else { words::WORD_ALIVE };
                push_value_cell(
                    list,
                    ui,
                    theme,
                    index,
                    rect,
                    6.0 + NOTE_W_U,
                    standing,
                    theme.text.value,
                    z,
                );
                let crew = if row.player {
                    words::WORD_YOU
                } else if row.human {
                    words::WORD_HUMAN
                } else {
                    words::WORD_BOT
                };
                let note = Rect::new(
                    rect.right() - ui.px(NOTE_W_U),
                    rect.y,
                    ui.px(NOTE_W_U - 8.0),
                    rect.h,
                );
                shell_put(
                    list,
                    z,
                    ShellPart::RowNote(index),
                    note,
                    shell_text(
                        crew,
                        Style::VALUE_STRONG,
                        16.0,
                        Align::Right,
                        if row.ally { theme.text.label } else { theme.text.value },
                        DigitMode::Proportional,
                    ),
                    WidgetState::Idle,
                );
            }
            let track_top = row_rect(1).y;
            let track_h = row_rect(KEY_ROWS_VISIBLE).bottom() - track_top;
            push_thumb(
                list,
                ui,
                theme,
                panel,
                track_top,
                track_h,
                page.first_visible,
                KEY_ROWS_VISIBLE,
                page.team.len(),
                z,
            );
        }
    }
    push_footer(list, ui, theme, panel, title, &page.footer, z);
}

/// The menu: a column of buttons on a narrower plate; the commits in the one red.
fn push_menu(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &MenuScreenModel,
    z: &mut i16,
) {
    let items = page.kind.items();
    let (title, row_top, panel) = push_page_frame_sized(
        list,
        ui,
        theme,
        words::MENU_TITLE,
        MENU_W_U,
        MENU_ROW_H_U,
        items.len(),
        z,
    );
    let hovered_row = page.hovered.and_then(shell_row_index);
    let painted = theme.plates.steel_painted;
    for (i, item) in items.iter().enumerate() {
        let index = i as u8;
        let rect = Rect::new(
            title.x,
            row_top + i as f32 * ui.px(MENU_ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(MENU_ROW_H_U),
        );
        let selected = i == page.selected;
        let color = if item.is_commit() { theme.semantic.commit } else { painted.color };
        shell_put(
            list,
            z,
            ShellPart::RowPlate(index),
            rect,
            Payload::Plate { tile: painted.tile, radius_u: 2.0, bevel_u: 1.0, color },
            row_state(selected, hovered_row == Some(index)),
        );
        let ink = if item.is_commit() {
            theme.text.value
        } else if selected {
            theme.lamp
        } else {
            theme.text.label
        };
        shell_put(
            list,
            z,
            ShellPart::RowLabel(index),
            rect,
            shell_text(
                item.word(),
                Style::VALUE_STRONG,
                20.0,
                Align::Center,
                ink,
                DigitMode::Proportional,
            ),
            WidgetState::Idle,
        );
    }
    push_footer(list, ui, theme, panel, title, &page.footer, z);
}

/// The plate, the title and the rule every page shares: the rows' left edge, their width,
/// the first row's top, and the plate itself.
fn push_page_frame(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    title_word: &str,
    rows: usize,
    z: &mut i16,
) -> (Rect, f32, Rect) {
    push_page_frame_sized(list, ui, theme, title_word, PANEL_W_U, ROW_H_U, rows, z)
}

#[allow(clippy::too_many_arguments)]
fn push_page_frame_sized(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    title_word: &str,
    width_u: f32,
    row_h_u: f32,
    rows: usize,
    z: &mut i16,
) -> (Rect, f32, Rect) {
    let panel_h = PAD_U
        + TITLE_H_U
        + 3.0
        + RULE_GAP_U
        + rows as f32 * (row_h_u + ROW_GAP_U)
        + FOOTER_GAP_U
        + FOOTER_H_U
        + PAD_U;
    let panel = ui.anchor(Anchor::Center, [width_u, panel_h], [0.0, 0.0]);
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
            title_word,
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
    (title, rule.bottom() + ui.px(RULE_GAP_U), panel)
}

fn push_footer(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    panel: Rect,
    title: Rect,
    footer: &str,
    z: &mut i16,
) {
    let pad = ui.px(PAD_U);
    let rect =
        Rect::new(title.x, panel.bottom() - pad - ui.px(FOOTER_H_U), title.w, ui.px(FOOTER_H_U));
    shell_put(
        list,
        z,
        ShellPart::Footer,
        rect,
        shell_text(
            footer,
            Style::VALUE_STRONG,
            16.0,
            Align::Center,
            theme.lamp,
            DigitMode::Proportional,
        ),
        WidgetState::Idle,
    );
}

/// A row's plate and label; the row's rectangle comes back for the value cell.
#[allow(clippy::too_many_arguments)]
fn push_row_plate(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    index: u8,
    rect: Rect,
    state: WidgetState,
    label: &str,
    ink: [f32; 4],
    z: &mut i16,
) {
    let painted = theme.plates.steel_painted;
    shell_put(
        list,
        z,
        ShellPart::RowPlate(index),
        rect,
        Payload::Plate { tile: painted.tile, radius_u: 2.0, bevel_u: 1.0, color: painted.color },
        state,
    );
    shell_put(
        list,
        z,
        ShellPart::RowLabel(index),
        rect.inset(ui.px(8.0)),
        shell_text(label, Style::VALUE_STRONG, 18.0, Align::Left, ink, DigitMode::Proportional),
        WidgetState::Idle,
    );
}

/// A value cell under glass at the row's right, `right_inset_u` in from the edge.
#[allow(clippy::too_many_arguments)]
fn push_value_cell(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    index: u8,
    rect: Rect,
    right_inset_u: f32,
    value: &str,
    ink: [f32; 4],
    z: &mut i16,
) -> Rect {
    let glass = theme.plates.glass;
    let cell = Rect::new(
        rect.right() - ui.px(right_inset_u + VALUE_W_U),
        rect.y + ui.px(3.0),
        ui.px(VALUE_W_U),
        rect.h - ui.px(6.0),
    );
    shell_put(
        list,
        z,
        ShellPart::RowGlass(index),
        cell,
        Payload::Glass { radius_u: 1.0, phase: 0.3, color: glass.color },
        WidgetState::Idle,
    );
    shell_put(
        list,
        z,
        ShellPart::RowValue(index),
        cell,
        shell_text(value, Style::VALUE, 18.0, Align::Center, ink, DigitMode::Tabular),
        WidgetState::Idle,
    );
    cell
}

/// The arrows either side of a value cell.
#[allow(clippy::too_many_arguments)]
fn push_arrows(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    index: u8,
    cell: Rect,
    row: Rect,
    hovered: Option<ShellPart>,
    stuck: (bool, bool),
    z: &mut i16,
) {
    let arrow_state = |part: ShellPart, stuck: bool| {
        if stuck {
            WidgetState::Disabled
        } else if hovered == Some(part) {
            WidgetState::Hover
        } else {
            WidgetState::Idle
        }
    };
    let dec = Rect::new(cell.x - ui.px(ARROW_W_U), row.y, ui.px(ARROW_W_U), row.h);
    let inc = Rect::new(cell.right(), row.y, ui.px(ARROW_W_U), row.h);
    shell_put(
        list,
        z,
        ShellPart::RowDec(index),
        dec,
        shell_text(
            words::ARROW_DEC,
            Style::VALUE_STRONG,
            18.0,
            Align::Center,
            theme.text.label,
            DigitMode::Proportional,
        ),
        arrow_state(ShellPart::RowDec(index), stuck.0),
    );
    shell_put(
        list,
        z,
        ShellPart::RowInc(index),
        inc,
        shell_text(
            words::ARROW_INC,
            Style::VALUE_STRONG,
            18.0,
            Align::Center,
            theme.text.label,
            DigitMode::Proportional,
        ),
        arrow_state(ShellPart::RowInc(index), stuck.1),
    );
}

fn row_state(selected: bool, hovered: bool) -> WidgetState {
    if selected {
        WidgetState::Focused
    } else if hovered {
        WidgetState::Hover
    } else {
        WidgetState::Idle
    }
}

fn push_keybinds(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &KeybindsScreenModel,
    z: &mut i16,
) {
    let (title, row_top, panel) =
        push_page_frame(list, ui, theme, words::KEYBINDS_TITLE, 1 + KEY_ROWS_VISIBLE, z);
    let row_rect = |visible: usize| {
        Rect::new(
            title.x,
            row_top + visible as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(ROW_H_U),
        )
    };
    let hovered_row = page.hovered.and_then(shell_row_index);
    // Row 0: the context, stepped by its arrows.
    let context_rect = row_rect(0);
    let selected = page.selected == 0;
    push_row_plate(
        list,
        ui,
        theme,
        0,
        context_rect,
        row_state(selected, hovered_row == Some(0)),
        words::SET_CONTEXT,
        if selected { theme.lamp } else { theme.text.label },
        z,
    );
    let cell = push_value_cell(
        list,
        ui,
        theme,
        0,
        context_rect,
        6.0 + ARROW_W_U,
        page.context.word(),
        theme.text.value,
        z,
    );
    push_arrows(list, ui, theme, 0, cell, context_rect, page.hovered, (false, false), z);
    // The actions: a window of rows.
    for (visible, row) in
        page.rows.iter().skip(page.first_visible).take(KEY_ROWS_VISIBLE).enumerate()
    {
        let index = (visible + 1) as u8;
        let rect = row_rect(visible + 1);
        let selected = page.selected == page.first_visible + visible + 1;
        let ink = if selected {
            theme.lamp
        } else if row.changed {
            theme.text.value
        } else {
            theme.text.label
        };
        push_row_plate(
            list,
            ui,
            theme,
            index,
            rect,
            row_state(selected || row.listening, hovered_row == Some(index)),
            &row.name,
            ink,
            z,
        );
        let (value, value_ink) = if row.listening {
            (words::LISTENING, theme.lamp)
        } else {
            (row.keys.as_str(), theme.text.value)
        };
        push_value_cell(list, ui, theme, index, rect, 6.0 + NOTE_W_U, value, value_ink, z);
        if let Some(other) = &row.conflict {
            let note =
                Rect::new(rect.right() - ui.px(NOTE_W_U), rect.y, ui.px(NOTE_W_U - 8.0), rect.h);
            shell_put(
                list,
                z,
                ShellPart::RowNote(index),
                note,
                shell_text(
                    &format!("{} {other}", words::SHARED_WITH),
                    Style::VALUE_STRONG,
                    16.0,
                    Align::Right,
                    theme.lamp,
                    DigitMode::Proportional,
                ),
                WidgetState::Idle,
            );
        }
    }
    // The thumb: where the window sits on the whole list.
    let track_top = row_rect(1).y;
    let track_h = row_rect(KEY_ROWS_VISIBLE).bottom() - track_top;
    push_thumb(
        list,
        ui,
        theme,
        panel,
        track_top,
        track_h,
        page.first_visible,
        KEY_ROWS_VISIBLE,
        page.rows.len(),
        z,
    );
    push_footer(list, ui, theme, panel, title, &page.footer, z);
}

fn push_settings(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    page: &SettingsScreenModel,
    z: &mut i16,
) {
    let rows = page.view.rows();
    let (title, row_top, panel) =
        push_page_frame(list, ui, theme, words::SETTINGS_TITLE, rows.len(), z);
    let dim = theme.text.label_dim;
    let hovered_row = page.hovered.and_then(shell_row_index);
    for (i, row) in rows.iter().enumerate() {
        let index = i as u8;
        let rect = Rect::new(
            title.x,
            row_top + i as f32 * ui.px(ROW_H_U + ROW_GAP_U),
            title.w,
            ui.px(ROW_H_U),
        );
        let selected = i == page.selected;
        push_row_plate(
            list,
            ui,
            theme,
            index,
            rect,
            row_state(selected, hovered_row == Some(index)),
            &row.label,
            if selected { theme.lamp } else { theme.text.label },
            z,
        );
        let cell = push_value_cell(
            list,
            ui,
            theme,
            index,
            rect,
            6.0 + ARROW_W_U,
            &row.value,
            theme.text.value,
            z,
        );
        if let Some(frac) = row.frac {
            let bar = Rect::new(
                cell.x + ui.px(12.0),
                cell.bottom() - ui.px(5.0),
                cell.w - ui.px(24.0),
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
        push_arrows(list, ui, theme, index, cell, rect, page.hovered, (row.at_min, row.at_max), z);
    }
    push_footer(list, ui, theme, panel, title, &page.footer, z);
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

/// The results page as the golden stages it: a won battle in a T-54, eight numbers, a timeline
/// of a fight — shots with their results, hits taken, a kill, spotted spans, the enemies that
/// saw the crew — and the roster; the REPLAY disabled with its reason and a recording named.
pub(crate) fn demo_results_screen(tab: ResultsTab) -> ShellModel {
    use game_core::{BattleEventId, ShellId, TankId};
    let stat =
        |label: &str, value: &str| StatRow { label: label.to_string(), value: value.to_string() };
    let row = |tick: u64, word: &str, detail: &str, source: RowSource| TimelineRow {
        tick,
        clock: format!("{}:{:02}", tick / 3600, (tick / 60) % 60),
        word: word.to_string(),
        detail: detail.to_string(),
        source,
    };
    let team = |name: &str, ally: bool, human: bool, destroyed: bool, player: bool| TeamRow {
        name: name.to_string(),
        ally,
        human,
        destroyed,
        player,
    };
    ShellModel::Results(ResultsScreenModel {
        outcome: BattleHudOutcome::Victory,
        hull: "T-54 \u{b7} A".to_string(),
        stats: vec![
            stat(words::STAT_SHOTS, "11"),
            stat(words::STAT_HITS, "8"),
            stat(words::STAT_PENETRATIONS, "6"),
            stat(words::STAT_DAMAGE_DEALT, "1 480"),
            stat(words::STAT_DAMAGE_TAKEN, "620"),
            stat(words::STAT_KILLS, "2"),
            stat(words::STAT_SPOTTED, "3"),
            stat(words::STAT_SEEN_BY, "2"),
            stat(words::STAT_DURATION, "6:42"),
        ],
        rows: vec![
            row(1_140, words::SPOTTED_LAMP, "14 S", RowSource::Spotted { from_tick: 1_140 }),
            row(
                1_200,
                words::HIT_PEN,
                "BR-412D \u{b7} 148 > 132 MM @ 22\u{b0} \u{b7} LOWER GLACIS \u{b7} Tiger II \u{b7} 240 \u{b7} 310 M",
                RowSource::Shot(ShellId(4)),
            ),
            row(
                1_380,
                words::TL_TAKEN,
                "Tiger II \u{b7} B \u{b7} PzGr. 39/42 \u{b7} 194 > 162 MM @ 31\u{b0} \u{b7} TURRET FRONT \u{b7} GUN \u{b7} 290",
                RowSource::Damage(BattleEventId(31)),
            ),
            row(
                1_620,
                words::HIT_RICOCHET,
                "BR-412D \u{b7} 171 > 310 MM @ 71\u{b0} \u{b7} UPPER GLACIS \u{b7} Jagdtiger \u{b7} 0 \u{b7} 510 M",
                RowSource::Shot(ShellId(5)),
            ),
            row(
                1_640,
                words::TL_SEEN_BY,
                "Tiger II \u{b7} B \u{b7} 308 M \u{b7} 9 S",
                RowSource::Observer { observer: TankId(9), from_tick: 1_100 },
            ),
            row(2_040, words::TL_SHOT, words::TL_NO_HIT, RowSource::Shot(ShellId(6))),
            row(
                2_400,
                words::HIT_PEN,
                "BR-412D \u{b7} 148 > 96 MM @ 8\u{b0} \u{b7} HULL SIDE \u{b7} Tiger II \u{b7} 320 \u{b7} 140 M",
                RowSource::Shot(ShellId(7)),
            ),
            row(
                2_400,
                words::TL_DESTROYED,
                "Tiger II \u{b7} B \u{b7} BY YOU",
                RowSource::Kill { victim: TankId(9), tick: 2_400 },
            ),
            row(
                3_300,
                words::TL_TAKEN,
                "UNSEEN \u{b7} HE \u{b7} 88 > 45 MM @ 40\u{b0} \u{b7} ENGINE DECK \u{b7} ENGINE \u{b7} 330",
                RowSource::Damage(BattleEventId(58)),
            ),
            row(
                4_020,
                words::TL_DESTROYED,
                "IS-3 \u{b7} B \u{b7} BY Panth II \u{b7} D",
                RowSource::Kill { victim: TankId(2), tick: 4_020 },
            ),
        ],
        team: vec![
            team("T-54 \u{b7} A", true, true, false, true),
            team("IS-3 \u{b7} B", true, false, true, false),
            team("Cent 3 \u{b7} C", true, false, false, false),
            team("Tiger I \u{b7} D", true, false, false, false),
            team("Panth II \u{b7} E", true, false, true, false),
            team("Jagdtig \u{b7} F", true, false, false, false),
            team("T-34-85 \u{b7} G", true, false, false, false),
            team("Tiger II \u{b7} A", false, false, false, false),
            team("Tiger II \u{b7} B", false, false, true, false),
            team("T-54 \u{b7} C", false, true, false, false),
            team("Panth II \u{b7} D", false, false, false, false),
            team("Jagdtig \u{b7} E", false, false, true, false),
            team("IS-3 \u{b7} F", false, false, true, false),
            team("T-34-85 \u{b7} G", false, false, true, false),
        ],
        replay: ReplayNote {
            reason: words::REPLAY_REASON.to_string(),
            recording: Some("replays/2026-09-06-prokhorovka.wotrec".to_string()),
        },
        tab,
        first_visible: 0,
        hovered: None,
        footer: crate::app::shell::results_footer(&crate::app::keybinds::KeyBindings::default()),
    })
}

/// The BATTLES page as the golden stages it: a week of battles, the newest selected.
pub(crate) fn demo_battles_screen() -> ShellModel {
    let row =
        |date: &str, map: &str, vehicle: &str, outcome: &str, kills: u32, damage_dealt: u32| {
            BattleRow {
                date: date.to_string(),
                map: map.to_string(),
                vehicle: vehicle.to_string(),
                outcome: outcome.to_string(),
                kills,
                damage_dealt,
            }
        };
    ShellModel::Battles(BattlesScreenModel {
        rows: vec![
            row("2026-09-06 12:40", "prokhorovka", "t-54", words::VICTORY, 2, 1_480),
            row("2026-09-06 12:21", "prokhorovka", "t-54", words::DEFEAT, 0, 310),
            row("2026-09-06 11:58", "bystra-valley", "tiger-ii", words::VICTORY, 3, 2_050),
            row("2026-09-05 22:14", "ostrogorsk", "is-3", words::DRAW, 1, 890),
            row("2026-09-05 21:47", "ostrogorsk", "is-3", words::DEFEAT, 1, 1_120),
            row("2026-09-05 21:20", "orliny-pereval", "t-54", words::VICTORY, 4, 2_610),
        ],
        selected: 0,
        first_visible: 0,
        hovered: None,
        footer: crate::app::shell::battles_footer(&crate::app::keybinds::KeyBindings::default()),
    })
}

/// A menu as the golden stages it: nothing under the cursor, the top entry selected.
pub(crate) fn demo_menu_screen(kind: MenuKind) -> ShellModel {
    ShellModel::Menu(MenuScreenModel {
        kind,
        selected: 0,
        hovered: None,
        footer: crate::app::shell::menu_footer(&crate::app::keybinds::KeyBindings::default()),
    })
}

/// The key bindings page as the golden stages it: the battle's keys, FIRE bound to W and so
/// shared with FORWARD — both rows say so — FIRE selected.
pub(crate) fn demo_keybinds_screen() -> ShellModel {
    use winit::keyboard::KeyCode;
    let mut keys = crate::app::keybinds::KeyBindings::default();
    keys.bind(Action::Fire, KeyCode::KeyW);
    let fire = Action::ALL
        .into_iter()
        .filter(|action| action.context() == Context::Battle)
        .position(|action| action == Action::Fire)
        .expect("FIRE is the battle's");
    ShellModel::Keybinds(crate::app::shell::keybinds_page_model(
        &keys,
        Context::Battle,
        fire + 1,
        0,
        None,
        None,
    ))
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
        let ShellModel::Settings(mut page) = demo_settings_screen() else { panic!("settings") };
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

    /// P8: the key bindings page prints the context row and a window of the context's
    /// actions with their keys, names a shared key on both rows, lights the selected row,
    /// shows a thumb when the list is longer than the plate, and a listening row says so.
    #[test]
    fn the_keybinds_page_prints_a_context_and_names_a_shared_key_on_both_rows() {
        let ui = Ui::reference();
        let mut model = demo::demo_model(false);
        model.shell = Some(demo_keybinds_screen());
        let list = build_battle_hud_list(&model, &ui);
        let text_of = |list: &DrawList<HudElement>, part: ShellPart| match &list
            .find(HudElement::Shell(part))
            .expect("part")
            .payload
        {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{part:?} is not text: {other:?}"),
        };
        assert_eq!(text_of(&list, ShellPart::RowLabel(0)), words::SET_CONTEXT);
        assert_eq!(text_of(&list, ShellPart::RowValue(0)), words::CONTEXT_BATTLE);
        assert_eq!(text_of(&list, ShellPart::RowLabel(1)), "FORWARD");
        assert_eq!(text_of(&list, ShellPart::RowValue(1)), "W / UP");
        assert_eq!(text_of(&list, ShellPart::RowNote(1)), format!("{} FIRE", words::SHARED_WITH));
        let fire = (1..=KEY_ROWS_VISIBLE as u8)
            .find(|i| text_of(&list, ShellPart::RowLabel(*i)) == "FIRE")
            .expect("FIRE on the plate");
        assert_eq!(text_of(&list, ShellPart::RowValue(fire)), "W");
        assert_eq!(
            text_of(&list, ShellPart::RowNote(fire)),
            format!("{} FORWARD", words::SHARED_WITH)
        );
        assert_eq!(
            list.find(HudElement::Shell(ShellPart::RowPlate(fire))).expect("plate").state,
            WidgetState::Focused
        );
        assert!(
            list.find(HudElement::Shell(ShellPart::RowNote(2))).is_none(),
            "BACK shares nothing"
        );
        assert!(
            list.find(HudElement::Shell(ShellPart::ScrollBar)).is_some(),
            "the battle's list scrolls"
        );
        assert!(
            list.find(HudElement::Shell(ShellPart::RowLabel(KEY_ROWS_VISIBLE as u8 + 1))).is_none(),
            "no row past the window"
        );
        // A listening row says so, in the lamp.
        let ShellModel::Keybinds(mut page) = demo_keybinds_screen() else { panic!("keybinds") };
        page.rows[0].listening = true;
        model.shell = Some(ShellModel::Keybinds(page));
        let list = build_battle_hud_list(&model, &ui);
        assert_eq!(text_of(&list, ShellPart::RowValue(1)), words::LISTENING);
    }

    /// P10: the REPLAY button is disabled and says why until a viewer exists; a recording the
    /// session wrote is named beside it. P1: the outcome and the hull stand beside the title,
    /// every number has its row, and the TEAM tab carries alive/dead and the crew's kind — no
    /// statistics.
    #[test]
    fn the_replay_button_is_disabled_and_says_why_until_a_viewer_exists() {
        let ui = Ui::reference();
        let mut model = demo::demo_model(false);
        model.shell = Some(demo_results_screen(ResultsTab::Summary));
        let list = build_battle_hud_list(&model, &ui);
        let text_of = |list: &DrawList<HudElement>, part: ShellPart| match &list
            .find(HudElement::Shell(part))
            .expect("part")
            .payload
        {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{part:?} is not text: {other:?}"),
        };
        let ShellModel::Results(page) = demo_results_screen(ResultsTab::Summary) else {
            panic!("results")
        };
        let replay = (page.stats.len() + 1) as u8;
        assert_eq!(text_of(&list, ShellPart::RowLabel(replay)), words::REPLAY);
        assert_eq!(
            list.find(HudElement::Shell(ShellPart::RowPlate(replay))).expect("plate").state,
            WidgetState::Disabled
        );
        assert_eq!(text_of(&list, ShellPart::RowValue(replay)), words::REPLAY_REASON);
        assert!(text_of(&list, ShellPart::RowNote(replay)).starts_with(words::RECORDED_TO));
        assert!(text_of(&list, ShellPart::Subtitle).starts_with(words::VICTORY));
        for (i, stat) in page.stats.iter().enumerate() {
            assert_eq!(text_of(&list, ShellPart::RowLabel(i as u8 + 1)), stat.label);
            assert_eq!(text_of(&list, ShellPart::RowValue(i as u8 + 1)), stat.value);
        }
        // Without a recording the note is absent; the reason stands.
        let ShellModel::Results(mut page) = demo_results_screen(ResultsTab::Summary) else {
            panic!("results")
        };
        page.replay.recording = None;
        model.shell = Some(ShellModel::Results(page));
        let list = build_battle_hud_list(&model, &ui);
        assert!(list.find(HudElement::Shell(ShellPart::RowNote(replay))).is_none());
        assert_eq!(text_of(&list, ShellPart::RowValue(replay)), words::REPLAY_REASON);
        // The TEAM tab: the roster with alive/dead and the crew's kind, the player in the lamp.
        model.shell = Some(demo_results_screen(ResultsTab::Team));
        let list = build_battle_hud_list(&model, &ui);
        assert_eq!(text_of(&list, ShellPart::RowLabel(1)), "T-54 \u{b7} A");
        assert_eq!(text_of(&list, ShellPart::RowValue(1)), words::WORD_ALIVE);
        assert_eq!(text_of(&list, ShellPart::RowNote(1)), words::WORD_YOU);
        assert_eq!(text_of(&list, ShellPart::RowValue(2)), words::TL_DESTROYED);
        assert_eq!(text_of(&list, ShellPart::RowNote(2)), words::WORD_BOT);
        assert_eq!(text_of(&list, ShellPart::RowValue(0)), words::TAB_TEAM);
        // The TIMELINE tab: a window of rows, the clock and the word on the plate, the detail
        // beside it.
        model.shell = Some(demo_results_screen(ResultsTab::Timeline));
        let list = build_battle_hud_list(&model, &ui);
        assert!(text_of(&list, ShellPart::RowLabel(1)).ends_with(words::SPOTTED_LAMP));
        assert!(text_of(&list, ShellPart::RowValue(2)).starts_with("BR-412D"));
    }

    /// P4: the BATTLES page lists the history newest first with the day, the map, the hull,
    /// the outcome and the crew's two numbers; the selected row is lit; an empty history says
    /// so on a disabled plate.
    #[test]
    fn the_battles_page_lists_the_history_and_an_empty_one_says_so() {
        let ui = Ui::reference();
        let mut model = demo::demo_model(false);
        model.shell = Some(demo_battles_screen());
        let list = build_battle_hud_list(&model, &ui);
        let text_of = |list: &DrawList<HudElement>, part: ShellPart| match &list
            .find(HudElement::Shell(part))
            .expect("part")
            .payload
        {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{part:?} is not text: {other:?}"),
        };
        assert_eq!(
            text_of(&list, ShellPart::RowLabel(0)),
            "2026-09-06 12:40 \u{b7} prokhorovka \u{b7} t-54"
        );
        assert_eq!(text_of(&list, ShellPart::RowValue(0)), words::VICTORY);
        assert_eq!(
            text_of(&list, ShellPart::RowNote(0)),
            format!("2 {} \u{b7} 1480 {}", words::STAT_KILLS, words::DMG_UNIT)
        );
        assert_eq!(
            list.find(HudElement::Shell(ShellPart::RowPlate(0))).expect("plate").state,
            WidgetState::Focused
        );
        assert!(list.find(HudElement::Shell(ShellPart::RowPlate(6))).is_none(), "six battles");
        model.shell = Some(ShellModel::Battles(BattlesScreenModel {
            rows: Vec::new(),
            selected: 0,
            first_visible: 0,
            hovered: None,
            footer: String::new(),
        }));
        let list = build_battle_hud_list(&model, &ui);
        assert_eq!(text_of(&list, ShellPart::RowLabel(0)), words::NO_BATTLES);
        assert_eq!(
            list.find(HudElement::Shell(ShellPart::RowPlate(0))).expect("plate").state,
            WidgetState::Disabled
        );
    }

    /// P8: the battle's menu prints its six entries (QUIT joined them 2026-09-09) and the
    /// garage's its four; the commits wear the one red and nothing else does; nothing is lit
    /// under a still cursor; a menu replaces the instruments like every page.
    #[test]
    fn the_menus_print_their_entries_and_only_the_commits_wear_the_red() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        for (kind, count) in [(MenuKind::Battle, 6), (MenuKind::Garage, 4)] {
            let mut model = demo::demo_model(false);
            model.shell = Some(demo_menu_screen(kind));
            let list = build_battle_hud_list(&model, &ui);
            assert!(list.iter().all(|element| matches!(element.id, HudElement::Shell(_))));
            for (i, item) in kind.items().iter().enumerate() {
                let plate =
                    list.find(HudElement::Shell(ShellPart::RowPlate(i as u8))).expect("plate");
                let Payload::Plate { color, .. } = plate.payload else { panic!("a plate") };
                assert_eq!(color == theme.semantic.commit, item.is_commit(), "{kind:?} {item:?}");
                let Payload::Text { text, .. } = &list
                    .find(HudElement::Shell(ShellPart::RowLabel(i as u8)))
                    .expect("label")
                    .payload
                else {
                    panic!("a word")
                };
                assert_eq!(text, item.word());
                assert_ne!(plate.state, WidgetState::Hover, "nothing is lit under a still cursor");
            }
            assert!(list.find(HudElement::Shell(ShellPart::RowPlate(count))).is_none());
            assert!(kind.items().iter().any(|item| item.is_way_out()), "{kind:?} has a way out");
            assert!(!kind.items()[0].is_commit(), "{kind:?}: the selection starts on a safe entry");
        }
        for item in MenuItem::ALL {
            assert!(
                MenuKind::Battle.items().contains(&item)
                    || MenuKind::Garage.items().contains(&item),
                "{item:?} sits on no menu"
            );
        }
    }

    /// The promise as an INVARIANT, not as a list: every menu the player can raise can leave
    /// the game. The count literal above cannot say this — it survives a reordering and it was
    /// green while QUIT sat on a menu no started session could reach.
    #[test]
    fn every_menu_can_leave_the_game() {
        for kind in [MenuKind::Battle, MenuKind::Garage] {
            assert!(
                kind.items().contains(&MenuItem::Quit),
                "{kind:?}: a player who wants to stop playing must find QUIT here"
            );
            assert!(
                kind.items().iter().any(|item| item.is_way_out()),
                "{kind:?}: is_way_out must mean leaving the GAME, not leaving the battle"
            );
        }
    }
}
