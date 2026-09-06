//! The kill feed (interface program H3): every kill on the field, off the wire's own kill
//! events (W-3) — a kill between two hulls this crew never saw still arrives, because the
//! event carries WHO and never WHERE. Newest first, under the enemy ear, each row the killer's
//! name, the word, the wreck's name; a kill without a killer (a drowning, a fall) is the wreck
//! and its cause. Names are the roster's: vehicle · seat.

use game_core::{DamageCause, KillEvent, TeamId};
use net::RosterEntry;
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{HudElement, KillFeedPart};
use crate::ui_strings::battle as words;

/// A row stays this long.
pub const ROW_TTL_S: f32 = 8.0;
/// The feed shows at most this many rows.
pub const MAX_ROWS: usize = 4;
/// Under the enemy ear (seven rows of the team list), against the right edge.
const FEED_TOP_U: f32 = super::team_list::EAR_TOP_U + 7.0 * 32.0 + 8.0;
const FEED_W_U: f32 = 380.0;
const ROW_H_U: f32 = 22.0;
const TEXT_U: f32 = 16.0;

/// One name on the feed: the roster's word for a hull and whose side it is on.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedName {
    pub text: String,
    pub enemy: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KillRow {
    /// The killer, when the event names one; a drowning or a fall names none.
    pub killer: Option<FeedName>,
    pub victim: FeedName,
    pub cause: DamageCause,
    pub age_s: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct KillFeedModel {
    /// Newest first.
    pub rows: Vec<KillRow>,
}

/// The cause's word when no killer is named.
fn cause_word(cause: DamageCause) -> &'static str {
    match cause {
        DamageCause::Shell => words::HIT_PEN,
        DamageCause::Ram => words::HIT_RAM,
        DamageCause::Impact => words::HIT_IMPACT,
        DamageCause::Splash => words::HIT_SPLASH,
        DamageCause::Drowning => words::HIT_DROWNED,
        DamageCause::Fire => words::FIRE_LAMP,
        DamageCause::AmmoRack => words::MODULE_AMMO_RACK,
    }
}

impl KillRow {
    /// The word between the names: DESTROYED after a killer, the cause after a wreck alone.
    pub fn word(&self) -> &'static str {
        if self.killer.is_some() { words::KILL_WORD } else { cause_word(self.cause) }
    }
}

impl KillFeedModel {
    /// The feed from the kills told so far: the newest `MAX_ROWS` younger than the TTL, named
    /// off the roster. A hull the roster does not know (never on a roster this crew got) is
    /// skipped rather than guessed.
    pub fn from_battle<'a>(
        kills: impl Iterator<Item = &'a KillEvent>,
        roster: &[RosterEntry],
        player_team: TeamId,
        now_tick: u64,
        tick_hz: f32,
    ) -> Self {
        let name = |id: game_core::TankId| {
            roster.iter().find(|entry| entry.tank_id == id).map(|entry| FeedName {
                text: format!("{} \u{b7} {}", entry.vehicle.short_name(), entry.seat_letter()),
                enemy: entry.team != player_team,
            })
        };
        let mut rows: Vec<KillRow> = kills
            .filter_map(|kill| {
                let age_s = now_tick.saturating_sub(kill.occurred_tick) as f32 / tick_hz.max(1.0);
                if age_s >= ROW_TTL_S {
                    return None;
                }
                Some(KillRow {
                    killer: kill.killer.and_then(name),
                    victim: name(kill.victim)?,
                    cause: kill.cause,
                    age_s,
                })
            })
            .collect();
        rows.reverse();
        rows.truncate(MAX_ROWS);
        Self { rows }
    }
}

fn row_faded(color: [f32; 4], age_s: f32) -> [f32; 4] {
    let fade = (1.0 - (age_s - ROW_TTL_S + 2.0).max(0.0) / 2.0).clamp(0.0, 1.0);
    [color[0], color[1], color[2], color[3] * fade]
}

pub(crate) fn push_kill_feed(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &KillFeedModel,
    z: &mut i16,
) {
    for (index, row) in model.rows.iter().enumerate().take(MAX_ROWS) {
        let i = index as u8;
        let line = ui.anchor(
            Anchor::TopRight,
            [FEED_W_U, ROW_H_U],
            [12.0, FEED_TOP_U + index as f32 * ROW_H_U],
        );
        // Right-aligned: the wreck's name sits at the edge, the word before it, the killer
        // before the word — each its own element so the names wear their team colours.
        let team_color = |enemy: bool| {
            row_faded(
                if enemy { theme.semantic.team_enemy } else { theme.semantic.team_ally },
                row.age_s,
            )
        };
        let word_w = ui.px(TEXT_U) * 0.62 * row.word().len() as f32 + ui.px(12.0);
        let name_w = |name: &FeedName| ui.px(TEXT_U) * 0.6 * name.text.len() as f32 + ui.px(6.0);
        let victim_w = name_w(&row.victim);
        let mut right = line.right();
        list.push(
            Element::new(
                HudElement::KillFeed(KillFeedPart::Victim(i)),
                Rect::new(right - victim_w, line.y, victim_w, line.h),
                Payload::Text {
                    text: row.victim.text.clone(),
                    style: Style::VALUE_STRONG,
                    size_u: TEXT_U,
                    align: Align::Right,
                    color: team_color(row.victim.enemy),
                    digits: DigitMode::Proportional,
                },
            )
            .z(*z),
        );
        *z += 1;
        right -= victim_w;
        list.push(
            Element::new(
                HudElement::KillFeed(KillFeedPart::Word(i)),
                Rect::new(right - word_w, line.y, word_w, line.h),
                Payload::Text {
                    text: row.word().to_string(),
                    style: Style::LABEL,
                    size_u: 14.0,
                    align: Align::Right,
                    color: row_faded(theme.text.label_dim, row.age_s),
                    digits: DigitMode::Proportional,
                },
            )
            .z(*z),
        );
        *z += 1;
        right -= word_w;
        if let Some(killer) = &row.killer {
            let killer_w = name_w(killer);
            list.push(
                Element::new(
                    HudElement::KillFeed(KillFeedPart::Killer(i)),
                    Rect::new(right - killer_w, line.y, killer_w, line.h),
                    Payload::Text {
                        text: killer.text.clone(),
                        style: Style::VALUE_STRONG,
                        size_u: TEXT_U,
                        align: Align::Right,
                        color: team_color(killer.enemy),
                        digits: DigitMode::Proportional,
                    },
                )
                .z(*z),
            );
            *z += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{TankId, VehicleKind};

    fn roster() -> Vec<RosterEntry> {
        let entry = |id: u64, team: u16, seat: u8| RosterEntry {
            tank_id: TankId(id),
            team: TeamId(team),
            vehicle: VehicleKind::BENCHMARK,
            seat,
            crew_kind: net::CrewKind::Human,
        };
        vec![entry(1, 1, 0), entry(2, 1, 1), entry(11, 2, 0), entry(12, 2, 1)]
    }

    /// H3: a kill between two enemy hulls this crew never saw (neither is in any snapshot it
    /// got) still makes a row — the wire's kill names WHO, never WHERE, and the feed has no
    /// position to invent.
    #[test]
    fn a_kill_between_two_unseen_hulls_still_reaches_the_feed_without_a_position() {
        let kills = [
            KillEvent {
                victim: TankId(12),
                killer: Some(TankId(11)),
                cause: DamageCause::Shell,
                occurred_tick: 100,
            },
            KillEvent {
                victim: TankId(2),
                killer: None,
                cause: DamageCause::Drowning,
                occurred_tick: 160,
            },
        ];
        let feed = KillFeedModel::from_battle(kills.iter(), &roster(), TeamId(1), 220, 60.0);
        assert_eq!(feed.rows.len(), 2);
        let newest = &feed.rows[0];
        assert!(newest.killer.is_none() && !newest.victim.enemy);
        assert_eq!(newest.word(), "DROWNED");
        let unseen = &feed.rows[1];
        assert_eq!(unseen.victim.text, format!("{} \u{b7} B", VehicleKind::BENCHMARK.short_name()));
        assert!(unseen.victim.enemy && unseen.killer.as_ref().is_some_and(|k| k.enemy));
        assert_eq!(unseen.word(), "DESTROYED");
        assert!((unseen.age_s - 2.0).abs() < 1e-6);
        // Aged out: gone. Unknown to the roster: skipped, never guessed.
        let stale =
            KillFeedModel::from_battle(kills.iter(), &roster(), TeamId(1), 100 + 8 * 60, 60.0);
        assert_eq!(stale.rows.len(), 1);
        let stranger = [KillEvent {
            victim: TankId(99),
            killer: None,
            cause: DamageCause::Fire,
            occurred_tick: 0,
        }];
        assert!(
            KillFeedModel::from_battle(stranger.iter(), &roster(), TeamId(1), 1, 60.0)
                .rows
                .is_empty()
        );
        // Drawn: three elements for a named kill, two for a wreck alone, under the enemy ear.
        let mut list = DrawList::new();
        let mut z = 0;
        push_kill_feed(&mut list, &Ui::reference(), &Theme::standard(), &feed, &mut z);
        assert!(list.find(HudElement::KillFeed(KillFeedPart::Killer(0))).is_none());
        let victim = list.find(HudElement::KillFeed(KillFeedPart::Victim(1))).expect("victim").rect;
        assert!(list.find(HudElement::KillFeed(KillFeedPart::Killer(1))).is_some());
        assert!(victim.right() <= 1920.0 - 11.0 && victim.y > 300.0, "{victim:?}");
    }
}
