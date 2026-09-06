//! The battle ledger (interface program P3): everything a battle is, accumulated on the
//! client from the wire's own words and nothing else — the reliable lane's damage events and
//! kills (each stamped with its `BattleEventId` / its tick), the own shots the snapshot names
//! (`shots_fired`, v41), the own spotted spans off the own `spotted_by_teams_mask` (the one
//! bit the filter never hides), and the battle's end. The results screen (P1) and the timeline
//! (P2) read from here; the eight-second hit log (H8) never was a record.
//!
//! Honesty: one record per event id, never a synthesised one. An unstamped event (id zero —
//! a presentation-only pulse) is not a record; a redelivered one is the same record; a kill
//! is a kill only when the wire said so. A replay of the same words, in any order, rebuilds
//! the same ledger — the lock below plays them twice and shuffled.

use std::collections::BTreeMap;

use game_core::{BattleEventId, DamageEvent, KillEvent, ShotFired, TankId};

use crate::hud::BattleHudOutcome;

/// A span the own hull was spotted for, in server ticks; `to_tick` is `None` while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpottedSpan {
    pub from_tick: u64,
    pub to_tick: Option<u64>,
}

/// How the battle ended for this crew, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BattleEnd {
    pub tick: u64,
    pub outcome: BattleHudOutcome,
}

/// The crew's own numbers, summed from the records (the results screen's first tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct OwnTally {
    pub shots: u32,
    /// Own shells that struck a hull (a damage event with the crew as its source).
    pub hits: u32,
    pub penetrations: u32,
    pub damage_dealt: u32,
    pub damage_taken: u32,
    pub kills: u32,
    pub spotted_spans: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct BattleLedger {
    player: TankId,
    /// One record per stamped event id, in id order.
    damage: BTreeMap<BattleEventId, DamageEvent>,
    /// One record per (victim, tick): the wire says a hull dies once.
    kills: BTreeMap<(u64, u64), KillEvent>,
    /// The own shots, in the order the snapshots named them.
    shots: Vec<ShotFired>,
    spotted: Vec<SpottedSpan>,
    ended: Option<BattleEnd>,
}

// The readers — the results screen (P1) and the timeline (P2) — land after the ledger; until
// they do, the accessors are the locks' alone.
#[cfg_attr(not(test), allow(dead_code))]
impl BattleLedger {
    pub(crate) fn new(player: TankId) -> Self {
        Self { player, ..Default::default() }
    }

    pub(crate) fn player(&self) -> TankId {
        self.player
    }

    /// A snapshot's words: the damage events it carries (stamped ones only) and the own shots.
    /// The crew's own hull is named with every snapshot, so a record started before the
    /// session knew its seat still counts the right shots.
    pub(crate) fn ingest_snapshot(&mut self, snapshot: &net::Snapshot, player: TankId) {
        self.player = player;
        for event in &snapshot.damage_events {
            if event.event_id == BattleEventId(0) {
                continue;
            }
            self.damage.entry(event.event_id).or_insert(*event);
        }
        for shot in &snapshot.shots_fired {
            if shot.shooter == self.player && !self.shots.contains(shot) {
                self.shots.push(*shot);
            }
        }
    }

    /// The field's kills (W-3), as told.
    pub(crate) fn ingest_kills(&mut self, kills: &[KillEvent]) {
        for kill in kills {
            self.kills.entry((kill.victim.0, kill.occurred_tick)).or_insert(*kill);
        }
    }

    /// The own mask at `tick`: a rising edge opens a span, a falling one closes it.
    pub(crate) fn spotted(&mut self, tick: u64, lit: bool) {
        match (self.spotted.last_mut(), lit) {
            (Some(span), false) if span.to_tick.is_none() => span.to_tick = Some(tick),
            (Some(span), true) if span.to_tick.is_none() => {}
            (_, true) => self.spotted.push(SpottedSpan { from_tick: tick, to_tick: None }),
            (_, false) => {}
        }
    }

    /// The end, once: the first word stands.
    pub(crate) fn end(&mut self, tick: u64, outcome: BattleHudOutcome) {
        if self.ended.is_none() {
            self.ended = Some(BattleEnd { tick, outcome });
            if let Some(span) = self.spotted.last_mut()
                && span.to_tick.is_none()
            {
                span.to_tick = Some(tick);
            }
        }
    }

    pub(crate) fn ended(&self) -> Option<BattleEnd> {
        self.ended
    }

    /// Every damage record, in event-id order.
    pub(crate) fn damage(&self) -> impl Iterator<Item = &DamageEvent> {
        self.damage.values()
    }

    /// Every kill, in (victim, tick) order.
    pub(crate) fn kills(&self) -> impl Iterator<Item = &KillEvent> {
        self.kills.values()
    }

    pub(crate) fn shots(&self) -> &[ShotFired] {
        &self.shots
    }

    pub(crate) fn spotted_spans(&self) -> &[SpottedSpan] {
        &self.spotted
    }

    /// The crew's own numbers, summed from the records alone.
    pub(crate) fn own(&self) -> OwnTally {
        let mut tally = OwnTally { shots: self.shots.len() as u32, ..Default::default() };
        for event in self.damage.values() {
            if event.source == self.player && event.target != self.player {
                tally.hits += u32::from(event.shell_id.is_some());
                tally.penetrations += u32::from(event.penetrated);
                tally.damage_dealt += event.damage_hp;
            }
            if event.target == self.player {
                tally.damage_taken += event.damage_hp;
            }
        }
        tally.kills =
            self.kills.values().filter(|kill| kill.killer == Some(self.player)).count() as u32;
        tally.spotted_spans = self.spotted.len() as u32;
        tally
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{DamageCause, ShellId};

    fn damage(id: u64, source: u64, target: u64, hp: u32, pen: bool) -> DamageEvent {
        DamageEvent {
            source: TankId(source),
            target: TankId(target),
            damage_hp: hp,
            penetrated: pen,
            event_id: BattleEventId(id),
            occurred_tick: id * 10,
            shell_id: Some(ShellId(id)),
            cause: DamageCause::Shell,
            ..Default::default()
        }
    }

    fn snapshot(events: Vec<DamageEvent>, shots: Vec<ShotFired>) -> net::Snapshot {
        net::Snapshot { damage_events: events, shots_fired: shots, ..Default::default() }
    }

    /// P3: one record per event id, never a synthesised one — a redelivery is the same
    /// record, an unstamped pulse is none, a kill is only the wire's — and a replay of the
    /// same words in another order rebuilds the same ledger, sums and all.
    #[test]
    fn the_ledger_holds_one_record_per_event_id_and_never_a_synthesised_one() {
        let me = TankId(1);
        let words = [
            damage(7, 1, 12, 240, true),
            damage(8, 12, 1, 90, true),
            damage(9, 1, 13, 0, false),
            DamageEvent { event_id: BattleEventId(0), ..damage(0, 1, 12, 999, true) },
        ];
        let shots = [
            ShotFired { shooter: me, shell_id: ShellId(7) },
            ShotFired { shooter: TankId(12), shell_id: ShellId(8) },
        ];
        let kills = [
            KillEvent {
                victim: TankId(12),
                killer: Some(me),
                cause: DamageCause::Shell,
                occurred_tick: 70,
            },
            KillEvent {
                victim: TankId(2),
                killer: None,
                cause: DamageCause::Drowning,
                occurred_tick: 90,
            },
        ];
        let mut live = BattleLedger::new(me);
        live.ingest_snapshot(&snapshot(words[..2].to_vec(), shots.to_vec()), me);
        live.ingest_snapshot(&snapshot(words[..2].to_vec(), shots.to_vec()), me);
        live.ingest_snapshot(&snapshot(words[2..].to_vec(), Vec::new()), me);
        live.ingest_kills(&kills);
        live.ingest_kills(&kills);
        live.spotted(20, true);
        live.spotted(25, true);
        live.spotted(60, false);
        live.spotted(80, true);
        live.end(100, BattleHudOutcome::Victory);
        live.end(200, BattleHudOutcome::Defeat);
        assert_eq!(
            live.damage().count(),
            3,
            "one record per stamped id; the unstamped pulse is none"
        );
        assert_eq!(live.kills().count(), 2);
        assert_eq!(live.shots().len(), 1, "the own shot, once");
        assert_eq!(
            live.spotted_spans(),
            &[
                SpottedSpan { from_tick: 20, to_tick: Some(60) },
                SpottedSpan { from_tick: 80, to_tick: Some(100) }
            ]
        );
        assert_eq!(live.ended(), Some(BattleEnd { tick: 100, outcome: BattleHudOutcome::Victory }));
        assert_eq!(
            live.own(),
            OwnTally {
                shots: 1,
                hits: 2,
                penetrations: 1,
                damage_dealt: 240,
                damage_taken: 90,
                kills: 1,
                spotted_spans: 2
            }
        );
        // The replay: the same words, shuffled and repeated, make the same ledger.
        let mut replay = BattleLedger::new(me);
        replay.ingest_kills(&[kills[1], kills[0]]);
        replay.ingest_snapshot(&snapshot(vec![words[2], words[3], words[0]], Vec::new()), me);
        replay.ingest_snapshot(&snapshot(vec![words[1], words[0]], shots.to_vec()), me);
        replay.ingest_kills(&kills);
        replay.spotted(20, true);
        replay.spotted(60, false);
        replay.spotted(80, true);
        replay.end(100, BattleHudOutcome::Victory);
        assert_eq!(replay, live, "a record rebuilds the ledger");
        // Nothing is invented: a ledger fed no kill has no kill, whatever the damage said.
        let mut quiet = BattleLedger::new(me);
        quiet.ingest_snapshot(
            &snapshot(
                vec![DamageEvent { target_destroyed: true, ..damage(5, 1, 12, 500, true) }],
                Vec::new(),
            ),
            me,
        );
        assert_eq!(quiet.own().kills, 0, "a kill is the wire's word (W-3), never a guess");
    }
}
