//! The garage's key legend and its tooltips (interface program G6). Every key the legend and a
//! tooltip print comes from the binding table by way of [`KeyLabels`] — the app refreshes them
//! from the table every garage frame — never from a literal in the copy: rebind REPAIR to H and
//! the hangar says H.

use crate::app::keybinds::{Action, KeyBindings};
use crate::app::shell::first_key_label;
use crate::ui_strings::garage as words;
use game_core::VehicleKind;

use super::GarageState;
use super::draft::FitSlot;
use super::elements::GarageElement as E;
use super::filter::Chip;
use super::layout::{carousel_window, slot_label};

/// The printed key of every garage action, as the table binds it right now.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) struct KeyLabels {
    pub prev: String,
    pub next: String,
    pub confirm: String,
    pub back: String,
    pub focus_prev: String,
    pub focus_next: String,
    pub cycle_prev: String,
    pub cycle_next: String,
    pub ammo: [String; 3],
    pub map: String,
    pub daylight: String,
    pub inspector: String,
    pub repair: String,
    pub tree: String,
}

impl KeyLabels {
    /// Read off the table: the first key of every garage action, printed the way the
    /// keybinds page prints it; an unbound action prints `-`.
    pub(in crate::app) fn from_table(keybinds: &KeyBindings) -> Self {
        let first = |action: Action| first_key_label(keybinds, action);
        Self {
            prev: first(Action::GaragePrev),
            next: first(Action::GarageNext),
            confirm: first(Action::GarageConfirm),
            back: first(Action::GarageBack),
            focus_prev: first(Action::FocusPrev),
            focus_next: first(Action::FocusNext),
            cycle_prev: first(Action::CycleFocusedPrev),
            cycle_next: first(Action::CycleFocusedNext),
            ammo: [
                first(Action::GarageAmmo1),
                first(Action::GarageAmmo2),
                first(Action::GarageAmmo3),
            ],
            map: first(Action::GarageMap),
            daylight: first(Action::Daylight),
            inspector: first(Action::Inspector),
            repair: first(Action::Repair),
            tree: first(Action::TechTree),
        }
    }
}

impl Default for KeyLabels {
    fn default() -> Self {
        Self::from_table(&KeyBindings::default())
    }
}

const DOT: &str = " \u{b7} ";

/// The legend's three lines on the top bar: the keys of the hangar, from the table.
pub(super) fn hint_lines(k: &KeyLabels) -> [String; 3] {
    [
        format!(
            "{} {} {}{DOT}{} {} {}{DOT}{} {} {}",
            k.prev,
            k.next,
            words::HINT_SELECT,
            k.focus_prev,
            k.focus_next,
            words::HINT_FOCUS,
            k.cycle_prev,
            k.cycle_next,
            words::HINT_CYCLE
        ),
        format!(
            "{} {} {} {}{DOT}{} {}{DOT}{} {}{DOT}{} {}",
            k.ammo[0],
            k.ammo[1],
            k.ammo[2],
            words::HINT_AMMO,
            k.map,
            words::HINT_MAP,
            k.inspector,
            words::HINT_ARMOUR,
            k.daylight,
            words::HINT_LIGHT
        ),
        format!(
            "{} {}{DOT}{} {}{DOT}{} {}{DOT}{} {}",
            k.repair,
            words::HINT_REPAIR,
            k.tree,
            words::HINT_TREE,
            k.confirm,
            words::BATTLE,
            k.back,
            words::BACK
        ),
    ]
}

/// What a control's tooltip says: the control's job and the key that also does it. `None` for
/// the surfaces that are not controls.
pub(super) fn tooltip_text(state: &GarageState, element: E) -> Option<String> {
    let k = state.key_labels();
    Some(match element {
        E::BattleButton | E::BattleLabel => {
            if state.is_locked() {
                words::TIP_LOCKED.to_string()
            } else {
                format!("{}{DOT}{}", words::TIP_DEPLOY, k.confirm)
            }
        }
        E::MapRow | E::MapLabel => {
            format!("{}{DOT}{}{DOT}{}", words::TIP_MAP, k.map, words::TIP_SHIFT_BACK)
        }
        E::TabTechTree => format!("{}{DOT}{}", words::TAB_TECH_TREE, k.tree),
        E::TabArmour => format!("{}{DOT}{}", words::TAB_ARMOUR, k.inspector),
        E::TabGarage => words::TAB_GARAGE.to_string(),
        E::TabBattles => words::TAB_BATTLES.to_string(),
        E::TabReplays => words::TAB_REPLAYS.to_string(),
        E::TabStatistics => words::TAB_STATISTICS.to_string(),
        E::TabSettings => words::TAB_SETTINGS.to_string(),
        E::TreeNode(i) => VehicleKind::PLAYABLE.get(usize::from(i))?.display_name().to_string(),
        E::TreeBack => words::BACK.to_string(),
        E::ModuleSlot(i) => format!(
            "{}{DOT}{}{DOT}{} {} {}",
            slot_label(FitSlot::ALL[usize::from(i)]),
            words::TIP_CHOOSE,
            k.cycle_prev,
            k.cycle_next,
            words::HINT_CYCLE
        ),
        E::AmmoSlot(i) => format!(
            "{}{DOT}{}",
            words::TIP_LOAD,
            k.ammo.get(usize::from(i)).map_or("-", String::as_str)
        ),
        E::AmmoMinus(_) | E::AmmoPlus(_) => words::TIP_COUNT.to_string(),
        E::CarouselCell(i) => {
            let roster = state.roster();
            let window = carousel_window(roster.len(), state.carousel_scroll());
            let kind = roster.get(window.start + usize::from(i))?;
            format!("{}{DOT}{}", kind.display_name(), words::TIP_COMPARE)
        }
        E::CarouselArrow(_) => words::TIP_SCROLL.to_string(),
        E::OptionRow(_) => words::TIP_INSTALL.to_string(),
        E::Chip(i) => format!(
            "{} {}{DOT}{}",
            words::TIP_FILTER,
            Chip::ALL[usize::from(i)].word(),
            words::TIP_SHIFT_BACK
        ),
        _ => return None,
    })
}
