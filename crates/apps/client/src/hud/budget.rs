//! The visibility budget (interface program H14, absorbing Inny Poziom V3): „SEEN FROM 308 M ·
//! STILL" — the range the ENEMY'S longest-sighted hull spots us from, times the sim's own
//! factor for what we are doing: still (the stationary factor), moving, or having fired (the
//! reveal window). Both halves are the sim's constants and the roster's specs, never a guess;
//! when V1 and V2 land, the bush and the camouflage eat into the same number here.

use game_core::{TeamId, VehicleKind};
use net::RosterEntry;
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

use super::elements::HudElement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetState {
    Still,
    Moving,
    /// Fired within the reveal window, this many whole seconds ago.
    Fired {
        ago_s: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetModel {
    /// The range we are seen from, metres, rounded.
    pub seen_from_m: u32,
    pub state: BudgetState,
}

impl BudgetModel {
    /// The enemy's longest view range (the roster's specs) times the sim's own spotting factor
    /// for our state (`sim::spotting_range_factor`'s rule, on the client's own knowledge of
    /// its speed and its last shot).
    pub fn from_battle(
        roster: &[RosterEntry],
        player_team: TeamId,
        speed_mps: f32,
        fire_age_s: Option<f32>,
        tick_hz: f32,
    ) -> Option<Self> {
        let longest = roster
            .iter()
            .filter(|entry| entry.team != player_team)
            .map(|entry| entry.vehicle.spec_ref().view_range_m())
            .fold(None, |best: Option<f32>, range| Some(best.map_or(range, |b| b.max(range))))?;
        let reveal_s = sim::FIRE_REVEAL_TICKS as f32 / tick_hz.max(1.0);
        let state = match fire_age_s {
            Some(age) if age <= reveal_s => BudgetState::Fired { ago_s: age.floor() as u32 },
            _ if speed_mps > sim::STATIONARY_SPEED_MPS => BudgetState::Moving,
            _ => BudgetState::Still,
        };
        let factor = match state {
            BudgetState::Still => sim::STATIONARY_SPOT_FACTOR,
            BudgetState::Moving | BudgetState::Fired { .. } => 1.0,
        };
        Some(Self { seen_from_m: (longest * factor).round() as u32, state })
    }

    pub fn line(&self) -> String {
        use crate::ui_strings::battle as words;
        let state = match self.state {
            BudgetState::Still => words::BUDGET_STILL.to_string(),
            BudgetState::Moving => words::BUDGET_MOVING.to_string(),
            BudgetState::Fired { ago_s } => {
                format!("{} {ago_s} {}", words::BUDGET_FIRED, words::SECONDS_UNIT)
            }
        };
        format!("{} {} {} \u{b7} {state}", words::SEEN_FROM, self.seen_from_m, words::DISTANCE_UNIT)
    }
}

/// A staged roster of two enemy hulls for the tests and the demo.
pub(crate) fn staged_enemies(kinds: [VehicleKind; 2]) -> Vec<RosterEntry> {
    kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| RosterEntry {
            tank_id: game_core::TankId(20 + i as u64),
            team: TeamId(2),
            vehicle: *kind,
            seat: i as u8,
            crew_kind: net::CrewKind::Bot,
        })
        .collect()
}

const LINE_W_U: f32 = 400.0;
/// Under the sixth-sense lamp's slot, whether or not the lamp is lit.
pub(crate) const LINE_TOP_U: f32 = super::sixth_sense::LAMP_TOP_U + 26.0 + 4.0;

pub(crate) fn push_budget(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &BudgetModel,
    z: &mut i16,
) {
    // Hung from the top through the anchor (H21: the context's nudge moves it with the stack).
    let rect = ui.anchor(ui_kit::ui::Anchor::Top, [LINE_W_U, 18.0], [0.0, LINE_TOP_U]);
    list.push(
        Element::new(
            HudElement::BudgetLine,
            rect,
            Payload::Text {
                text: model.line(),
                style: Style::VALUE,
                size_u: 16.0,
                align: Align::Center,
                color: theme.text.unit,
                digits: DigitMode::Tabular,
            },
        )
        .z(*z),
    );
    *z += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H14: the number is the enemy's longest view range times the sim's own factor — still is
    /// the stationary factor, moving or a recent shot is the full range — and the line says
    /// which; no enemy on the roster, no line.
    #[test]
    fn the_budget_line_is_the_enemys_longest_view_range_times_the_sims_own_factor() {
        let roster = staged_enemies([VehicleKind::TigerII, VehicleKind::BENCHMARK]);
        let longest = VehicleKind::TigerII
            .spec_ref()
            .view_range_m()
            .max(VehicleKind::BENCHMARK.spec_ref().view_range_m());
        let still = BudgetModel::from_battle(&roster, TeamId(1), 0.0, None, 60.0).expect("line");
        assert_eq!(still.state, BudgetState::Still);
        assert_eq!(still.seen_from_m, (longest * sim::STATIONARY_SPOT_FACTOR).round() as u32);
        let moving = BudgetModel::from_battle(&roster, TeamId(1), 3.0, None, 60.0).expect("line");
        assert_eq!(moving.state, BudgetState::Moving);
        assert_eq!(moving.seen_from_m, longest.round() as u32);
        let fired =
            BudgetModel::from_battle(&roster, TeamId(1), 0.0, Some(3.4), 60.0).expect("line");
        assert_eq!(fired.state, BudgetState::Fired { ago_s: 3 });
        assert_eq!(fired.seen_from_m, longest.round() as u32);
        let long_ago =
            BudgetModel::from_battle(&roster, TeamId(1), 0.0, Some(30.0), 60.0).expect("line");
        assert_eq!(long_ago.state, BudgetState::Still, "the reveal window closed");
        assert_eq!(
            fired.line(),
            format!("SEEN FROM {} M \u{b7} FIRED 3 S", longest.round() as u32)
        );
        assert!(
            BudgetModel::from_battle(&roster, TeamId(2), 0.0, None, 60.0).is_none(),
            "no enemy, no line"
        );

        let mut list = DrawList::new();
        let mut z = 0;
        push_budget(&mut list, &Ui::reference(), &Theme::standard(), &still, &mut z);
        let line = list.find(HudElement::BudgetLine).expect("line").rect;
        assert!((line.center()[0] - 960.0).abs() < 1.0 && line.y > 80.0 && line.y < 120.0);
    }
}
