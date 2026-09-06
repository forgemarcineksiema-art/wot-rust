//! The top bar (interface program H1): the battle clock under glass in the middle, and on
//! either side the frags a team has taken and the pool of hit points it has left — the
//! player's team on the left, the enemy on the right, in the team colours.
//!
//! Honesty: the frag counter counts the kills the wire told this crew (W-3, every kill on the
//! field) and the wrecks the snapshot shows; the pools are the server's sums (W-2), never a
//! client-side guess from the hulls it happens to see.

use game_core::{KillEvent, TankId, TeamId};
use net::{RosterEntry, TankSnapshot};
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::HudElement;

/// Index 0 is the player's team, 1 the enemy — the sides of the bar, not the wire's team ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopBarModel {
    /// Hulls each side has destroyed.
    pub frags: [u8; 2],
    /// Hit points each side has left, the server's sum.
    pub team_hit_points: [u32; 2],
    /// The pools at full strength: the sum of every roster hull's stock hit points.
    pub team_hit_points_max: [u32; 2],
}

/// The wire's index of a team's pool: `TeamId(1)` first.
fn pool_index(team: TeamId) -> Option<usize> {
    match team {
        TeamId(1) => Some(0),
        TeamId(2) => Some(1),
        _ => None,
    }
}

/// The hulls known to be dead: every kill the wire told (W-3) and every wreck the snapshot
/// shows — the union of what this crew was told and what it can see.
pub(crate) fn dead_hulls<'a, I: Iterator<Item = &'a KillEvent>>(
    tanks: &[TankSnapshot],
    kills: I,
) -> Vec<TankId> {
    let mut dead: Vec<TankId> = kills.map(|kill| kill.victim).collect();
    dead.extend(tanks.iter().filter(|tank| tank.hit_points == 0).map(|tank| tank.tank_id));
    dead.sort_unstable_by_key(|id| id.0);
    dead.dedup();
    dead
}

impl TopBarModel {
    pub fn from_battle<'a, I: Iterator<Item = &'a KillEvent>>(
        roster: &[RosterEntry],
        tanks: &[TankSnapshot],
        team_hit_points: [u32; 2],
        kills: I,
        player_team: TeamId,
    ) -> Self {
        let side = |team: TeamId| usize::from(team != player_team);
        let dead = dead_hulls(tanks, kills);
        let mut frags = [0u8; 2];
        let mut max = [0u32; 2];
        let mut pools = [0u32; 2];
        for entry in roster {
            let s = side(entry.team);
            max[s] = max[s].saturating_add(entry.vehicle.spec_ref().hit_points);
            if dead.contains(&entry.tank_id) {
                // A dead hull on side `s` is a frag for the OTHER side.
                frags[1 - s] = frags[1 - s].saturating_add(1);
            }
            if let Some(index) = pool_index(entry.team) {
                pools[s] = team_hit_points[index];
            }
        }
        Self { frags, team_hit_points: pools, team_hit_points_max: max }
    }
}

/// The plate's size in `u`, and the clock's slot in the middle of it.
const BAR_SIZE_U: [f32; 2] = [560.0, 48.0];
const BAR_TOP_U: f32 = 8.0;
const CLOCK_SLOT_W_U: f32 = 132.0;
/// The last minute warms the clock to the lamp: the closing squeeze, in the one glow the
/// theme has — and at three to one on the glass, where the amber read 2.96 (H23).
const CLOSING_S: u32 = 60;

pub(crate) fn push_top_bar(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &TopBarModel,
    clock_remaining_s: Option<f32>,
    z: &mut i16,
) {
    let mut push = |element: Element<HudElement>| {
        list.push(element.z(*z));
        *z += 1;
    };
    let plate = ui.anchor(Anchor::Top, BAR_SIZE_U, [0.0, BAR_TOP_U]);
    let steel = theme.plates.steel_painted;
    push(Element::new(
        HudElement::TopBar,
        plate,
        Payload::Plate {
            tile: steel.tile,
            radius_u: 4.0,
            bevel_u: theme.bevel_u,
            color: steel.color,
        },
    ));

    let clock_w = ui.px(CLOCK_SLOT_W_U);
    let clock = Rect::new(
        plate.center()[0] - clock_w * 0.5,
        plate.y + ui.px(4.0),
        clock_w,
        plate.h - ui.px(8.0),
    );
    let enamel = theme.plates.enamel_black;
    if let Some(remaining_s) = clock_remaining_s {
        let total = remaining_s.max(0.0).ceil() as u32;
        let color = if total <= CLOSING_S { theme.lamp } else { theme.text.value };
        push(Element::new(
            HudElement::TopBarClock,
            clock,
            Payload::Text {
                text: format!("{}:{:02}", total / 60, total % 60),
                style: Style::VALUE_STRONG,
                size_u: 30.0,
                align: Align::Center,
                color,
                digits: DigitMode::Tabular,
            },
        ));
        push(Element::new(
            HudElement::TopBarClockGlass,
            clock,
            Payload::Glass { radius_u: 3.0, phase: 0.35, color: theme.plates.glass.color },
        ));
    }

    let sides = [
        (HudElement::TopBarAllyFrags, HudElement::TopBarAllyPool, theme.semantic.team_ally, false),
        (
            HudElement::TopBarEnemyFrags,
            HudElement::TopBarEnemyPool,
            theme.semantic.team_enemy,
            true,
        ),
    ];
    for (s, (frags_id, pool_id, color, mirrored)) in sides.into_iter().enumerate() {
        let inner = ui.px(18.0);
        let number_w = ui.px(64.0);
        let (number, bar) = if mirrored {
            let number = Rect::new(plate.right() - inner - number_w, plate.y, number_w, plate.h);
            let bar = Rect::new(
                clock.right() + ui.px(14.0),
                plate.y + ui.px(20.0),
                number.x - ui.px(12.0) - clock.right() - ui.px(14.0),
                ui.px(8.0),
            );
            (number, bar)
        } else {
            let number = Rect::new(plate.x + inner, plate.y, number_w, plate.h);
            let bar = Rect::new(
                number.right() + ui.px(12.0),
                plate.y + ui.px(20.0),
                clock.x - ui.px(14.0) - number.right() - ui.px(12.0),
                ui.px(8.0),
            );
            (number, bar)
        };
        push(Element::new(
            frags_id,
            number,
            Payload::Text {
                text: model.frags[s].to_string(),
                style: Style::VALUE_BOLD,
                size_u: 26.0,
                align: if mirrored { Align::Right } else { Align::Left },
                color: theme.text.value,
                digits: DigitMode::Tabular,
            },
        ));
        let frac = model.team_hit_points[s] as f32 / model.team_hit_points_max[s].max(1) as f32;
        push(Element::new(
            pool_id,
            bar,
            Payload::Bar {
                frac: frac.clamp(0.0, 1.0),
                fill: color,
                back: [enamel.color[0], enamel.color[1], enamel.color[2], 0.85],
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{DamageCause, VehicleKind};
    use net::CrewKind;

    fn entry(id: u64, team: u16, vehicle: VehicleKind) -> RosterEntry {
        RosterEntry {
            tank_id: TankId(id),
            team: TeamId(team),
            vehicle,
            seat: 0,
            crew_kind: CrewKind::Bot,
        }
    }

    fn tank(id: u64, team: u16, hp: u32) -> TankSnapshot {
        crate::hud::tests::tank_snapshot(id, team, hp)
    }

    /// H1: the frags are the kills told plus the wrecks seen (a wreck told AND seen counts
    /// once); the pools are the server's sums, verbatim, mapped to the player's side first.
    #[test]
    fn the_top_bar_counts_wrecks_it_can_see_and_sums_hp_it_was_told() {
        let roster = vec![
            entry(1, 1, VehicleKind::T54_1951),
            entry(2, 1, VehicleKind::IS3),
            entry(3, 2, VehicleKind::TigerII),
            entry(4, 2, VehicleKind::PantherII),
            entry(5, 2, VehicleKind::Jagdtiger),
        ];
        // The snapshot: one enemy wreck visible, the rest filtered away; one ally alive.
        let tanks = vec![tank(1, 1, 900), tank(3, 2, 0)];
        // The wire told of two kills: the wreck we see (once more) and an enemy we never saw.
        let kills = [
            KillEvent {
                victim: TankId(3),
                killer: Some(TankId(1)),
                cause: DamageCause::Shell,
                occurred_tick: 5,
            },
            KillEvent {
                victim: TankId(5),
                killer: None,
                cause: DamageCause::Drowning,
                occurred_tick: 9,
            },
        ];
        let model =
            TopBarModel::from_battle(&roster, &tanks, [1_500, 700], kills.iter(), TeamId(1));
        assert_eq!(model.frags, [2, 0], "two enemy hulls are down, no ally");
        assert_eq!(model.team_hit_points, [1_500, 700], "the pools are the server's, verbatim");
        let max_ally =
            VehicleKind::T54_1951.spec_ref().hit_points + VehicleKind::IS3.spec_ref().hit_points;
        assert_eq!(model.team_hit_points_max[0], max_ally);

        // Seen from the other team the sides swap and the frags are theirs.
        let flipped =
            TopBarModel::from_battle(&roster, &tanks, [1_500, 700], kills.iter(), TeamId(2));
        assert_eq!(flipped.frags, [0, 2]);
        assert_eq!(flipped.team_hit_points, [700, 1_500]);
    }

    /// The clock is an element only when the battle is timed, and warms in the last minute.
    #[test]
    fn the_clock_sits_under_glass_only_when_timed_and_warms_in_the_last_minute() {
        let ui = Ui::reference();
        let theme = Theme::standard();
        let model = TopBarModel {
            frags: [1, 2],
            team_hit_points: [3_000, 2_000],
            team_hit_points_max: [7_000, 7_000],
        };
        let build = |clock: Option<f32>| {
            let mut list = DrawList::new();
            let mut z = 0;
            push_top_bar(&mut list, &ui, &theme, &model, clock, &mut z);
            list
        };
        assert!(build(None).find(HudElement::TopBarClock).is_none(), "untimed: no clock");
        let timed = build(Some(474.0));
        match &timed.find(HudElement::TopBarClock).expect("clock").payload {
            Payload::Text { text, color, .. } => {
                assert_eq!(text, "7:54");
                assert_eq!(*color, theme.text.value);
            }
            other => panic!("the clock is text, found {other:?}"),
        }
        assert!(timed.find(HudElement::TopBarClockGlass).is_some(), "the clock sits under glass");
        match &build(Some(42.0)).find(HudElement::TopBarClock).expect("clock").payload {
            Payload::Text { color, .. } => {
                assert_eq!(*color, theme.lamp, "the last minute warms")
            }
            other => panic!("{other:?}"),
        }
        // The bar and its parts hang from the top edge, centred, inside the viewport.
        let plate = timed.find(HudElement::TopBar).expect("plate").rect;
        assert!((plate.center()[0] - 960.0).abs() < 0.5 && plate.y == 8.0);
        assert!(ui.viewport().encloses(&plate));
    }
}
