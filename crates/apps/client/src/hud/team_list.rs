//! The team lists (interface program H2) — WoT's "ears": the player's team down the left edge,
//! the enemy down the right, one row per hull from the roster (W-1). A row carries the class
//! glyph, the vehicle's short name, the seat letter, a hit-point bar and its state — and
//! nothing about the player behind the hull (GDD §15.1: no other players' statistics).
//!
//! Honesty: an enemy the snapshot filter withholds is a row with a name and NO bar — the
//! roster names the field, it locates nobody, and a bar would be a guess. A bar appears when
//! the hull is in the snapshot (spotted, or a wreck); a dead hull is a dimmed row.

use game_core::{KillEvent, TankId, TeamId, VehicleKind};
use net::{CrewKind, RosterEntry, TankSnapshot};
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload, WidgetState};
use ui_kit::flow::Column;
use ui_kit::font::Style;
use ui_kit::icons::HudIcon;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::{Anchor, Ui};

use super::elements::{HudElement, TeamRowPart};
use super::top_bar::dead_hulls;

/// One hull's row. Every field is battle state a crew may know; there is no player in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRow {
    pub vehicle: VehicleKind,
    pub seat: char,
    pub human: bool,
    /// `(live, max)` when the snapshot shows the hull — an ally always, an enemy when spotted
    /// or wrecked; `None` for an enemy the filter withholds.
    pub hp: Option<(u32, u32)>,
    pub alive: bool,
    pub is_player: bool,
    /// Whether the hull is in the snapshot right now (an enemy: spotted or a wreck).
    pub spotted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TeamListsModel {
    pub allies: Vec<TeamRow>,
    pub enemies: Vec<TeamRow>,
}

impl TeamListsModel {
    pub fn from_battle<'a, I: Iterator<Item = &'a KillEvent>>(
        roster: &[RosterEntry],
        tanks: &[TankSnapshot],
        kills: I,
        player_tank: TankId,
        player_team: TeamId,
    ) -> Self {
        let dead = dead_hulls(tanks, kills);
        let mut model = Self::default();
        let mut entries: Vec<&RosterEntry> = roster.iter().collect();
        entries.sort_by_key(|entry| (entry.team != player_team, entry.seat));
        for entry in entries {
            let seen = tanks.iter().find(|tank| tank.tank_id == entry.tank_id);
            let max = entry.vehicle.spec_ref().hit_points;
            let row = TeamRow {
                vehicle: entry.vehicle,
                seat: entry.seat_letter(),
                human: entry.crew_kind == CrewKind::Human,
                hp: seen.map(|tank| (tank.hit_points, max)),
                alive: !dead.contains(&entry.tank_id)
                    && seen.is_none_or(|tank| tank.hit_points > 0),
                is_player: entry.tank_id == player_tank,
                spotted: seen.is_some(),
            };
            if entry.team == player_team {
                model.allies.push(row);
            } else {
                model.enemies.push(row);
            }
        }
        model
    }
}

const EAR_W_U: f32 = 190.0;
const ROW_H_U: f32 = 28.0;
const ROW_GAP_U: f32 = 4.0;
const EAR_INSET_U: f32 = 12.0;
/// Below the legacy module and crew panels that still sit at the top-left (H4 moves them to
/// the bottom-left, where the damage panel belongs; the ears then rise to 96 u).
const EAR_TOP_U: f32 = 232.0;
const TEXT_U: f32 = 16.0;

pub(crate) fn push_team_lists(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    model: &TeamListsModel,
    z: &mut i16,
) {
    let rows = model.allies.len().max(model.enemies.len()) as f32;
    let ear_h = rows * ROW_H_U + (rows - 1.0).max(0.0) * ROW_GAP_U;
    for (enemy, rows) in [(false, &model.allies), (true, &model.enemies)] {
        let anchor = if enemy { Anchor::TopRight } else { Anchor::TopLeft };
        let frame = ui.anchor(anchor, [EAR_W_U, ear_h], [EAR_INSET_U, EAR_TOP_U]);
        let mut column = Column::new(frame, 0.0, ui.px(ROW_GAP_U));
        for (index, row) in rows.iter().enumerate() {
            let rect = column.next(ui.px(ROW_H_U));
            push_row(list, ui, theme, row, enemy, index as u8, rect, z);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_row(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    row: &TeamRow,
    enemy: bool,
    index: u8,
    rect: Rect,
    z: &mut i16,
) {
    let key = |part: TeamRowPart| HudElement::TeamRow { enemy, index, part };
    let mut push = |mut element: Element<HudElement>| {
        if !row.alive {
            element.state = WidgetState::Disabled;
        }
        list.push(element.z(*z));
        *z += 1;
    };
    let team = if enemy { theme.semantic.team_enemy } else { theme.semantic.team_ally };
    let enamel = theme.plates.enamel_black;
    push(Element::new(
        key(TeamRowPart::Strip),
        rect,
        Payload::Plate {
            tile: enamel.tile,
            radius_u: 2.0,
            bevel_u: 0.0,
            color: [enamel.color[0], enamel.color[1], enamel.color[2], 0.62],
        },
    ));
    if row.is_player {
        push(Element::new(
            key(TeamRowPart::Lamp),
            Rect::new(rect.x, rect.y, ui.px(3.0), rect.h),
            Payload::Plate { tile: enamel.tile, radius_u: 1.0, bevel_u: 0.0, color: theme.lamp },
        ));
    }
    let known = row.alive && (!enemy || row.spotted);
    let glyph_color = if known { team } else { theme.text.label_dim };
    let glyph = ui.px(20.0);
    push(Element::new(
        key(TeamRowPart::Class),
        Rect::new(rect.x + ui.px(7.0), rect.y + (rect.h - glyph) * 0.5, glyph, glyph),
        Payload::Icon { icon: HudIcon::for_class(row.vehicle.class()), color: glyph_color },
    ));
    let text_h = ui.px(TEXT_U);
    let name_x = rect.x + ui.px(34.0);
    push(Element::new(
        key(TeamRowPart::Name),
        Rect::new(name_x, rect.y + ui.px(3.0), ui.px(112.0), text_h),
        Payload::Text {
            text: row.vehicle.short_name().to_string(),
            style: Style::VALUE,
            size_u: TEXT_U,
            align: Align::Left,
            color: if row.alive { theme.text.label } else { theme.text.label_dim },
            digits: DigitMode::Proportional,
        },
    ));
    let seat_color = if row.is_player {
        theme.semantic.team_self
    } else if row.human {
        theme.text.label
    } else {
        theme.text.label_dim
    };
    push(Element::new(
        key(TeamRowPart::Seat),
        Rect::new(rect.right() - ui.px(34.0), rect.y + ui.px(3.0), ui.px(28.0), text_h),
        Payload::Text {
            text: row.seat.to_string(),
            style: Style::VALUE_STRONG,
            size_u: TEXT_U,
            align: Align::Right,
            color: seat_color,
            digits: DigitMode::Proportional,
        },
    ));
    if let Some((live, max)) = row.hp
        && row.alive
    {
        push(Element::new(
            key(TeamRowPart::Health),
            Rect::new(
                name_x,
                rect.bottom() - ui.px(6.0),
                rect.right() - ui.px(6.0) - name_x,
                ui.px(3.0),
            ),
            Payload::Bar {
                frac: live as f32 / max.max(1) as f32,
                fill: team,
                back: [0.0, 0.0, 0.0, 0.35],
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::DamageCause;

    fn entry(
        id: u64,
        team: u16,
        seat: u8,
        vehicle: VehicleKind,
        crew_kind: CrewKind,
    ) -> RosterEntry {
        RosterEntry { tank_id: TankId(id), team: TeamId(team), vehicle, seat, crew_kind }
    }

    fn tank(id: u64, team: u16, hp: u32) -> TankSnapshot {
        crate::hud::tests::tank_snapshot(id, team, hp)
    }

    fn battle() -> (Vec<RosterEntry>, Vec<TankSnapshot>, Vec<KillEvent>) {
        let roster = vec![
            entry(1, 1, 0, VehicleKind::T54_1951, CrewKind::Human),
            entry(2, 1, 1, VehicleKind::IS3, CrewKind::Bot),
            entry(3, 2, 0, VehicleKind::TigerII, CrewKind::Bot),
            entry(4, 2, 1, VehicleKind::PantherII, CrewKind::Bot),
            entry(5, 2, 2, VehicleKind::Jagdtiger, CrewKind::Bot),
        ];
        // Allies always in the snapshot; enemy 3 spotted, enemy 4 withheld, enemy 5 told dead.
        let tanks = vec![tank(1, 1, 900), tank(2, 1, 1_200), tank(3, 2, 800)];
        let kills = vec![KillEvent {
            victim: TankId(5),
            killer: None,
            cause: DamageCause::Fire,
            occurred_tick: 3,
        }];
        (roster, tanks, kills)
    }

    fn rows_of(list: &DrawList<HudElement>, enemy: bool, index: u8) -> Vec<&Element<HudElement>> {
        list.iter()
            .filter(|e| matches!(e.id, HudElement::TeamRow { enemy: en, index: ix, .. } if en == enemy && ix == index))
            .collect()
    }

    /// H2: a row is vehicle, seat, hit points and state — the destructuring names every field,
    /// so a field about the PLAYER (a name, a rating, a win rate) cannot be added unnoticed.
    #[test]
    fn a_team_list_row_carries_vehicle_seat_hp_and_state_and_nothing_about_the_player() {
        let (roster, tanks, kills) = battle();
        let model =
            TeamListsModel::from_battle(&roster, &tanks, kills.iter(), TankId(1), TeamId(1));
        assert_eq!(model.allies.len(), 2);
        assert_eq!(model.enemies.len(), 3);
        let TeamRow { vehicle, seat, human, hp, alive, is_player, spotted } =
            model.allies[0].clone();
        assert_eq!(
            (vehicle, seat, human, hp, alive, is_player, spotted),
            (
                VehicleKind::T54_1951,
                'A',
                true,
                Some((900, VehicleKind::T54_1951.spec_ref().hit_points)),
                true,
                true,
                true
            )
        );
        assert_eq!(model.allies[1].seat, 'B');
        assert!(!model.allies[1].human && !model.allies[1].is_player);

        let ui = Ui::reference();
        let theme = Theme::standard();
        let mut list = DrawList::new();
        let mut z = 0;
        push_team_lists(&mut list, &ui, &theme, &model, &mut z);
        let parts: Vec<TeamRowPart> = rows_of(&list, false, 0)
            .iter()
            .map(|e| match e.id {
                HudElement::TeamRow { part, .. } => part,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            parts,
            vec![
                TeamRowPart::Strip,
                TeamRowPart::Lamp,
                TeamRowPart::Class,
                TeamRowPart::Name,
                TeamRowPart::Seat,
                TeamRowPart::Health
            ],
            "the player's row: the strip, the lamp, the class, the name, the seat, the bar"
        );
        assert!(
            rows_of(&list, false, 1)
                .iter()
                .all(|e| !matches!(e.id, HudElement::TeamRow { part: TeamRowPart::Lamp, .. })),
            "only the player wears the lamp"
        );
        // The ears hang from the two top corners, inside the viewport, allies left.
        let ally = rows_of(&list, false, 0)[0].rect;
        let enemy = rows_of(&list, true, 0)[0].rect;
        assert!(ally.x < 960.0 && enemy.x > 960.0 && ally.y == enemy.y);
        assert!(ui.viewport().encloses(&ally) && ui.viewport().encloses(&enemy));
    }

    /// H2: an enemy the filter withholds is a name and a seat — no bar, and the row type has
    /// no position to leak.
    #[test]
    fn an_unseen_enemy_row_has_no_bar_and_no_position() {
        let (roster, tanks, kills) = battle();
        let model =
            TeamListsModel::from_battle(&roster, &tanks, kills.iter(), TankId(1), TeamId(1));
        let spotted = &model.enemies[0];
        let unseen = &model.enemies[1];
        let told_dead = &model.enemies[2];
        assert!(spotted.spotted && spotted.hp.is_some() && spotted.alive);
        assert!(
            !unseen.spotted && unseen.hp.is_none() && unseen.alive,
            "withheld: named, unlocated, no bar"
        );
        assert!(!told_dead.alive && !told_dead.spotted, "a kill told is a dead row even unseen");

        let ui = Ui::reference();
        let theme = Theme::standard();
        let mut list = DrawList::new();
        let mut z = 0;
        push_team_lists(&mut list, &ui, &theme, &model, &mut z);
        let has_bar = |index: u8| {
            rows_of(&list, true, index)
                .iter()
                .any(|e| matches!(e.id, HudElement::TeamRow { part: TeamRowPart::Health, .. }))
        };
        assert!(has_bar(0), "the spotted enemy wears a bar");
        assert!(!has_bar(1), "the withheld enemy wears none");
        assert!(!has_bar(2), "nor does the dead one");
        assert!(
            rows_of(&list, true, 2).iter().all(|e| e.state == WidgetState::Disabled),
            "the dead row is dimmed"
        );
        // No world coordinate anywhere in the ear: every rect is an ear slot, none a map point.
        let expected_x = ui.anchor(Anchor::TopRight, [EAR_W_U, 1.0], [EAR_INSET_U, 0.0]).x;
        assert!(rows_of(&list, true, 1).iter().all(|e| e.rect.x >= expected_x - 1.0));
    }
}
