//! The hit log (interface program H8): every hit the player dealt or took, one row each, under
//! the reticle where the eye already is — the outcome word, the damage, the round, the shell's
//! penetration against the plate's EFFECTIVE armour at the angle it struck, the zone, and the
//! other hull. Everything the wire carries, printed; nothing the wire withholds, guessed. Bounces
//! earn a row (a held shell is a fact worth reading), a taken hit prints its range (W-6), the
//! ticks of a fire coalesce into one row, and N collapses the log to its last row.

use std::collections::VecDeque;

use game_core::{ArmorZone, DamageCause, DamageEvent, ModuleSlot, TankId, TrackSide, VehicleKind};
use net::TankSnapshot;
use ui_kit::draw_list::{Align, DigitMode, DrawList, Element, Payload};
use ui_kit::font::Style;
use ui_kit::rect::Rect;
use ui_kit::theme::Theme;
use ui_kit::ui::Ui;

use super::elements::{HitLogPart, HudElement};

const LOG_CAP: usize = 6;
const LOG_TTL_S: f32 = 8.0;
/// The last second of a row's life fades it.
const FADE_S: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogDirection {
    Dealt,
    Taken,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DamageLogEntry {
    pub direction: LogDirection,
    pub damage_hp: u32,
    pub module: Option<ModuleSlot>,
    pub track: Option<(TrackSide, bool)>,
    pub other_vehicle: Option<VehicleKind>,
    pub crew_hits_mask: u8,
    pub round: Option<game_core::RoundId>,
    pub age_s: f32,
    pub cause: DamageCause,
    pub penetrated: bool,
    pub ricocheted: bool,
    pub shattered: bool,
    /// The shell's penetration at the range it struck, millimetres.
    pub shell_penetration_mm: u32,
    /// The plate's effective armour along the shell's line, millimetres.
    pub effective_armor_mm: u32,
    pub impact_angle_degrees: u32,
    pub zone: ArmorZone,
    /// The range the shell flew (W-6); `None` before the wire carried it or without a shell.
    pub distance_m: Option<u32>,
}

impl DamageLogEntry {
    /// The outcome family the row belongs to: 0 penetration, 1 held, 2 ricochet, 3 shatter.
    pub fn family(&self) -> usize {
        if self.cause != DamageCause::Shell || self.penetrated {
            0
        } else if self.shattered {
            3
        } else if self.ricocheted {
            2
        } else {
            1
        }
    }

    /// The outcome word.
    pub fn word(&self) -> &'static str {
        use crate::ui_strings::battle as words;
        match self.cause {
            DamageCause::Shell => {
                if self.shattered {
                    words::HIT_SHATTER
                } else if self.ricocheted {
                    words::HIT_RICOCHET
                } else if self.penetrated {
                    words::HIT_PEN
                } else if self.track.is_some() {
                    words::HIT_TRACKED
                } else {
                    words::HIT_NO_PEN
                }
            }
            DamageCause::Ram => words::HIT_RAM,
            DamageCause::Impact => words::HIT_IMPACT,
            DamageCause::Splash => words::HIT_SPLASH,
            DamageCause::Drowning => words::HIT_DROWNED,
            DamageCause::Fire => words::FIRE_LAMP,
            _ => words::RACK_FUZE,
        }
    }

    /// The row's text, one line, right-aligned under the reticle.
    pub fn line(&self) -> String {
        use crate::ui_strings::battle as words;
        let mut parts: Vec<String> = Vec::new();
        let arrow = match self.direction {
            LogDirection::Dealt => "\u{bb}",
            LogDirection::Taken => "\u{ab}",
        };
        parts.push(if self.damage_hp > 0 {
            format!("{arrow} {} {}", self.word(), self.damage_hp.min(9_999))
        } else {
            format!("{arrow} {}", self.word())
        });
        if let Some(round) = self.round {
            parts.push(round.designation().to_string());
        }
        if self.cause == DamageCause::Shell {
            parts.push(format!(
                "{} > {} {} @ {}\u{b0}",
                self.shell_penetration_mm,
                self.effective_armor_mm,
                words::MILLIMETRES,
                self.impact_angle_degrees
            ));
            parts.push(zone_name(self.zone).to_string());
        }
        if let Some(distance) = self.distance_m
            && self.direction == LogDirection::Taken
        {
            parts.push(format!("{distance} {}", words::DISTANCE_UNIT));
        }
        if let Some(module) = self.module {
            parts.push(module_name(module).to_string());
        }
        // The track side, unless the zone already named it.
        if let Some((side, _)) = self.track
            && !matches!(self.zone, ArmorZone::LeftTrack | ArmorZone::RightTrack)
        {
            parts.push(
                match side {
                    TrackSide::Left => words::ZONE_LEFT_TRACK,
                    TrackSide::Right => words::ZONE_RIGHT_TRACK,
                }
                .to_string(),
            );
        }
        let crew: String = game_core::CrewRole::ALL
            .iter()
            .filter(|role| self.crew_hits_mask & role.mask_bit() != 0)
            .map(|role| super::damage_panel::role_letter(*role))
            .collect::<Vec<_>>()
            .join(" ");
        if !crew.is_empty() {
            parts.push(crew);
        }
        if let Some(kind) = self.other_vehicle {
            parts.push(kind.short_name().to_string());
        }
        parts.join(" \u{b7} ")
    }
}

pub(crate) fn zone_name(zone: ArmorZone) -> &'static str {
    use crate::ui_strings::battle as words;
    match zone {
        ArmorZone::UpperGlacis => words::ZONE_UPPER_GLACIS,
        ArmorZone::LowerPlate => words::ZONE_LOWER_PLATE,
        ArmorZone::HullSide => words::ZONE_HULL_SIDE,
        ArmorZone::HullRear => words::ZONE_HULL_REAR,
        ArmorZone::TurretFront => words::ZONE_TURRET_FRONT,
        ArmorZone::Mantlet => words::ZONE_MANTLET,
        ArmorZone::TurretSide => words::ZONE_TURRET_SIDE,
        ArmorZone::TurretRear => words::ZONE_TURRET_REAR,
        ArmorZone::Roof => words::ZONE_ROOF,
        ArmorZone::LeftTrack => words::ZONE_LEFT_TRACK,
        ArmorZone::RightTrack => words::ZONE_RIGHT_TRACK,
        ArmorZone::Skirt => words::ZONE_SKIRT,
        ArmorZone::HullDeck => words::ZONE_HULL_DECK,
        ArmorZone::Cupola => words::ZONE_CUPOLA,
        ArmorZone::GlacisPort => words::ZONE_GLACIS_PORT,
    }
}

pub(crate) fn module_name(module: ModuleSlot) -> &'static str {
    use crate::ui_strings::battle as words;
    match module {
        ModuleSlot::Engine => words::MODULE_ENGINE,
        ModuleSlot::Suspension => words::MODULE_SUSPENSION,
        ModuleSlot::Turret => words::MODULE_TURRET,
        ModuleSlot::Gun => words::MODULE_GUN,
        ModuleSlot::AmmoRack => words::MODULE_AMMO_RACK,
        ModuleSlot::Radio => words::MODULE_RADIO,
    }
}

#[derive(Debug, Default)]
pub(crate) struct DamageLog {
    entries: VecDeque<DamageLogEntry>,
}

impl DamageLog {
    pub(crate) fn ingest(
        &mut self,
        events: &[DamageEvent],
        player: TankId,
        tanks: &[TankSnapshot],
    ) {
        for event in events {
            let direction = if event.source == player && event.target != player {
                LogDirection::Dealt
            } else if event.target == player && event.source != player {
                LogDirection::Taken
            } else {
                continue;
            };
            let track = event.track_hit.map(|hit| (hit.side, hit.broke));
            // A shell that did nothing is still a hit worth reading — it was held, it skipped,
            // it shattered. What earns no row is a non-shell cause that did nothing.
            if event.cause != DamageCause::Shell
                && event.damage_hp == 0
                && track.is_none()
                && event.crew_hits_mask == 0
            {
                continue;
            }
            let other_id =
                if direction == LogDirection::Dealt { event.target } else { event.source };
            let other_vehicle =
                tanks.iter().find(|tank| tank.tank_id == other_id).map(|tank| tank.vehicle);
            // A fire ticks every few hundred milliseconds; its ticks are one row that grows.
            if event.cause == DamageCause::Fire
                && let Some(front) = self.entries.front_mut()
                && front.cause == DamageCause::Fire
                && front.direction == direction
                && front.other_vehicle == other_vehicle
            {
                front.damage_hp = front.damage_hp.saturating_add(event.damage_hp);
                front.age_s = 0.0;
                continue;
            }
            self.entries.push_front(DamageLogEntry {
                direction,
                damage_hp: event.damage_hp,
                module: event.module,
                track,
                other_vehicle,
                crew_hits_mask: event.crew_hits_mask,
                round: event.round,
                age_s: 0.0,
                cause: event.cause,
                penetrated: event.penetrated,
                ricocheted: event.ricocheted,
                shattered: event.shattered,
                shell_penetration_mm: event.shell_penetration_mm.round().max(0.0) as u32,
                effective_armor_mm: event.effective_armor_mm.round().max(0.0) as u32,
                impact_angle_degrees: event.impact_angle_degrees.round().max(0.0) as u32,
                zone: event.armor_zone,
                distance_m: (event.cause == DamageCause::Shell && event.distance_m > 0.0)
                    .then(|| event.distance_m.round() as u32),
            });
        }
        self.entries.truncate(LOG_CAP);
    }

    pub(crate) fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            entry.age_s += dt;
        }
        self.entries.retain(|entry| entry.age_s < LOG_TTL_S);
    }

    pub(crate) fn visible(&self) -> Vec<DamageLogEntry> {
        self.entries.iter().copied().collect()
    }
}

const LOG_W_U: f32 = 620.0;
const ROW_H_U: f32 = 22.0;
const ROW_GAP_U: f32 = 3.0;
/// Below the reticle's own readouts, where the eye already is.
const LOG_TOP_BELOW_CENTER_U: f32 = 110.0;
const TEXT_U: f32 = 16.0;

/// The log under the reticle: the newest row nearest it, right-aligned, fading at the end of
/// its life; `collapsed` (N) keeps only the newest.
pub(crate) fn push_hit_log(
    list: &mut DrawList<HudElement>,
    ui: &Ui,
    theme: &Theme,
    entries: &[DamageLogEntry],
    collapsed: bool,
    z: &mut i16,
) {
    let mut push = |element: Element<HudElement>| {
        list.push(element.z(*z));
        *z += 1;
    };
    let viewport = ui.viewport();
    let width = ui.px(LOG_W_U);
    let left = viewport.center()[0] - width * 0.5;
    let top = viewport.center()[1] + ui.px(LOG_TOP_BELOW_CENTER_U);
    let enamel = theme.plates.enamel_black;
    let shown = if collapsed { 1 } else { LOG_CAP };
    for (index, entry) in entries.iter().take(shown).enumerate() {
        let alpha = ((LOG_TTL_S - entry.age_s) / FADE_S).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }
        let row =
            Rect::new(left, top + index as f32 * ui.px(ROW_H_U + ROW_GAP_U), width, ui.px(ROW_H_U));
        let i = index as u8;
        push(Element::new(
            HudElement::HitLog(HitLogPart::Row(i)),
            row,
            Payload::Plate {
                tile: enamel.tile,
                radius_u: 2.0,
                bevel_u: 0.0,
                color: [enamel.color[0], enamel.color[1], enamel.color[2], 0.55 * alpha],
            },
        ));
        let tone = theme.semantic.floating[entry.family()];
        let square = ui.px(8.0);
        push(Element::new(
            HudElement::HitLog(HitLogPart::Verdict(i)),
            Rect::new(
                row.right() - ui.px(8.0) - square,
                row.y + (row.h - square) * 0.5,
                square,
                square,
            ),
            Payload::Plate {
                tile: enamel.tile,
                radius_u: 1.0,
                bevel_u: 0.0,
                color: [tone[0], tone[1], tone[2], tone[3] * alpha],
            },
        ));
        let text_color = match entry.direction {
            LogDirection::Dealt => theme.text.label,
            LogDirection::Taken => theme.text.label_dim,
        };
        push(Element::new(
            HudElement::HitLog(HitLogPart::Text(i)),
            Rect::new(row.x + ui.px(8.0), row.y + ui.px(3.0), row.w - ui.px(28.0), ui.px(TEXT_U)),
            Payload::Text {
                text: entry.line(),
                style: Style::VALUE,
                size_u: TEXT_U,
                align: Align::Right,
                color: [text_color[0], text_color[1], text_color[2], text_color[3] * alpha],
                digits: DigitMode::Tabular,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{ArmorFacing, ShellType};

    fn shot(source: u64, target: u64, penetrated: bool, damage_hp: u32) -> DamageEvent {
        DamageEvent {
            source: TankId(source),
            target: TankId(target),
            damage_hp,
            penetrated,
            ricocheted: false,
            cause: DamageCause::Shell,
            shell_type: ShellType::ArmorPiercing,
            impact_angle_degrees: 31.4,
            effective_armor_mm: 162.2,
            shell_penetration_mm: 148.0,
            armor_facing: ArmorFacing::TurretFront,
            armor_zone: ArmorZone::TurretFront,
            round: Some(game_core::RoundId::Br412D),
            distance_m: 412.4,
            ..Default::default()
        }
    }

    fn other(id: u64, kind: VehicleKind) -> TankSnapshot {
        let mut tank = crate::hud::tests::tank_snapshot(id, 2, 1_000);
        tank.vehicle = kind;
        tank
    }

    fn build(entries: &[DamageLogEntry], collapsed: bool) -> DrawList<HudElement> {
        let mut list = DrawList::new();
        let mut z = 0;
        push_hit_log(&mut list, &Ui::reference(), &Theme::standard(), entries, collapsed, &mut z);
        list
    }

    fn text_of(list: &DrawList<HudElement>, index: u8) -> String {
        match &list.find(HudElement::HitLog(HitLogPart::Text(index))).expect("row text").payload {
            Payload::Text { text, .. } => text.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// H8: a dealt penetration prints the word, the damage, the round, pen against effective
    /// armour at the angle, the zone and the hull — the wire's numbers, rounded once.
    #[test]
    fn a_hit_log_row_prints_pen_effective_angle_zone_and_the_outcome_word() {
        let mut log = DamageLog::default();
        let mut event = shot(1, 2, true, 240);
        event.module = Some(ModuleSlot::Gun);
        log.ingest(&[event], TankId(1), &[other(2, VehicleKind::TigerII)]);
        let rows = log.visible();
        assert_eq!(rows.len(), 1);
        let line = text_of(&build(&rows, false), 0);
        assert_eq!(
            line,
            "\u{bb} PEN 240 \u{b7} BR-412D \u{b7} 148 > 162 MM @ 31\u{b0} \u{b7} TURRET FRONT \u{b7} GUN \u{b7} Tiger II"
        );
        assert_eq!(rows[0].family(), 0);
        let row =
            build(&rows, false).find(HudElement::HitLog(HitLogPart::Row(0))).expect("row").rect;
        let ui = Ui::reference();
        assert!(
            row.y > 540.0 && (row.center()[0] - 960.0).abs() < 1.0,
            "under the reticle: {row:?}"
        );
        assert!(ui.viewport().encloses(&row));
    }

    /// H8: a bounce earns a row; a taken hit prints its range (W-6).
    #[test]
    fn a_bounce_earns_a_row() {
        let mut log = DamageLog::default();
        let mut bounce = shot(2, 1, false, 0);
        bounce.ricocheted = true;
        log.ingest(&[bounce], TankId(1), &[other(2, VehicleKind::IS3)]);
        let rows = log.visible();
        assert_eq!(rows.len(), 1, "a held shell is a fact worth a row");
        assert_eq!(rows[0].family(), 2);
        let line = text_of(&build(&rows, false), 0);
        assert!(line.starts_with("\u{ab} RICOCHET \u{b7} BR-412D"), "{line}");
        assert!(line.contains("412 M"), "a taken hit prints its range: {line}");
        // A track hit names its track once: the zone says it, the side stays quiet.
        let mut tracked = shot(2, 1, false, 0);
        tracked.track_hit = Some(game_core::TrackHit { side: TrackSide::Right, broke: true });
        tracked.armor_zone = ArmorZone::RightTrack;
        let mut once = DamageLog::default();
        once.ingest(&[tracked], TankId(1), &[]);
        let tracked_line = text_of(&build(&once.visible(), false), 0);
        assert_eq!(tracked_line.matches("RIGHT TRACK").count(), 1, "{tracked_line}");
        assert!(tracked_line.starts_with("\u{ab} TRACKED"), "{tracked_line}");
        assert!(line.ends_with("IS-3"), "{line}");
        // A ram that did nothing earns none.
        let mut nothing = shot(2, 1, false, 0);
        nothing.cause = DamageCause::Ram;
        log.ingest(&[nothing], TankId(1), &[]);
        assert_eq!(log.visible().len(), 1);
    }

    /// H8: N keeps the newest row only; the ticks of one fire are one row that grows.
    #[test]
    fn n_collapses_the_log_to_its_last_row_and_fire_ticks_coalesce() {
        let mut log = DamageLog::default();
        log.ingest(&[shot(1, 2, true, 100), shot(1, 2, true, 120)], TankId(1), &[]);
        assert_eq!(log.visible().len(), 2);
        assert_eq!(
            build(&log.visible(), false)
                .iter()
                .filter(|e| matches!(e.id, HudElement::HitLog(HitLogPart::Row(_))))
                .count(),
            2
        );
        assert_eq!(
            build(&log.visible(), true)
                .iter()
                .filter(|e| matches!(e.id, HudElement::HitLog(HitLogPart::Row(_))))
                .count(),
            1
        );
        let mut fire = shot(2, 1, false, 12);
        fire.cause = DamageCause::Fire;
        log.ingest(&[fire, fire, fire], TankId(1), &[]);
        let rows = log.visible();
        assert_eq!(rows.len(), 3, "three ticks, one row");
        assert_eq!(rows[0].damage_hp, 36);
        assert_eq!(rows[0].word(), "FIRE");
    }
}
