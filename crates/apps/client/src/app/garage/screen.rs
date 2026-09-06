//! The hangar screen as a draw list (interface program G1–G7, G14): the top bar with BATTLE,
//! the map row and the tabs; the nameplate that says which tank and why; the crew column; the
//! VEHICLE column — every row labelled, with a bar against the roster and the derived rows;
//! the loadout strip with the ammunition explained; the carousel with its recognition cards;
//! the inspector's legend; the option list. Steel, enamel and glass from the theme, laid out in
//! `u` through the toolkit's anchors; every clickable surface is an interactive element, so the
//! hit test reads the same rectangles the eye does — nothing is hit-tested against a constant
//! the drawing no longer uses.

use game_core::VehicleKind;
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload, WidgetState};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::chrome::{push_chips, push_hint_strip, push_tooltip};
use super::draft::FitSlot;
use super::elements::GarageElement as E;
use super::filter::Chip;
use super::hints::tooltip_text;
use super::layout::{
    ammo_icon, carousel_overflows, carousel_window, map_pick_label, slot_icon, slot_label,
};
use super::silhouette::silhouette_rects;
use super::stats::stat_rows;
use super::{GarageHit, GarageState, GarageView};
use crate::ui_strings::garage as words;

// The bar across the top: BATTLE in the middle, the map row beside it, the tabs under them.
const TOP_BAR_H_U: f32 = 118.0;
const BATTLE_SIZE_U: [f32; 2] = [250.0, 58.0];
const BATTLE_TOP_U: f32 = 22.0;
const MAP_SIZE_U: [f32; 2] = [300.0, 40.0];
const MAP_OFFSET_U: [f32; 2] = [340.0, 31.0];
const TAB_SIZE_U: [f32; 2] = [200.0, 34.0];
const TAB_TOP_U: f32 = 78.0;
const TAB_GARAGE_X_U: f32 = -360.0;
const TAB_TREE_X_U: f32 = 220.0;
// The nameplate under the bar, the repair tag under it, the legend under that.
const NAME_SIZE_U: [f32; 2] = [640.0, 100.0];
const NAME_TOP_U: f32 = 132.0;
const TAG_TOP_U: f32 = 238.0;
const LEGEND_SIZE_U: [f32; 2] = [640.0, 56.0];
const LEGEND_TOP_U: f32 = 270.0;
// The columns.
const CREW_SIZE_U: [f32; 2] = [330.0, 400.0];
const CREW_OFFSET_U: [f32; 2] = [30.0, -60.0];
const CREW_ROW_H_U: f32 = 62.0;
const STATS_SIZE_U: [f32; 2] = [400.0, 600.0];
// Hung from the top-right corner under the bar, so the widened compare column (G3) never
// reaches the loadout strip below it.
const STATS_OFFSET_U: [f32; 2] = [20.0, 130.0];
// G3: the column widens for the compared hull's numbers and the deltas beside the own.
const STATS_COMPARE_SIZE_U: [f32; 2] = [560.0, 600.0];
const COMPARE_OWN_RIGHT_U: f32 = 230.0;
const COMPARE_OTHER_RIGHT_U: f32 = 100.0;
const STAT_ROW_H_U: f32 = 40.0;
const HEADER_H_U: f32 = 30.0;
const PAD_U: f32 = 14.0;
// The loadout strip and the carousel along the bottom.
const LOADOUT_SIZE_U: [f32; 2] = [1120.0, 156.0];
const LOADOUT_OFFSET_U: [f32; 2] = [-40.0, 184.0];
const SLOT_SIZE_U: [f32; 2] = [90.0, 124.0];
const SLOT_PITCH_U: f32 = 100.0;
const MODULE_START_U: f32 = 24.0;
const AMMO_START_U: f32 = 690.0;
const RACK_LEFT_U: f32 = 990.0;
const CAR_CELL_SIZE_U: [f32; 2] = [116.0, 150.0];
const CAR_PITCH_U: f32 = 130.0;
const CAR_BOTTOM_U: f32 = 8.0;
const CAR_ARROW_SIZE_U: [f32; 2] = [50.0, 150.0];
// The option list over the strip.
const OPTION_ROW_SIZE_U: [f32; 2] = [440.0, 54.0];
const OPTION_ROW_PITCH_U: f32 = 60.0;

/// The smallest text the garage prints, in `u` (G5): nothing below it survives 768p. The
/// floor is the lock's; the screen sets every size at or above it by hand.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const GARAGE_TEXT_FLOOR_U: f32 = 16.0;

pub(super) fn text(
    text: &str,
    style: Style,
    size_u: f32,
    align: Align,
    color: [f32; 4],
    digits: DigitMode,
) -> Payload {
    Payload::Text { text: text.to_string(), style, size_u, align, color, digits }
}

pub(super) fn put(list: &mut DrawList<E>, id: E, rect: Rect, payload: Payload) {
    let z = list.len() as i16;
    list.push(Element::new(id, rect, payload).z(z));
}

pub(super) fn put_control(
    list: &mut DrawList<E>,
    id: E,
    rect: Rect,
    payload: Payload,
    state: WidgetState,
) {
    let z = list.len() as i16;
    list.push(Element::new(id, rect, payload).z(z).interactive(state));
}

pub(super) fn plate(
    theme: &Theme,
    material: ui_kit::theme::PlateMaterial,
    radius_u: f32,
) -> Payload {
    let _ = theme;
    Payload::Plate { tile: material.tile, radius_u, bevel_u: 1.0, color: material.color }
}

fn lock_clock_word(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl GarageState {
    /// The cursor in the context's pixels: the clip-space cursor the app feeds, mapped over
    /// the viewport the list is laid out in.
    pub(super) fn cursor_px(&self, ui: &Ui) -> [f32; 2] {
        let v = ui.viewport();
        [(self.cursor_clip[0] + 1.0) * 0.5 * v.w, (1.0 - self.cursor_clip[1]) * 0.5 * v.h]
    }

    /// The layout context the garage lays itself out in: the viewport the app last told it
    /// about (the reference 1080p until then) at the player's interface scale.
    pub(super) fn ui(&self) -> Ui {
        Ui::new(
            self.viewport_px[0].max(1.0) as u32,
            self.viewport_px[1].max(1.0) as u32,
            self.ui_scale,
        )
    }

    /// The viewport and the interface scale, from the app, every garage frame.
    pub(in crate::app) fn set_viewport(&mut self, width_px: u32, height_px: u32, ui_scale: f32) {
        self.viewport_px = [width_px as f32, height_px as f32];
        self.ui_scale = ui_scale;
    }

    /// G14: the hull is locked in a battle that still runs; `None` when it is free.
    pub(in crate::app) fn set_locked(&mut self, remaining_s: Option<f32>) {
        self.locked_remaining_s = remaining_s;
    }

    pub(in crate::app) fn is_locked(&self) -> bool {
        self.locked_remaining_s.is_some()
    }

    /// Put the cursor on the centre of an element of the current screen; `false` when the
    /// screen does not draw it. The locks' way to click.
    #[cfg(test)]
    pub(in crate::app) fn set_cursor_on(&mut self, element: E) -> bool {
        let ui = self.ui();
        let list = build_screen_list(self, &ui, None);
        let Some(rect) = list.find(element).map(|e| e.rect) else { return false };
        let c = rect.center();
        let v = ui.viewport();
        self.set_cursor([c[0] / v.w * 2.0 - 1.0, 1.0 - c[1] / v.h * 2.0]);
        true
    }
}

/// The element under the cursor — the hover, and what a click lands on.
pub(super) fn hit_key(state: &GarageState, ui: &Ui) -> Option<E> {
    build_screen_list(state, ui, None).hit(state.cursor_px(ui))
}

/// What a click lands on, from the same rectangles the screen draws.
pub(super) fn hit_screen(state: &GarageState, ui: &Ui, shift: bool) -> GarageHit {
    let dir: isize = if shift { -1 } else { 1 };
    let roster = state.roster();
    let window = carousel_window(roster.len(), state.carousel_scroll());
    match hit_key(state, ui) {
        Some(E::BattleButton) => {
            if state.is_locked() {
                GarageHit::Locked
            } else {
                GarageHit::Battle
            }
        }
        Some(E::MapRow) => GarageHit::MapCycle(dir as i8),
        Some(E::TabTechTree) => match state.view() {
            GarageView::Hangar => GarageHit::OpenTechTree,
            GarageView::TechTree => GarageHit::CloseTechTree,
        },
        // From the hangar GARAGE is already the active view: the click falls through.
        Some(E::TabGarage) => match state.view() {
            GarageView::Hangar => GarageHit::Scene,
            GarageView::TechTree => GarageHit::CloseTechTree,
        },
        Some(E::ModuleSlot(i)) => GarageHit::ModuleCycle(FitSlot::ALL[usize::from(i)], dir),
        Some(E::OptionRow(i)) => match state.option_list() {
            Some(slot) => GarageHit::OptionRow(slot, usize::from(i)),
            None => GarageHit::Scene,
        },
        Some(E::AmmoSlot(i)) => GarageHit::AmmoSelect(usize::from(i)),
        Some(E::AmmoMinus(i)) => GarageHit::AmmoAdjust(usize::from(i), -1),
        Some(E::AmmoPlus(i)) => GarageHit::AmmoAdjust(usize::from(i), 1),
        // The cells are the roster the chips let through (G9); shift on one compares (G3).
        Some(E::CarouselCell(i)) => match roster.get(window.start + usize::from(i)) {
            Some(kind) => {
                let absolute = VehicleKind::PLAYABLE
                    .iter()
                    .position(|k| k == kind)
                    .expect("the roster is playable");
                if shift { GarageHit::Compare(absolute) } else { GarageHit::Vehicle(absolute) }
            }
            None => GarageHit::Scene,
        },
        Some(E::CarouselArrow(0)) => GarageHit::CarouselScroll(-1),
        Some(E::CarouselArrow(_)) => GarageHit::CarouselScroll(1),
        Some(E::Chip(i)) => GarageHit::Chip(Chip::ALL[usize::from(i)], dir as i8),
        _ => match state.view() {
            GarageView::TechTree => super::panels::techtree::hit_test(state),
            GarageView::Hangar => GarageHit::Scene,
        },
    }
}

/// The whole screen for the current view, with the hovered element lit.
pub(super) fn build_screen(state: &GarageState, ui: &Ui, theme: &Theme) -> DrawList<E> {
    let hovered = hit_key(state, ui);
    let mut list = build_screen_list(state, ui, hovered);
    if state.view() == GarageView::TechTree {
        // The tree stays its legacy self until G12; its nodes and BACK keep the old hover wash.
        let aspect = ui.aspect();
        let legacy_z = list.len() as i16;
        list.push(
            Element::new(
                E::TechTree,
                Rect::default(),
                Payload::Legacy(super::panels::techtree::draw(state, aspect)),
            )
            .z(legacy_z),
        );
        if let Some((center, half)) = super::overlay::tree_hover_rect(state) {
            // The legacy rectangle is clip space (y up); the wash is a flat full bar over it.
            let viewport = ui.viewport();
            let rect = Rect::new(
                (center[0] - half[0] + 1.0) * 0.5 * viewport.w,
                (1.0 - center[1] - half[1]) * 0.5 * viewport.h,
                half[0] * viewport.w,
                half[1] * viewport.h,
            );
            let wash = Payload::Bar {
                frac: 1.0,
                fill: ui_kit::theme::color::HOVER,
                back: [0.0, 0.0, 0.0, 0.0],
            };
            let z = list.len() as i16;
            list.push(Element::new(E::Hover, rect, wash).z(z));
        }
    }
    // G6: the tooltip of the control the cursor has rested on, over everything.
    if let Some(key) = state.tooltip_key()
        && Some(key) == hovered
        && let Some(anchor) = list.find(key).map(|e| e.rect)
        && let Some(words) = tooltip_text(state, key)
    {
        push_tooltip(&mut list, ui, theme, anchor, &words);
    }
    list
}

/// The screen's elements (the top bar in both views; the hangar's panels in the hangar), with
/// `hovered` worn as the Hover state where a control is not already Focused or Disabled.
pub(super) fn build_screen_list(state: &GarageState, ui: &Ui, hovered: Option<E>) -> DrawList<E> {
    let theme = Theme::standard();
    let mut list = DrawList::new();
    if !state.is_open() {
        return list;
    }
    push_garage_top_bar(&mut list, ui, &theme, state);
    push_hint_strip(&mut list, ui, &theme, state);
    if state.view() == GarageView::Hangar {
        push_nameplate(&mut list, ui, &theme, state);
        if state.inspector_on() {
            push_legend(&mut list, ui, &theme);
        }
        push_crew(&mut list, ui, &theme);
        push_stats(&mut list, ui, &theme, state);
        push_loadout(&mut list, ui, &theme, state);
        push_carousel(&mut list, ui, &theme, state);
        push_chips(&mut list, ui, &theme, state);
        if let Some(slot) = state.option_list() {
            push_options(&mut list, ui, &theme, state, slot);
        }
    }
    if let Some(key) = hovered
        && let Some(element) = list.find_mut(key)
        && element.state == WidgetState::Idle
    {
        // G6: the machine's word — pressed while the button is down on it, hovered otherwise.
        element.state = if state.pressed_key() == Some(key) {
            WidgetState::Pressed
        } else {
            WidgetState::Hover
        };
    }
    list
}

fn push_garage_top_bar(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    let bar = ui.anchor(Anchor::Top, [ui.u(ui.viewport().w), TOP_BAR_H_U], [0.0, 0.0]);
    put(list, E::TopBar, bar, plate(theme, theme.plates.steel_brushed, 0.0));
    let rule = Rect::new(bar.x, bar.bottom() - ui.px(1.0), bar.w, ui.px(1.0));
    put(
        list,
        E::TopBarRule,
        rule,
        Payload::Bar { frac: 0.0, fill: theme.text.label_dim, back: theme.text.label_dim },
    );

    // BATTLE: the one commit, in the one red — unless the hull is locked in a running battle.
    let battle = ui.anchor(Anchor::Top, BATTLE_SIZE_U, [0.0, BATTLE_TOP_U]);
    let painted = theme.plates.steel_painted;
    match state.locked_remaining_s {
        Some(remaining) => {
            put_control(
                list,
                E::BattleButton,
                battle,
                Payload::Plate {
                    tile: painted.tile,
                    radius_u: 3.0,
                    bevel_u: 1.0,
                    color: painted.color,
                },
                WidgetState::Disabled,
            );
            put(
                list,
                E::BattleLabel,
                battle,
                text(
                    &format!("{} \u{b7} {}", words::IN_BATTLE, lock_clock_word(remaining)),
                    Style::LABEL,
                    20.0,
                    Align::Center,
                    theme.text.label,
                    DigitMode::Tabular,
                ),
            );
        }
        None => {
            put_control(
                list,
                E::BattleButton,
                battle,
                Payload::Plate {
                    tile: painted.tile,
                    radius_u: 3.0,
                    bevel_u: 1.0,
                    color: theme.semantic.commit,
                },
                WidgetState::Idle,
            );
            put(
                list,
                E::BattleLabel,
                battle,
                text(
                    words::BATTLE,
                    Style::BANNER,
                    26.0,
                    Align::Center,
                    theme.text.value,
                    DigitMode::Proportional,
                ),
            );
        }
    }

    // The map row: which world BATTLE deploys into; AUTO reads dim, a choice reads as a value.
    let map = ui.anchor(Anchor::Top, MAP_SIZE_U, MAP_OFFSET_U);
    put_control(list, E::MapRow, map, plate(theme, painted, 2.0), WidgetState::Idle);
    let set = state.selected_map().is_some();
    put(
        list,
        E::MapLabel,
        map,
        text(
            map_pick_label(state.selected_map()),
            Style::LABEL,
            18.0,
            Align::Center,
            if set { theme.text.value } else { theme.text.label },
            DigitMode::Proportional,
        ),
    );

    // The tabs: the active one in the lamp; both are the click targets they look like.
    for (id, x, word, active) in [
        (E::TabGarage, TAB_GARAGE_X_U, words::TAB_GARAGE, state.view() == GarageView::Hangar),
        (E::TabTechTree, TAB_TREE_X_U, words::TAB_TECH_TREE, state.view() == GarageView::TechTree),
    ] {
        let rect = ui.anchor(Anchor::Top, TAB_SIZE_U, [x, TAB_TOP_U]);
        put_control(
            list,
            id,
            rect,
            text(
                word,
                Style::LABEL,
                22.0,
                Align::Center,
                if active { theme.lamp } else { theme.text.label },
                DigitMode::Proportional,
            ),
            WidgetState::Idle,
        );
    }
}

fn push_nameplate(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    let kind = state.selected_vehicle();
    let plate_rect = ui.anchor(Anchor::Top, NAME_SIZE_U, [0.0, NAME_TOP_U]);
    put(list, E::NamePlate, plate_rect, plate(theme, theme.plates.steel_painted, 3.0));
    let line = |top_u: f32, h_u: f32| {
        Rect::new(plate_rect.x, plate_rect.y + ui.px(top_u), plate_rect.w, ui.px(h_u))
    };
    put(
        list,
        E::NameText,
        line(8.0, 36.0),
        text(
            kind.display_name(),
            Style::BANNER,
            30.0,
            Align::Center,
            theme.text.value,
            DigitMode::Proportional,
        ),
    );
    let identity = format!(
        "{} \u{b7} {} \u{b7} {}",
        game_core::tier_roman(kind.tier()),
        kind.class().label().to_uppercase(),
        kind.nation().label().to_uppercase()
    );
    put(
        list,
        E::NameLine,
        line(46.0, 22.0),
        text(
            &identity,
            Style::LABEL,
            18.0,
            Align::Center,
            theme.text.label,
            DigitMode::Proportional,
        ),
    );
    put(
        list,
        E::NameRole,
        line(70.0, 22.0),
        text(
            &kind.role_line().to_uppercase(),
            Style::VALUE,
            16.0,
            Align::Center,
            theme.lamp,
            DigitMode::Proportional,
        ),
    );
    // L2: the hero wears battle damage — the plate says so and offers the fix; during the beat
    // the line is the work in progress. Earned state only: a clean machine shows no tag.
    let tag = if state.repair_active() {
        Some((words::NAME_REPAIRING.to_string(), theme.lamp))
    } else if state.hero_is_marked() {
        Some((
            format!(
                "{} \u{b7} {} {}",
                words::NAME_DAMAGED,
                state.key_labels().repair,
                words::HINT_REPAIR
            ),
            theme.semantic.module[1],
        ))
    } else {
        None
    };
    if let Some((word, color)) = tag {
        put(
            list,
            E::NameTag,
            ui.anchor(Anchor::Top, [NAME_SIZE_U[0], 24.0], [0.0, TAG_TOP_U]),
            text(&word, Style::LABEL, 18.0, Align::Center, color, DigitMode::Proportional),
        );
    }
}

/// The armour inspector's millimetre legend (R1): the swatches SAMPLE `color_for_mm` at the
/// ramp's own stops, so the legend cannot drift from the scale it explains.
pub(crate) const LEGEND_STOPS_MM: [f32; 5] = [10.0, 40.0, 90.0, 150.0, 230.0];

pub(crate) fn legend_swatches() -> [(f32, [f32; 3]); 5] {
    LEGEND_STOPS_MM.map(|mm| (mm, crate::vehicle::armor_overlay::color_for_mm(mm)))
}

fn push_legend(list: &mut DrawList<E>, ui: &Ui, theme: &Theme) {
    let plate_rect = ui.anchor(Anchor::Top, LEGEND_SIZE_U, [0.0, LEGEND_TOP_U]);
    let enamel = theme.plates.enamel_black;
    put(list, E::LegendPlate, plate_rect, plate(theme, enamel, 3.0));
    let pad = ui.px(PAD_U);
    put(
        list,
        E::LegendTitle,
        Rect::new(plate_rect.x + pad, plate_rect.y, ui.px(110.0), plate_rect.h),
        text(
            words::LEGEND_TITLE,
            Style::LABEL,
            18.0,
            Align::Left,
            theme.text.label,
            DigitMode::Proportional,
        ),
    );
    let unit_w = ui.px(44.0);
    let row_left = plate_rect.x + pad + ui.px(110.0);
    let row_right = plate_rect.right() - pad - unit_w;
    let swatches = legend_swatches();
    let step = (row_right - row_left) / swatches.len() as f32;
    for (index, (mm, rgb)) in swatches.iter().enumerate() {
        let x = row_left + step * index as f32;
        let swatch = Rect::new(x, plate_rect.y + ui.px(16.0), ui.px(24.0), ui.px(24.0));
        let color = [rgb[0], rgb[1], rgb[2], 0.95];
        put(
            list,
            E::LegendSwatch(index as u8),
            swatch,
            Payload::Bar { frac: 0.0, fill: color, back: color },
        );
        put(
            list,
            E::LegendLabel(index as u8),
            Rect::new(x + ui.px(30.0), plate_rect.y, step - ui.px(30.0), plate_rect.h),
            text(
                &format!("{}", *mm as i32),
                Style::VALUE,
                16.0,
                Align::Left,
                theme.text.value,
                DigitMode::Tabular,
            ),
        );
    }
    put(
        list,
        E::LegendUnit,
        Rect::new(row_right, plate_rect.y, unit_w, plate_rect.h),
        text(
            words::LEGEND_UNIT,
            Style::LABEL,
            16.0,
            Align::Right,
            theme.text.label,
            DigitMode::Proportional,
        ),
    );
}

fn push_column_header(
    list: &mut DrawList<E>,
    ui: &Ui,
    theme: &Theme,
    plate_rect: Rect,
    header: E,
    rule: E,
    word: &str,
) -> f32 {
    let pad = ui.px(PAD_U);
    let head = Rect::new(
        plate_rect.x + pad,
        plate_rect.y + pad,
        plate_rect.w - 2.0 * pad,
        ui.px(HEADER_H_U),
    );
    put(
        list,
        header,
        head,
        text(word, Style::LABEL, 20.0, Align::Left, theme.text.label, DigitMode::Proportional),
    );
    let rule_rect = Rect::new(head.x, head.bottom() + ui.px(4.0), head.w, ui.px(1.0));
    put(
        list,
        rule,
        rule_rect,
        Payload::Bar { frac: 0.0, fill: theme.text.label_dim, back: theme.text.label_dim },
    );
    rule_rect.bottom() + ui.px(10.0)
}

fn push_crew(list: &mut DrawList<E>, ui: &Ui, theme: &Theme) {
    let plate_rect = ui.anchor(Anchor::Left, CREW_SIZE_U, CREW_OFFSET_U);
    put(list, E::CrewPlate, plate_rect, plate(theme, theme.plates.steel_painted, 3.0));
    let top =
        push_column_header(list, ui, theme, plate_rect, E::CrewHeader, E::CrewRule, words::CREW);
    let pad = ui.px(PAD_U);
    for (i, role) in game_core::Crew::roles().into_iter().enumerate() {
        let y = top + i as f32 * ui.px(CREW_ROW_H_U);
        let icon = Rect::new(plate_rect.x + pad, y + ui.px(8.0), ui.px(40.0), ui.px(40.0));
        put(
            list,
            E::CrewIcon(i as u8),
            icon,
            Payload::Icon { icon: crate::hud::icons::HudIcon::Crew, color: theme.text.label },
        );
        put(
            list,
            E::CrewRole(i as u8),
            Rect::new(
                icon.right() + ui.px(14.0),
                y,
                plate_rect.w - 2.0 * pad - ui.px(54.0),
                ui.px(56.0),
            ),
            text(
                &role.label().to_uppercase(),
                Style::LABEL,
                20.0,
                Align::Left,
                theme.text.value,
                DigitMode::Proportional,
            ),
        );
    }
}

fn push_stats(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    let spec = state.draft().assembled_spec();
    // G3: with a hull to compare against, the numbers sit in two columns and the delta in a
    // third; the plate widens to hold them.
    let other = state.compare_spec();
    let size = if other.is_some() { STATS_COMPARE_SIZE_U } else { STATS_SIZE_U };
    let plate_rect = ui.anchor(Anchor::TopRight, size, STATS_OFFSET_U);
    put(list, E::StatsPlate, plate_rect, plate(theme, theme.plates.steel_painted, 3.0));
    let top = push_column_header(
        list,
        ui,
        theme,
        plate_rect,
        E::StatsHeader,
        E::StatsRule,
        words::VEHICLE,
    );
    let pad = ui.px(PAD_U);
    let text_right = plate_rect.right() - pad;
    let own_right =
        if other.is_some() { text_right - ui.px(COMPARE_OWN_RIGHT_U) } else { text_right };
    let other_right = text_right - ui.px(COMPARE_OTHER_RIGHT_U);
    let header = Rect::new(
        plate_rect.x + pad,
        plate_rect.y + pad,
        plate_rect.w - 2.0 * pad,
        ui.px(HEADER_H_U),
    );
    // The matchmaking bracket on the header row: a battle is tier ±1.
    put(
        list,
        E::StatsTier,
        Rect::new(header.x, header.y, own_right - header.x, header.h),
        text(
            game_core::tier_roman(spec.kind.tier()),
            Style::BANNER,
            20.0,
            Align::Right,
            theme.lamp,
            DigitMode::Proportional,
        ),
    );
    if let Some(other) = &other {
        put(
            list,
            E::CompareName,
            header,
            text(
                &format!(
                    "{} \u{b7} {}",
                    other.kind.short_name().to_uppercase(),
                    game_core::tier_roman(other.kind.tier())
                ),
                Style::LABEL,
                18.0,
                Align::Right,
                theme.text.value,
                DigitMode::Proportional,
            ),
        );
    }
    let dim = theme.text.label_dim;
    let other_rows = other.as_ref().map(stat_rows);
    for (i, row) in stat_rows(&spec).iter().enumerate() {
        let index = i as u8;
        let y = top + i as f32 * ui.px(STAT_ROW_H_U);
        let icon = Rect::new(plate_rect.x + pad, y + ui.px(6.0), ui.px(26.0), ui.px(26.0));
        put(
            list,
            E::StatIcon(index),
            icon,
            Payload::Icon { icon: row.kind.icon(), color: theme.text.label },
        );
        let text_left = icon.right() + ui.px(12.0);
        let text_w = own_right - text_left;
        let line = Rect::new(text_left, y + ui.px(2.0), text_w, ui.px(20.0));
        put(
            list,
            E::StatLabel(index),
            line,
            text(
                row.label,
                Style::LABEL,
                16.0,
                Align::Left,
                theme.text.label,
                DigitMode::Proportional,
            ),
        );
        put(
            list,
            E::StatValue(index),
            line,
            text(
                &row.value,
                Style::VALUE,
                18.0,
                Align::Right,
                theme.text.value,
                DigitMode::Tabular,
            ),
        );
        if let (Some(other), Some(rows)) = (&other, &other_rows) {
            put(
                list,
                E::StatOther(index),
                Rect::new(own_right, line.y, other_right - own_right, line.h),
                text(
                    &rows[i].value,
                    Style::VALUE,
                    18.0,
                    Align::Right,
                    theme.text.label,
                    DigitMode::Tabular,
                ),
            );
            let delta = row.kind.measure(&spec) - row.kind.measure(other);
            let (word, color) = fmt_delta(theme, row.kind.higher_is_better(), delta);
            put(
                list,
                E::StatDelta(index),
                Rect::new(other_right, line.y, text_right - other_right, line.h),
                text(&word, Style::VALUE, 16.0, Align::Right, color, DigitMode::Tabular),
            );
        }
        let bar = Rect::new(text_left, y + ui.px(28.0), text_w, ui.px(3.0));
        put(
            list,
            E::StatBar(index),
            bar,
            Payload::Bar { frac: row.frac, fill: theme.lamp, back: [dim[0], dim[1], dim[2], 0.35] },
        );
        let tick = Rect::new(
            text_left + text_w * row.median_frac - ui.px(1.0),
            y + ui.px(25.0),
            ui.px(2.0),
            ui.px(9.0),
        );
        put(
            list,
            E::StatTick(index),
            tick,
            Payload::Bar { frac: 0.0, fill: theme.text.value, back: theme.text.value },
        );
    }
}

fn push_loadout(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    let strip = ui.anchor(Anchor::Bottom, LOADOUT_SIZE_U, LOADOUT_OFFSET_U);
    put(list, E::LoadoutPlate, strip, plate(theme, theme.plates.steel_brushed, 3.0));
    let painted = theme.plates.steel_painted;
    let slot_top = strip.y + (strip.h - ui.px(SLOT_SIZE_U[1])) * 0.5;
    let draft = state.draft();
    for (i, slot) in FitSlot::ALL.into_iter().enumerate() {
        let index = i as u8;
        let rect = Rect::new(
            strip.x + ui.px(MODULE_START_U + i as f32 * SLOT_PITCH_U),
            slot_top,
            ui.px(SLOT_SIZE_U[0]),
            ui.px(SLOT_SIZE_U[1]),
        );
        let rejected = state.rejected_slot() == Some(slot);
        let color = if rejected { theme.semantic.hp_ramp[2] } else { painted.color };
        put_control(
            list,
            E::ModuleSlot(index),
            rect,
            Payload::Plate { tile: painted.tile, radius_u: 2.0, bevel_u: 1.0, color },
            if state.focused_slot() == slot { WidgetState::Focused } else { WidgetState::Idle },
        );
        let tint = if draft.has_choice(slot) { theme.text.value } else { theme.text.label_dim };
        put(
            list,
            E::ModuleIcon(index),
            Rect::new(
                rect.x + (rect.w - ui.px(48.0)) * 0.5,
                rect.y + ui.px(10.0),
                ui.px(48.0),
                ui.px(48.0),
            ),
            Payload::Icon { icon: slot_icon(slot), color: tint },
        );
        let summary =
            Rect::new(rect.x + ui.px(4.0), rect.y + ui.px(64.0), rect.w - ui.px(8.0), ui.px(52.0));
        let z = list.len() as i16;
        list.push(
            Element::new(
                E::ModuleSummary(index),
                summary,
                text(
                    &draft.current_module_summary(slot),
                    Style::LABEL,
                    16.0,
                    Align::Center,
                    theme.text.label,
                    DigitMode::Proportional,
                ),
            )
            .z(z)
            .clipped(Some(summary)),
        );
    }
    // The ammunition: which round, what it does (pen at 100 m · damage), how many.
    let selected = draft.ammo_index();
    let counts = draft.ammo_counts();
    for (i, shell) in draft.ammo_options().iter().enumerate() {
        let index = i as u8;
        let rect = Rect::new(
            strip.x + ui.px(AMMO_START_U + i as f32 * SLOT_PITCH_U),
            slot_top,
            ui.px(SLOT_SIZE_U[0]),
            ui.px(SLOT_SIZE_U[1]),
        );
        put_control(
            list,
            E::AmmoSlot(index),
            rect,
            Payload::Plate {
                tile: painted.tile,
                radius_u: 2.0,
                bevel_u: 1.0,
                color: painted.color,
            },
            if i == selected { WidgetState::Focused } else { WidgetState::Idle },
        );
        put(
            list,
            E::AmmoIcon(index),
            Rect::new(
                rect.x + (rect.w - ui.px(34.0)) * 0.5,
                rect.y + ui.px(6.0),
                ui.px(34.0),
                ui.px(34.0),
            ),
            Payload::Icon { icon: ammo_icon(shell.shell_type), color: theme.text.value },
        );
        let name = shell.round.map_or_else(
            || {
                match shell.shell_type {
                    game_core::ShellType::ArmorPiercing => "AP",
                    game_core::ShellType::Apcr => "APCR",
                    game_core::ShellType::Heat => "HEAT",
                    game_core::ShellType::HighExplosive => "HE",
                }
                .to_string()
            },
            |round| round.designation().to_string(),
        );
        put(
            list,
            E::AmmoName(index),
            Rect::new(rect.x, rect.y + ui.px(42.0), rect.w, ui.px(18.0)),
            text(
                &name,
                Style::LABEL,
                16.0,
                Align::Center,
                theme.text.label,
                DigitMode::Proportional,
            ),
        );
        put(
            list,
            E::AmmoFigures(index),
            Rect::new(rect.x, rect.y + ui.px(62.0), rect.w, ui.px(18.0)),
            text(
                &format!(
                    "{} \u{b7} {}",
                    shell.penetration_mm_at_100m.round() as i32,
                    shell.damage_hp
                ),
                Style::VALUE,
                16.0,
                Align::Center,
                theme.text.value,
                DigitMode::Tabular,
            ),
        );
        // The count row: − N + — the zones are real controls and hit-test before the slot.
        let band_y = rect.y + ui.px(90.0);
        let band_h = ui.px(28.0);
        put_control(
            list,
            E::AmmoMinus(index),
            Rect::new(rect.x, band_y, ui.px(28.0), band_h),
            text("-", Style::LABEL, 22.0, Align::Center, theme.text.value, DigitMode::Proportional),
            WidgetState::Idle,
        );
        put(
            list,
            E::AmmoCount(index),
            Rect::new(rect.x + ui.px(28.0), band_y, rect.w - ui.px(56.0), band_h),
            text(
                &format!("{}", counts.get(i).copied().unwrap_or(0)),
                Style::VALUE,
                20.0,
                Align::Center,
                theme.text.value,
                DigitMode::Tabular,
            ),
        );
        put_control(
            list,
            E::AmmoPlus(index),
            Rect::new(rect.right() - ui.px(28.0), band_y, ui.px(28.0), band_h),
            text("+", Style::LABEL, 22.0, Align::Center, theme.text.value, DigitMode::Proportional),
            WidgetState::Idle,
        );
    }
    // The rack: how full against the vehicle's authored capacity.
    put(
        list,
        E::RackTotal,
        Rect::new(
            strip.x + ui.px(RACK_LEFT_U),
            slot_top,
            strip.right() - ui.px(8.0) - (strip.x + ui.px(RACK_LEFT_U)),
            ui.px(SLOT_SIZE_U[1]),
        ),
        text(
            &format!("{} / {}", draft.rack_total(), draft.rack_capacity()),
            Style::VALUE,
            18.0,
            Align::Center,
            theme.text.value,
            DigitMode::Tabular,
        ),
    );
}

fn push_carousel(list: &mut DrawList<E>, ui: &Ui, theme: &Theme, state: &GarageState) {
    // G9: the cells are the roster the chips let through.
    let roster = state.roster();
    let count = roster.len();
    let window = carousel_window(count, state.carousel_scroll());
    let visible = window.len();
    let strip = ui.anchor(
        Anchor::Bottom,
        [visible.max(1) as f32 * CAR_PITCH_U + 40.0, CAR_CELL_SIZE_U[1] + 20.0],
        [0.0, CAR_BOTTOM_U],
    );
    put(list, E::CarouselPlate, strip, plate(theme, theme.plates.steel_brushed, 3.0));
    let painted = theme.plates.steel_painted;
    for (slot, at) in window.clone().enumerate() {
        let index = slot as u8;
        let kind = roster[at];
        let cell = Rect::new(
            strip.x + ui.px(20.0 + slot as f32 * CAR_PITCH_U),
            strip.y + ui.px(10.0),
            ui.px(CAR_CELL_SIZE_U[0]),
            ui.px(CAR_CELL_SIZE_U[1]),
        );
        let selected = kind == state.selected_vehicle();
        let compared = state.compare() == Some(kind);
        put_control(
            list,
            E::CarouselCell(index),
            cell,
            Payload::Plate {
                tile: painted.tile,
                radius_u: 2.0,
                bevel_u: 1.0,
                color: painted.color,
            },
            if selected { WidgetState::Focused } else { WidgetState::Idle },
        );
        let nation = kind.nation();
        let c = nation.color();
        // G3: the compared hull's cell says so where its nation goes.
        let (tag, tag_color) = if compared {
            (format!("{} \u{b7} {}", words::COMPARE_TAG, nation.label().to_uppercase()), theme.lamp)
        } else {
            (nation.label().to_uppercase(), [c[0], c[1], c[2], 0.95])
        };
        put(
            list,
            E::CarouselNation(index),
            Rect::new(cell.x, cell.y + ui.px(6.0), cell.w, ui.px(18.0)),
            text(&tag, Style::LABEL, 16.0, Align::Center, tag_color, DigitMode::Proportional),
        );
        put(
            list,
            E::CarouselName(index),
            Rect::new(cell.x, cell.y + ui.px(26.0), cell.w, ui.px(24.0)),
            text(
                kind.short_name(),
                Style::LABEL,
                20.0,
                Align::Center,
                if selected { theme.lamp } else { theme.text.value },
                DigitMode::Proportional,
            ),
        );
        let card = Rect::new(
            cell.x + ui.px(4.0),
            cell.y + ui.px(54.0),
            cell.w - ui.px(8.0),
            cell.h - ui.px(58.0),
        );
        let ink = if selected { theme.text.value } else { theme.text.label };
        let [hull, turret, barrel] = silhouette_rects(kind, card, ui.px(1.0));
        for (id, rect) in [
            (E::SilhouetteHull(index), hull),
            (E::SilhouetteTurret(index), turret),
            (E::SilhouetteBarrel(index), barrel),
        ] {
            put(list, id, rect, Payload::Bar { frac: 0.0, fill: ink, back: ink });
        }
    }
    if carousel_overflows(count) {
        let dx = strip.w * 0.5 + ui.px(30.0);
        for (index, sign, glyph) in [(0u8, -1.0, "<"), (1u8, 1.0, ">")] {
            let rect =
                ui.anchor(Anchor::Bottom, CAR_ARROW_SIZE_U, [sign * ui.u(dx), CAR_BOTTOM_U + 10.0]);
            put_control(
                list,
                E::CarouselArrow(index),
                rect,
                plate(theme, painted, 2.0),
                WidgetState::Idle,
            );
            put(
                list,
                E::CarouselArrowGlyph(index),
                rect,
                text(
                    glyph,
                    Style::LABEL,
                    24.0,
                    Align::Center,
                    theme.text.value,
                    DigitMode::Proportional,
                ),
            );
        }
    }
}

fn push_options(
    list: &mut DrawList<E>,
    ui: &Ui,
    theme: &Theme,
    state: &GarageState,
    slot: FitSlot,
) {
    let options = state.draft().module_options(slot);
    if options.len() < 2 {
        return;
    }
    let strip = ui.anchor(Anchor::Bottom, LOADOUT_SIZE_U, LOADOUT_OFFSET_U);
    let slot_center_x =
        strip.x + ui.px(MODULE_START_U + slot.index() as f32 * SLOT_PITCH_U + SLOT_SIZE_U[0] * 0.5);
    let row_w = ui.px(OPTION_ROW_SIZE_U[0]);
    let x = (slot_center_x - row_w * 0.5).clamp(ui.px(20.0), ui.viewport().w - row_w - ui.px(20.0));
    let bottom_row_top = strip.y - ui.px(12.0) - ui.px(OPTION_ROW_SIZE_U[1]);
    let row_rect = |i: usize| {
        Rect::new(
            x,
            bottom_row_top - i as f32 * ui.px(OPTION_ROW_PITCH_U),
            row_w,
            ui.px(OPTION_ROW_SIZE_U[1]),
        )
    };
    let top_row = row_rect(options.len() - 1);
    let header = Rect::new(x, top_row.y - ui.px(34.0), row_w, ui.px(28.0));
    let pad = ui.px(12.0);
    let panel = Rect::new(
        x - pad,
        header.y - pad,
        row_w + 2.0 * pad,
        row_rect(0).bottom() + pad - (header.y - pad),
    );
    put(list, E::OptionsPlate, panel, plate(theme, theme.plates.enamel_black, 4.0));
    put(
        list,
        E::OptionsHeader,
        header,
        text(
            slot_label(slot),
            Style::LABEL,
            18.0,
            Align::Center,
            theme.text.label,
            DigitMode::Proportional,
        ),
    );
    let painted = theme.plates.steel_painted;
    let installed_stat = options.iter().find(|o| o.installed).map_or(0.0, |o| o.stat);
    for (i, option) in options.iter().enumerate() {
        let index = i as u8;
        let rect = row_rect(i);
        put_control(
            list,
            E::OptionRow(index),
            rect,
            Payload::Plate {
                tile: painted.tile,
                radius_u: 2.0,
                bevel_u: 1.0,
                color: painted.color,
            },
            if option.installed { WidgetState::Focused } else { WidgetState::Idle },
        );
        let inner = rect.inset(ui.px(10.0));
        put(
            list,
            E::OptionName(index),
            inner,
            text(
                &option.name,
                Style::LABEL,
                18.0,
                Align::Left,
                if option.installed { theme.lamp } else { theme.text.value },
                DigitMode::Proportional,
            ),
        );
        put(
            list,
            E::OptionStat(index),
            Rect::new(inner.x, inner.y, inner.w, ui.px(18.0)),
            text(
                &format!("{} {}", fmt_stat(option.stat), option.unit),
                Style::VALUE,
                16.0,
                Align::Right,
                theme.text.value,
                DigitMode::Tabular,
            ),
        );
        if !option.installed {
            let delta = option.stat - installed_stat;
            if delta.abs() > 1.0e-3 {
                let better = (delta > 0.0) == option.higher_is_better;
                let sign = if delta > 0.0 { "+" } else { "" };
                put(
                    list,
                    E::OptionDelta(index),
                    Rect::new(inner.x, inner.bottom() - ui.px(16.0), inner.w, ui.px(16.0)),
                    text(
                        &format!("{sign}{}", fmt_stat(delta)),
                        Style::VALUE,
                        16.0,
                        Align::Right,
                        if better { theme.semantic.hp_ramp[0] } else { theme.semantic.hp_ramp[2] },
                        DigitMode::Tabular,
                    ),
                );
            }
        }
    }
}

/// Compact stat formatting: whole numbers for big or round values, one decimal for small ones.
fn fmt_stat(x: f32) -> String {
    if x.abs() >= 100.0 || x.fract().abs() < 0.05 {
        format!("{}", x.round() as i32)
    } else {
        format!("{x:.1}")
    }
}

/// A signed delta as the compare column prints it (G3): `+` for more, `-` for less, `=` for
/// the same; the ramp's good when it is the better way round for this row, its bad otherwise.
fn fmt_delta(theme: &Theme, higher_is_better: bool, delta: f32) -> (String, [f32; 4]) {
    if delta.abs() < 1.0e-3 {
        return ("=".to_string(), theme.text.label_dim);
    }
    let better = (delta > 0.0) == higher_is_better;
    let sign = if delta > 0.0 { "+" } else { "-" };
    let color = if better { theme.semantic.hp_ramp[0] } else { theme.semantic.hp_ramp[2] };
    (format!("{sign}{}", fmt_stat(delta.abs())), color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::garage::stats::StatKind;

    fn hangar() -> (GarageState, Ui) {
        let mut state = GarageState::default();
        state.open();
        let ui = state.ui();
        (state, ui)
    }

    fn text_of(list: &DrawList<E>, id: E) -> String {
        match &list.find(id).expect("element").payload {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{id:?} is not text: {other:?}"),
        }
    }

    /// G1: every stat row on the screen carries its label, its number and its bar — the same
    /// rows the column's content lock walks — inside the plate.
    #[test]
    fn every_stat_row_is_drawn_with_its_label_number_bar_and_tick() {
        let (state, ui) = hangar();
        let list = build_screen_list(&state, &ui, None);
        let plate = list.find(E::StatsPlate).expect("plate").rect;
        for (i, kind) in StatKind::ALL.iter().enumerate() {
            let index = i as u8;
            assert_eq!(text_of(&list, E::StatLabel(index)), kind.label());
            assert!(text_of(&list, E::StatValue(index)).chars().any(|c| c.is_ascii_digit()));
            let bar = list.find(E::StatBar(index)).expect("bar");
            let Payload::Bar { frac, .. } = bar.payload else { panic!("a bar") };
            assert!((0.0..=1.0).contains(&frac));
            assert!(plate.encloses(&bar.rect), "{kind:?}'s bar leaves the plate");
            assert!(list.find(E::StatTick(index)).is_some());
        }
        assert!(list.find(E::StatLabel(StatKind::ALL.len() as u8)).is_none());
    }

    /// G4: the nameplate names the hull, its tier, class and nation, and its role — for every
    /// playable vehicle, and it follows the selection.
    #[test]
    fn the_nameplate_names_the_class_and_the_role() {
        for kind in VehicleKind::PLAYABLE {
            let mut state = GarageState::default();
            state.open();
            state.select_vehicle(kind);
            let ui = state.ui();
            let list = build_screen_list(&state, &ui, None);
            assert_eq!(text_of(&list, E::NameText), kind.display_name());
            let line = text_of(&list, E::NameLine);
            assert!(line.contains(&kind.class().label().to_uppercase()), "{kind:?}: {line}");
            assert!(line.contains(&kind.nation().label().to_uppercase()), "{kind:?}: {line}");
            assert!(line.starts_with(game_core::tier_roman(kind.tier())), "{kind:?}: {line}");
            let role = text_of(&list, E::NameRole);
            assert_eq!(role, kind.role_line().to_uppercase());
            assert!(role.len() > 20, "{kind:?}: a role is a sentence, not a tag: {role}");
        }
    }

    /// G5: no garage string renders below the legibility floor; the ammunition slots print
    /// the round's designation and its penetration and damage; the rack says how full it is.
    #[test]
    fn no_garage_string_renders_below_the_legibility_floor() {
        let (mut state, ui) = hangar();
        state.open_option_list(FitSlot::Gun);
        state.toggle_inspector();
        let list = build_screen_list(&state, &ui, None);
        for element in list.iter() {
            if let Payload::Text { size_u, text, .. } = &element.payload {
                assert!(
                    *size_u >= GARAGE_TEXT_FLOOR_U,
                    "{:?} prints {text:?} at {size_u} u",
                    element.id
                );
            }
        }
        let draft = state.draft();
        for (i, shell) in draft.ammo_options().iter().enumerate() {
            let figures = text_of(&list, E::AmmoFigures(i as u8));
            assert!(
                figures.starts_with(&format!("{}", shell.penetration_mm_at_100m.round() as i32)),
                "{figures}"
            );
            assert!(figures.ends_with(&format!("{}", shell.damage_hp)), "{figures}");
            assert!(!text_of(&list, E::AmmoName(i as u8)).is_empty());
        }
        assert_eq!(
            text_of(&list, E::RackTotal),
            format!("{} / {}", draft.rack_total(), draft.rack_capacity())
        );
    }

    /// G7: the one red is the commit's — BATTLE — and nothing else on the screen wears it; a
    /// locked hull's BATTLE wears none at all.
    #[test]
    fn signal_red_is_only_worn_by_commit() {
        let (mut state, ui) = hangar();
        state.open_option_list(FitSlot::Gun);
        let theme = Theme::standard();
        let list = build_screen_list(&state, &ui, None);
        let red: Vec<E> = list
            .iter()
            .filter(|e| matches!(&e.payload, Payload::Plate { color, .. } | Payload::Bar { fill: color, .. } | Payload::Text { color, .. } if *color == theme.semantic.commit))
            .map(|e| e.id)
            .collect();
        assert_eq!(red, vec![E::BattleButton]);
        state.set_locked(Some(95.0));
        let list = build_screen_list(&state, &ui, None);
        assert!(!list.iter().any(|e| matches!(&e.payload, Payload::Plate { color, .. } if *color == theme.semantic.commit)));
        assert_eq!(list.find(E::BattleButton).expect("battle").state, WidgetState::Disabled);
        assert_eq!(text_of(&list, E::BattleLabel), format!("{} \u{b7} 1:35", words::IN_BATTLE));
        // The tree's BACK is plain steel now, never the signal: the plate it is cut from is
        // named, and the name is not the red.
        assert_ne!(
            super::super::panels::techtree::BACK_PLATE,
            ui_kit::theme::color::SIGNAL,
            "BACK wore the commit red"
        );
        assert_eq!(super::super::panels::techtree::BACK_PLATE, ui_kit::theme::color::SLOT);
    }

    /// The hit test reads the rectangles the screen draws: every control answers at its
    /// centre, the count zones before their slot, an open list's rows above the strip, and
    /// empty floor is the scene; hovering a control lights it.
    #[test]
    fn the_hit_test_answers_the_rects_the_screen_draws() {
        let (mut state, ui) = hangar();
        state.select_vehicle(VehicleKind::BENCHMARK);
        let hit = |state: &mut GarageState, element: E, shift: bool| {
            assert!(state.set_cursor_on(element), "{element:?} is drawn");
            hit_screen(state, &state.ui(), shift)
        };
        assert_eq!(hit(&mut state, E::BattleButton, false), GarageHit::Battle);
        assert_eq!(
            hit(&mut state, E::BattleLabel, false),
            GarageHit::Battle,
            "the label is on the button"
        );
        assert_eq!(hit(&mut state, E::MapRow, false), GarageHit::MapCycle(1));
        assert_eq!(hit(&mut state, E::MapRow, true), GarageHit::MapCycle(-1));
        assert_eq!(hit(&mut state, E::TabTechTree, false), GarageHit::OpenTechTree);
        assert_eq!(
            hit(&mut state, E::TabGarage, false),
            GarageHit::Scene,
            "GARAGE is already the view"
        );
        assert_eq!(
            hit(&mut state, E::ModuleSlot(1), false),
            GarageHit::ModuleCycle(FitSlot::Gun, 1)
        );
        assert_eq!(
            hit(&mut state, E::ModuleSlot(1), true),
            GarageHit::ModuleCycle(FitSlot::Gun, -1)
        );
        assert_eq!(hit(&mut state, E::AmmoSlot(1), false), GarageHit::AmmoSelect(1));
        assert_eq!(hit(&mut state, E::AmmoMinus(1), false), GarageHit::AmmoAdjust(1, -1));
        assert_eq!(hit(&mut state, E::AmmoPlus(1), false), GarageHit::AmmoAdjust(1, 1));
        assert_eq!(hit(&mut state, E::CarouselCell(3), false), GarageHit::Vehicle(3));
        assert_eq!(hit(&mut state, E::CarouselCell(3), true), GarageHit::Compare(3));
        assert_eq!(hit(&mut state, E::Chip(0), false), GarageHit::Chip(Chip::Class, 1));
        assert_eq!(hit(&mut state, E::Chip(2), true), GarageHit::Chip(Chip::Tier, -1));
        assert!(!state.set_cursor_on(E::CarouselArrow(0)), "the roster fits: no arrows");
        state.set_cursor([0.0, 0.0]);
        assert_eq!(hit_screen(&state, &ui, false), GarageHit::Scene);
        state.open_option_list(FitSlot::Gun);
        assert_eq!(hit(&mut state, E::OptionRow(1), false), GarageHit::OptionRow(FitSlot::Gun, 1));
        // The hover lights the control under the cursor and nothing else.
        state.set_cursor_on(E::BattleButton);
        let list = build_screen(&state, &ui, &Theme::standard());
        assert_eq!(list.find(E::BattleButton).expect("battle").state, WidgetState::Hover);
        assert_eq!(list.find(E::MapRow).expect("map").state, WidgetState::Idle);
        // The tree: the tabs answer, and the nodes still do.
        state.open_tech_tree();
        assert_eq!(hit(&mut state, E::TabGarage, false), GarageHit::CloseTechTree);
        assert_eq!(hit(&mut state, E::TabTechTree, false), GarageHit::CloseTechTree);
        assert_eq!(
            hit(&mut state, E::MapRow, false),
            GarageHit::MapCycle(1),
            "the map row lives on the bar in both views"
        );
    }

    /// The legend IS the scale: every swatch equals `color_for_mm` at its printed stop, and the
    /// stops ascend; it is drawn exactly while the inspector is on.
    #[test]
    fn the_legend_is_the_scale_it_explains() {
        for (mm, rgb) in legend_swatches() {
            assert_eq!(rgb, crate::vehicle::armor_overlay::color_for_mm(mm));
        }
        for pair in LEGEND_STOPS_MM.windows(2) {
            assert!(pair[0] < pair[1]);
        }
        let (mut state, ui) = hangar();
        assert!(build_screen_list(&state, &ui, None).find(E::LegendPlate).is_none());
        state.toggle_inspector();
        let list = build_screen_list(&state, &ui, None);
        assert!(list.find(E::LegendPlate).is_some());
        assert_eq!(text_of(&list, E::LegendLabel(4)), "230");
    }

    /// The carousel draws one cell per roster hull with its nation, its name and its card, the
    /// selected one focused; every panel stays inside the frame.
    #[test]
    fn the_carousel_and_every_panel_stay_inside_the_frame() {
        let (state, ui) = hangar();
        let list = build_screen_list(&state, &ui, None);
        let frame = ui.viewport();
        for element in list.iter() {
            if element.rect.w > 0.0 {
                assert!(
                    frame.encloses(&element.rect),
                    "{:?} leaves the frame: {:?}",
                    element.id,
                    element.rect
                );
            }
        }
        for i in 0..VehicleKind::PLAYABLE.len() as u8 {
            assert_eq!(
                text_of(&list, E::CarouselName(i)),
                VehicleKind::PLAYABLE[i as usize].short_name()
            );
            assert!(list.find(E::SilhouetteBarrel(i)).is_some());
        }
        assert_eq!(list.find(E::CarouselCell(0)).expect("cell").state, WidgetState::Focused);
        assert_eq!(list.find(E::CarouselCell(1)).expect("cell").state, WidgetState::Idle);
        let strip = list.find(E::LoadoutPlate).expect("strip").rect;
        let stats = list.find(E::StatsPlate).expect("stats").rect;
        assert!(strip.intersect(&stats).is_none(), "the strip and the column do not overlap");
    }

    /// G6: every clickable control on the screen has its three states — idle, hovered under
    /// the cursor, PRESSED while the button is down on it — and a release on it is the click
    /// the machine reports; a control the builder already marks (the selected cell, a locked
    /// BATTLE) keeps its own word.
    #[test]
    fn every_clickable_has_three_states() {
        let (mut state, ui) = hangar();
        state.select_vehicle(VehicleKind::BENCHMARK);
        state.open_option_list(FitSlot::Gun);
        let theme = Theme::standard();
        let ids: Vec<E> = build_screen_list(&state, &ui, None)
            .iter()
            .filter(|e| e.interactive && e.state == WidgetState::Idle)
            .map(|e| e.id)
            .collect();
        assert!(ids.len() >= 20, "the hangar is full of controls: {}", ids.len());
        let mut walked = 0;
        for id in ids {
            assert!(state.set_cursor_on(id), "{id:?} is drawn");
            if hit_key(&state, &ui) != Some(id) {
                continue; // covered by the modal list's rows
            }
            let list = build_screen(&state, &ui, &theme);
            assert_eq!(list.find(id).expect("drawn").state, WidgetState::Hover, "{id:?} hovers");
            state.press_cursor();
            let list = build_screen(&state, &ui, &theme);
            assert_eq!(list.find(id).expect("drawn").state, WidgetState::Pressed, "{id:?} presses");
            assert_eq!(state.release_cursor(), Some(id), "{id:?} clicks on release");
            let list = build_screen(&state, &ui, &theme);
            assert_eq!(list.find(id).expect("drawn").state, WidgetState::Hover, "{id:?} rests");
            walked += 1;
        }
        assert!(walked >= 20, "{walked} controls walked the three states");
        // A press that leaves the control before its release is no click.
        assert!(state.set_cursor_on(E::MapRow));
        state.press_cursor();
        assert!(state.set_cursor_on(E::TabTechTree));
        assert_eq!(state.release_cursor(), None);
        assert_eq!(
            build_screen(&state, &ui, &theme).find(E::MapRow).expect("map").state,
            WidgetState::Idle
        );
    }

    /// G6: the legend prints the table's keys, never a literal — rebind REPAIR and DEPLOY and
    /// the strip says the new keys and forgets the old; no garage string carries a key of its
    /// own; the tree view keeps the legend on its bar.
    #[test]
    fn the_hint_strip_prints_the_bound_keys_not_literals() {
        use crate::app::garage::hints::KeyLabels;
        use crate::app::keybinds::{Action, KeyBindings, key_label};
        use winit::keyboard::KeyCode;

        let (mut state, ui) = hangar();
        let strip = |state: &GarageState| {
            let list = build_screen_list(state, &ui, None);
            (0..3u8).map(|i| text_of(&list, E::HintLine(i))).collect::<Vec<_>>().join(" ")
        };
        let default = strip(&state);
        let table = KeyBindings::default();
        for (action, word) in [
            (Action::GaragePrev, words::HINT_SELECT),
            (Action::Repair, words::HINT_REPAIR),
            (Action::GarageConfirm, words::BATTLE),
            (Action::GarageBack, words::BACK),
            (Action::Inspector, words::HINT_ARMOUR),
        ] {
            let key = key_label(table.keys(action)[0]);
            assert!(default.contains(&format!("{key} ")) && default.contains(word), "{default}");
        }
        let mut table = KeyBindings::default();
        table.bind(Action::Repair, KeyCode::KeyH);
        table.bind(Action::GarageConfirm, KeyCode::Space);
        state.set_key_labels(KeyLabels::from_table(&table));
        let rebound = strip(&state);
        assert!(rebound.contains(&format!("H {}", words::HINT_REPAIR)), "{rebound}");
        assert!(!rebound.contains(&format!("R {}", words::HINT_REPAIR)), "{rebound}");
        assert!(rebound.contains(&format!("SPACE {}", words::BATTLE)), "{rebound}");
        assert!(!rebound.contains(&format!("ENTER {}", words::BATTLE)), "{rebound}");
        for word in words::ALL {
            assert!(
                !word.starts_with("R ") && !word.contains(" R ") && !word.starts_with("ENTER"),
                "{word:?} carries a key the table owns"
            );
        }
        state.open_tech_tree();
        assert!(build_screen_list(&state, &ui, None).find(E::HintLine(2)).is_some());
    }

    /// G6: a tooltip waits the toolkit's delay, names the control and its key, sits inside
    /// the frame, dies with the hover and never shows under a held button.
    #[test]
    fn a_garage_tooltip_names_the_control_and_its_key_after_the_delay() {
        use ui_kit::interaction::TOOLTIP_DELAY_S;

        let (mut state, ui) = hangar();
        let theme = Theme::standard();
        assert!(state.set_cursor_on(E::BattleButton));
        state.tick_interaction(TOOLTIP_DELAY_S * 0.5);
        assert!(build_screen(&state, &ui, &theme).find(E::TooltipPlate).is_none(), "too soon");
        state.tick_interaction(TOOLTIP_DELAY_S);
        let list = build_screen(&state, &ui, &theme);
        let tip = text_of(&list, E::TooltipText);
        assert!(
            tip.contains(words::TIP_DEPLOY) && tip.contains(&state.key_labels().confirm),
            "{tip}"
        );
        let plate = list.find(E::TooltipPlate).expect("plate").rect;
        assert!(ui.viewport().encloses(&plate));
        assert!(plate.encloses(&list.find(E::TooltipText).expect("text").rect));
        assert!(plate.y >= list.find(E::BattleButton).expect("battle").rect.bottom());
        // A held button shows none; a cursor that leaves takes it away.
        state.press_cursor();
        assert!(build_screen(&state, &ui, &theme).find(E::TooltipPlate).is_none());
        state.release_cursor();
        state.set_cursor([0.0, 0.0]);
        state.tick_interaction(2.0);
        assert!(build_screen(&state, &ui, &theme).find(E::TooltipPlate).is_none());
        // Every control has words; the floor and the plates have none.
        assert!(state.set_cursor_on(E::ModuleSlot(1)));
        state.tick_interaction(1.0);
        assert!(text_of(&build_screen(&state, &ui, &theme), E::TooltipText).contains("GUN"));
        assert_eq!(tooltip_text(&state, E::StatsPlate), None);
    }

    /// G3: shift-click on a carousel cell compares — both numbers on every row, the signed
    /// delta in the ramp's good or bad, the header naming the other hull, the cell tagged VS —
    /// and the same cell again, or selecting that hull, clears it.
    #[test]
    fn a_compare_column_shows_both_numbers_and_the_signed_delta() {
        let (mut state, ui) = hangar();
        state.select_vehicle(VehicleKind::BENCHMARK);
        let theme = Theme::standard();
        assert!(build_screen_list(&state, &ui, None).find(E::StatOther(0)).is_none());
        let narrow = build_screen_list(&state, &ui, None).find(E::StatsPlate).expect("plate").rect;
        assert!(state.set_cursor_on(E::CarouselCell(1)));
        assert_eq!(hit_screen(&state, &ui, true), GarageHit::Compare(1));
        state.toggle_compare(1);
        let other = VehicleKind::PLAYABLE[1];
        assert_eq!(state.compare(), Some(other));
        let theirs = state.compare_spec().expect("compared");
        let own = state.draft().assembled_spec();
        let list = build_screen_list(&state, &ui, None);
        let plate = list.find(E::StatsPlate).expect("plate").rect;
        assert!(plate.w > narrow.w, "the column widens for the second hull");
        assert!(ui.viewport().encloses(&plate));
        let strip = list.find(E::LoadoutPlate).expect("strip").rect;
        assert!(plate.intersect(&strip).is_none(), "the widened column stays off the strip");
        assert_eq!(
            text_of(&list, E::CompareName),
            format!(
                "{} \u{b7} {}",
                other.short_name().to_uppercase(),
                game_core::tier_roman(other.tier())
            )
        );
        assert_eq!(
            text_of(&list, E::CarouselNation(1)),
            format!("VS \u{b7} {}", other.nation().label().to_uppercase())
        );
        let mut signed = 0;
        for (i, kind) in StatKind::ALL.iter().enumerate() {
            let index = i as u8;
            assert_eq!(text_of(&list, E::StatValue(index)), kind.value(&own));
            assert_eq!(text_of(&list, E::StatOther(index)), kind.value(&theirs));
            let delta = kind.measure(&own) - kind.measure(&theirs);
            let printed = text_of(&list, E::StatDelta(index));
            let Payload::Text { color, .. } =
                list.find(E::StatDelta(index)).expect("delta").payload
            else {
                panic!("text")
            };
            if delta.abs() < 1.0e-3 {
                assert_eq!(printed, "=");
            } else {
                signed += 1;
                assert!(
                    printed.starts_with(if delta > 0.0 { "+" } else { "-" }),
                    "{kind:?}: {printed}"
                );
                let better = (delta > 0.0) == kind.higher_is_better();
                assert_eq!(
                    color,
                    if better { theme.semantic.hp_ramp[0] } else { theme.semantic.hp_ramp[2] },
                    "{kind:?}"
                );
            }
            let value = list.find(E::StatValue(index)).expect("own").rect;
            let theirs_rect = list.find(E::StatOther(index)).expect("other").rect;
            let delta_rect = list.find(E::StatDelta(index)).expect("delta").rect;
            assert!(
                value.right() <= theirs_rect.x + 0.5 && theirs_rect.right() <= delta_rect.x + 0.5
            );
            assert!(plate.encloses(&delta_rect));
        }
        assert!(signed >= 5, "two different hulls differ somewhere");
        state.toggle_compare(1);
        assert_eq!(state.compare(), None, "the same cell again clears the compare");
        state.toggle_compare(1);
        state.select_index(1);
        assert_eq!(state.compare(), None, "selecting the compared hull clears it");
        state.toggle_compare(1);
        assert_eq!(state.compare(), None, "the turntable's own hull compares to nothing");
    }

    /// G9: a chip filters the carousel to what passes it and the arrows cycle only that; the
    /// chips print their words, the lamp on a set one; shift walks a ring back; a filter that
    /// nothing passes leaves the selection standing and the carousel empty, never a panic.
    #[test]
    fn a_filtered_carousel_cycles_only_what_passes_the_chips() {
        let (mut state, ui) = hangar();
        assert_eq!(
            text_of(&build_screen_list(&state, &ui, None), E::ChipValue(0)),
            words::CHIP_ALL
        );
        state.cycle_chip(Chip::Class, 1);
        let class = state.filter().class.expect("the first stop is a class");
        let roster = state.roster();
        assert!(
            roster.iter().all(|k| k.class() == class) && roster.len() < VehicleKind::PLAYABLE.len()
        );
        let list = build_screen_list(&state, &ui, None);
        for (i, kind) in roster.iter().enumerate() {
            assert_eq!(text_of(&list, E::CarouselName(i as u8)), kind.short_name());
        }
        assert!(
            list.find(E::CarouselCell(roster.len() as u8)).is_none(),
            "no cell past the roster"
        );
        assert_eq!(text_of(&list, E::ChipValue(0)), class.label().to_uppercase());
        let Payload::Text { color, .. } = list.find(E::ChipValue(0)).expect("value").payload else {
            panic!("text")
        };
        assert_eq!(color, Theme::standard().lamp, "a set chip reads in the lamp");
        let mut seen = Vec::new();
        for _ in 0..=roster.len() {
            state.cycle(1);
            seen.push(state.selected_vehicle());
        }
        assert!(seen.iter().all(|k| roster.contains(k)), "{seen:?} strays off the roster");
        assert_eq!(seen.first(), seen.last(), "the arrows wrap inside the roster");
        state.cycle(-1);
        assert!(roster.contains(&state.selected_vehicle()));
        // Shift walks the ring back to ALL.
        state.cycle_chip(Chip::Class, -1);
        assert_eq!(state.filter().class, None);
        assert_eq!(state.roster().len(), VehicleKind::PLAYABLE.len());
        // Nothing passes TD × Britain: the carousel empties, the selection stands.
        let selected = state.selected_vehicle();
        while state.filter().class != Some(game_core::VehicleClass::TankDestroyer) {
            state.cycle_chip(Chip::Class, 1);
        }
        while state.filter().nation != Some(game_core::Nation::Britain) {
            state.cycle_chip(Chip::Nation, 1);
        }
        assert!(state.roster().is_empty());
        state.cycle(1);
        assert_eq!(state.selected_vehicle(), selected);
        let list = build_screen_list(&state, &ui, None);
        assert!(list.find(E::CarouselCell(0)).is_none());
        assert!(list.find(E::CarouselPlate).is_some());
        assert_eq!(hit_screen(&state, &ui, false), GarageHit::Scene);
    }
}
