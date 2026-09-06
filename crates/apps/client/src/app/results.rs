//! The results page's model (interface program P1, P2, P10): the ledger's records and nothing
//! else — the crew's own numbers; the timeline, every row backed by a wire identity (a stamped
//! damage event, a kill, an own shot's shell, an own spotted span, an observer from the end
//! word) and never a synthesised one; the roster with alive/dead and no statistics; a REPLAY
//! that is disabled and says why (L3 has no viewer yet), naming the recording when the
//! session wrote one.

use game_core::{DamageEvent, KillEvent, TankId, TeamId};
use net::RosterEntry;

use super::keybinds::{Action, KeyBindings};
use super::ledger::BattleLedger;
use crate::hud::BattleHudOutcome;
use crate::hud::damage_log::{damage_word, module_name, zone_name};
use crate::hud::elements::ShellPart;
use crate::hud::kill_feed::cause_word;
use crate::hud::shell::{
    ReplayNote, ResultsScreenModel, ResultsTab, RowSource, StatRow, TeamRow, TimelineRow,
};
use crate::ui_strings::battle as words;

/// M:SS at the server's tick rate.
pub(crate) fn clock_word(tick: u64) -> String {
    let seconds = tick / sim::DEFAULT_SERVER_TICK_HZ as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// A span's length in whole seconds.
fn span_word(from_tick: u64, to_tick: u64) -> String {
    let seconds = to_tick.saturating_sub(from_tick) / sim::DEFAULT_SERVER_TICK_HZ as u64;
    format!("{seconds} S")
}

/// The roster's word for a hull — "T-54 · B" — or UNSEEN for one the roster never named.
pub(crate) fn hull_word(roster: &[RosterEntry], id: TankId) -> String {
    roster.iter().find(|entry| entry.tank_id == id).map_or_else(
        || words::TL_UNSEEN.to_string(),
        |entry| format!("{} \u{b7} {}", entry.vehicle.short_name(), entry.seat_letter()),
    )
}

/// A hit's detail: the round, pen › effective @ angle, the zone, the module, the damage, the
/// range — what the hit log says, kept for good.
fn hit_detail(event: &DamageEvent, other: &str) -> String {
    let mut parts: Vec<String> = vec![other.to_string()];
    if let Some(round) = event.round {
        parts.push(round.designation().to_string());
    }
    if event.shell_id.is_some() {
        parts.push(format!(
            "{} > {} {} @ {}\u{b0}",
            event.shell_penetration_mm.round() as u32,
            event.effective_armor_mm.round() as u32,
            words::MILLIMETRES,
            event.impact_angle_degrees.round() as u32
        ));
        parts.push(zone_name(event.armor_zone).to_string());
    }
    if let Some(module) = event.module {
        parts.push(module_name(module).to_string());
    }
    parts.push(event.damage_hp.to_string());
    if event.distance_m > 0.5 {
        parts.push(format!("{} {}", event.distance_m.round() as u32, words::DISTANCE_UNIT));
    }
    parts.join(" \u{b7} ")
}

fn event_word(event: &DamageEvent) -> &'static str {
    damage_word(
        event.cause,
        event.penetrated,
        event.ricocheted,
        event.shattered,
        event.track_hit.is_some(),
    )
}

/// The footer of the results page.
pub(crate) fn results_footer(keybinds: &KeyBindings) -> String {
    let first = |action: Action| {
        keybinds
            .keys(action)
            .first()
            .map_or_else(|| "-".to_string(), |key| super::keybinds::key_label(*key))
    };
    format!(
        "{}/{} {} \u{b7} {}/{} {} \u{b7} {} {} \u{b7} {} {}",
        first(Action::MenuLeft),
        first(Action::MenuRight),
        words::TAB_WORD,
        first(Action::MenuUp),
        first(Action::MenuDown),
        words::FOOTER_SCROLL,
        first(Action::MenuAccept),
        words::FOOTER_CONTINUE,
        first(Action::MenuBack),
        words::FOOTER_BACK
    )
}

/// What the page is built from.
pub(crate) struct ResultsInputs<'a> {
    pub ledger: &'a BattleLedger,
    pub roster: &'a [RosterEntry],
    pub player_team: TeamId,
    pub recording: Option<String>,
    pub tab: ResultsTab,
    pub first_visible: usize,
    pub hovered: Option<ShellPart>,
    pub footer: String,
}

/// The page off the ledger: nothing it does not hold.
pub(crate) fn results_model(inputs: ResultsInputs<'_>) -> ResultsScreenModel {
    let ledger = inputs.ledger;
    let player = ledger.player();
    let roster = inputs.roster;
    let own = ledger.own();
    let ended = ledger.ended();
    let stat = |label: &str, value: String| StatRow { label: label.to_string(), value };
    let stats = vec![
        stat(words::STAT_SHOTS, own.shots.to_string()),
        stat(words::STAT_HITS, own.hits.to_string()),
        stat(words::STAT_PENETRATIONS, own.penetrations.to_string()),
        stat(words::STAT_DAMAGE_DEALT, own.damage_dealt.to_string()),
        stat(words::STAT_DAMAGE_TAKEN, own.damage_taken.to_string()),
        stat(words::STAT_KILLS, own.kills.to_string()),
        stat(words::STAT_SPOTTED, own.spotted_spans.to_string()),
        stat(words::STAT_SEEN_BY, ledger.observers().len().to_string()),
        stat(
            words::STAT_DURATION,
            ended.map_or_else(|| "-".to_string(), |end| clock_word(end.tick)),
        ),
    ];

    let mut rows: Vec<TimelineRow> = Vec::new();
    let mut push = |tick: u64, word: &str, detail: String, source: RowSource| {
        rows.push(TimelineRow {
            tick,
            clock: clock_word(tick),
            word: word.to_string(),
            detail,
            source,
        });
    };
    let damage: Vec<&DamageEvent> = ledger.damage().collect();
    // Every own shot: with its result when a damage event shares the shell, or NO HIT SEEN.
    for own_shot in ledger.shots() {
        let result = damage
            .iter()
            .find(|event| event.source == player && event.shell_id == Some(own_shot.shot.shell_id));
        match result {
            Some(event) => push(
                event.occurred_tick,
                event_word(event),
                hit_detail(event, &hull_word(roster, event.target)),
                RowSource::Shot(own_shot.shot.shell_id),
            ),
            None => push(
                own_shot.tick,
                words::TL_SHOT,
                words::TL_NO_HIT.to_string(),
                RowSource::Shot(own_shot.shot.shell_id),
            ),
        }
    }
    // Every other own hit (a ram, a fire lit) and every hit taken, by its event id.
    for event in &damage {
        let dealt_by_shot = event.source == player
            && event.shell_id.is_some_and(|shell| {
                ledger.shots().iter().any(|own_shot| own_shot.shot.shell_id == shell)
            });
        if event.source == player && event.target != player && !dealt_by_shot {
            push(
                event.occurred_tick,
                event_word(event),
                hit_detail(event, &hull_word(roster, event.target)),
                RowSource::Damage(event.event_id),
            );
        } else if event.target == player {
            let other = if event.source == player {
                event_word(event).to_string()
            } else {
                hull_word(roster, event.source)
            };
            push(
                event.occurred_tick,
                words::TL_TAKEN,
                hit_detail(event, &other),
                RowSource::Damage(event.event_id),
            );
        }
    }
    // Every kill on the field; the crew's own death by its name.
    for kill in ledger.kills() {
        let by = match kill.killer {
            Some(killer) if killer == player => words::WORD_YOU.to_string(),
            Some(killer) => hull_word(roster, killer),
            None => cause_word(kill.cause).to_string(),
        };
        let (word, detail) = if kill.victim == player {
            (words::TL_HULL_LOST, format!("{} {by}", words::TL_BY))
        } else {
            (
                words::TL_DESTROYED,
                format!("{} \u{b7} {} {by}", hull_word(roster, kill.victim), words::TL_BY),
            )
        };
        push(
            kill.occurred_tick,
            word,
            detail,
            RowSource::Kill { victim: kill.victim, tick: kill.occurred_tick },
        );
    }
    // Every own spotted span, and — from the end word only — every enemy that saw the crew.
    for span in ledger.spotted_spans() {
        let to = span.to_tick.or(ended.map(|end| end.tick)).unwrap_or(span.from_tick);
        push(
            span.from_tick,
            words::SPOTTED_LAMP,
            span_word(span.from_tick, to),
            RowSource::Spotted { from_tick: span.from_tick },
        );
    }
    for record in ledger.observers() {
        push(
            record.from_tick,
            words::TL_SEEN_BY,
            format!(
                "{} \u{b7} {} {} \u{b7} {}",
                hull_word(roster, record.observer),
                record.distance_m.round() as u32,
                words::DISTANCE_UNIT,
                span_word(record.from_tick, record.to_tick)
            ),
            RowSource::Observer { observer: record.observer, from_tick: record.from_tick },
        );
    }
    rows.sort_by_key(|row| row.tick);

    let kills: Vec<&KillEvent> = ledger.kills().collect();
    let mut team: Vec<TeamRow> = roster
        .iter()
        .map(|entry| TeamRow {
            name: hull_word(roster, entry.tank_id),
            ally: entry.team == inputs.player_team,
            human: entry.crew_kind == net::CrewKind::Human,
            destroyed: kills.iter().any(|kill| kill.victim == entry.tank_id),
            player: entry.tank_id == player,
        })
        .collect();
    team.sort_by_key(|row| !row.ally);

    ResultsScreenModel {
        outcome: ended.map_or(BattleHudOutcome::BattleOver, |end| end.outcome),
        hull: hull_word(roster, player),
        stats,
        rows,
        team,
        replay: ReplayNote {
            reason: words::REPLAY_REASON.to_string(),
            recording: inputs.recording,
        },
        tab: inputs.tab,
        first_visible: inputs.first_visible,
        hovered: inputs.hovered,
        footer: inputs.footer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{BattleEventId, DamageCause, ShellId, ShotFired, VehicleKind};

    fn roster() -> Vec<RosterEntry> {
        let entry = |id: u64, team: u16, seat: u8, kind: net::CrewKind| RosterEntry {
            tank_id: TankId(id),
            team: TeamId(team),
            vehicle: VehicleKind::BENCHMARK,
            seat,
            crew_kind: kind,
        };
        vec![
            entry(1, 1, 0, net::CrewKind::Human),
            entry(2, 1, 1, net::CrewKind::Bot),
            entry(9, 2, 1, net::CrewKind::Bot),
        ]
    }

    fn hit(
        id: u64,
        tick: u64,
        source: u64,
        target: u64,
        shell: Option<u64>,
        damage: u32,
    ) -> DamageEvent {
        DamageEvent {
            source: TankId(source),
            target: TankId(target),
            damage_hp: damage,
            penetrated: damage > 0,
            cause: DamageCause::Shell,
            event_id: BattleEventId(id),
            occurred_tick: tick,
            shell_id: shell.map(ShellId),
            shell_penetration_mm: 148.0,
            effective_armor_mm: 132.0,
            impact_angle_degrees: 22.0,
            distance_m: 310.0,
            ..Default::default()
        }
    }

    /// A ledger of one fight: two own shots (one struck, one missed), one hit taken, the enemy
    /// killed by the crew, one spotted span, one observer named by the end word.
    fn ledger() -> BattleLedger {
        let me = TankId(1);
        let mut ledger = BattleLedger::new(me);
        let snapshot = net::Snapshot {
            server_tick: 1_200,
            damage_events: vec![
                hit(31, 1_200, 1, 9, Some(4), 240),
                hit(32, 1_380, 9, 1, Some(70), 90),
            ],
            shots_fired: vec![
                ShotFired { shooter: me, shell_id: ShellId(4) },
                ShotFired { shooter: me, shell_id: ShellId(6) },
            ],
            ..Default::default()
        };
        ledger.ingest_snapshot(&snapshot, me);
        ledger.ingest_kills(&[KillEvent {
            victim: TankId(9),
            killer: Some(me),
            cause: DamageCause::Shell,
            occurred_tick: 2_400,
        }]);
        ledger.spotted(1_140, true);
        ledger.spotted(1_980, false);
        ledger.end(4_000, BattleHudOutcome::Victory);
        ledger.name_observers(vec![net::SpottingRecord {
            observer: TankId(9),
            distance_m: 308.5,
            from_tick: 1_100,
            to_tick: 1_640,
        }]);
        ledger
    }

    fn model(ledger: &BattleLedger, recording: Option<&str>) -> ResultsScreenModel {
        let roster = roster();
        results_model(ResultsInputs {
            ledger,
            roster: &roster,
            player_team: TeamId(1),
            recording: recording.map(str::to_string),
            tab: ResultsTab::Summary,
            first_visible: 0,
            hovered: None,
            footer: String::new(),
        })
    }

    /// P1: the page prints only what the ledger holds — the numbers are the tally's, the
    /// duration is the end's tick, the roster is the wire's, alive/dead is the kills' word, the
    /// REPLAY is disabled and says why.
    #[test]
    fn the_results_screen_prints_only_what_the_ledger_holds() {
        let ledger = ledger();
        let page = model(&ledger, Some("battles/one.wotrec"));
        let value = |label: &str| {
            page.stats.iter().find(|stat| stat.label == label).expect(label).value.clone()
        };
        assert_eq!(value(words::STAT_SHOTS), "2");
        assert_eq!(value(words::STAT_HITS), "1");
        assert_eq!(value(words::STAT_PENETRATIONS), "1");
        assert_eq!(value(words::STAT_DAMAGE_DEALT), "240");
        assert_eq!(value(words::STAT_DAMAGE_TAKEN), "90");
        assert_eq!(value(words::STAT_KILLS), "1");
        assert_eq!(value(words::STAT_SPOTTED), "1");
        assert_eq!(value(words::STAT_SEEN_BY), "1");
        assert_eq!(value(words::STAT_DURATION), "1:06");
        assert_eq!(page.outcome, BattleHudOutcome::Victory);
        assert_eq!(page.hull, format!("{} \u{b7} A", VehicleKind::BENCHMARK.short_name()));
        assert_eq!(page.team.len(), 3);
        assert!(page.team[0].player && page.team[0].ally && page.team[0].human);
        assert!(page.team[1].ally && !page.team[1].destroyed);
        assert!(!page.team[2].ally && page.team[2].destroyed, "the kill's word");
        assert_eq!(page.replay.reason, words::REPLAY_REASON);
        assert_eq!(page.replay.recording.as_deref(), Some("battles/one.wotrec"));
        // A live ledger: no outcome, no duration, no observers.
        let mut live = BattleLedger::new(TankId(1));
        live.spotted(10, true);
        let page = model(&live, None);
        assert_eq!(page.outcome, BattleHudOutcome::BattleOver);
        assert_eq!(
            page.stats
                .iter()
                .find(|stat| stat.label == words::STAT_DURATION)
                .expect("duration")
                .value,
            "-"
        );
        assert!(page.rows.iter().all(|row| !matches!(row.source, RowSource::Observer { .. })));
    }

    /// P2: every timeline row is backed by a wire identity the ledger holds — a shot's shell,
    /// a stamped event, a kill, a span, an observer — one row per record, in tick order, and
    /// the words are the hit log's.
    #[test]
    fn every_timeline_row_is_backed_by_a_wire_event_id() {
        let ledger = ledger();
        let page = model(&ledger, None);
        // two shots (one with its result) + one hit taken + one kill + one span + one observer
        assert_eq!(page.rows.len(), 6, "{:?}", page.rows);
        assert!(page.rows.windows(2).all(|pair| pair[0].tick <= pair[1].tick), "tick order");
        for row in &page.rows {
            let backed = match row.source {
                RowSource::Damage(id) => ledger.damage().any(|event| event.event_id == id),
                RowSource::Shot(shell) => {
                    ledger.shots().iter().any(|own| own.shot.shell_id == shell)
                }
                RowSource::Kill { victim, tick } => {
                    ledger.kills().any(|kill| kill.victim == victim && kill.occurred_tick == tick)
                }
                RowSource::Spotted { from_tick } => {
                    ledger.spotted_spans().iter().any(|span| span.from_tick == from_tick)
                }
                RowSource::Observer { observer, from_tick } => ledger
                    .observers()
                    .iter()
                    .any(|record| record.observer == observer && record.from_tick == from_tick),
            };
            assert!(backed, "{row:?} is backed by nothing on the wire");
        }
        let words_of: Vec<&str> = page.rows.iter().map(|row| row.word.as_str()).collect();
        assert_eq!(
            words_of,
            vec![
                words::TL_SEEN_BY,
                words::SPOTTED_LAMP,
                words::HIT_PEN,
                words::TL_SHOT,
                words::TL_TAKEN,
                words::TL_DESTROYED
            ]
        );
        let pen = &page.rows[2];
        assert!(pen.detail.contains("148 > 132 MM @ 22\u{b0}"), "{}", pen.detail);
        assert!(pen.detail.ends_with("240 \u{b7} 310 M"), "{}", pen.detail);
        assert_eq!(page.rows[3].detail, words::TL_NO_HIT);
        assert!(page.rows[5].detail.ends_with(&format!("{} {}", words::TL_BY, words::WORD_YOU)));
        assert_eq!(page.rows[0].clock, "0:18");
    }
}
