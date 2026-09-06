//! The spotting log (protocol v52, interface program W-7): who saw whom, from how far, from
//! when to when — kept from the observer masks the host already computes for the per-viewer
//! cut, one word per snapshot tick; closed at the battle's end; read for ONE hull at a time,
//! and only once the battle is over, so a live client never holds an observer.

use std::collections::HashMap;

use game_core::TankId;
use net::SpottingRecord;
use sim::{MAX_OBSERVERS, ObserverMask, TankState};

#[derive(Debug, Default, Clone)]
pub struct SpottingLog {
    /// Open spans by (target, observer): the tick they opened on and the range then.
    open: HashMap<(TankId, TankId), (u64, f32)>,
    /// Closed spans by target, in the order they closed.
    closed: Vec<(TankId, SpottingRecord)>,
    finished: bool,
}

impl SpottingLog {
    /// One recompute's word: an enemy observer's fresh line of sight on a living target opens
    /// a span (at the range of that moment); its loss closes one. Allies are never logged —
    /// they always see their own — and a wreck is nobody's sighting.
    pub fn observe(&mut self, tanks: &[TankState], masks: &[ObserverMask], tick: u64) {
        if self.finished {
            return;
        }
        for (target_index, target) in tanks.iter().enumerate() {
            let Some(mask) = masks.get(target_index) else { continue };
            for (observer_index, observer) in tanks.iter().enumerate().take(MAX_OBSERVERS) {
                if observer.team == target.team || observer.id == target.id {
                    continue;
                }
                let sees = target.hit_points > 0
                    && observer.hit_points > 0
                    && mask & ((1 as ObserverMask) << observer_index) != 0;
                let key = (target.id, observer.id);
                match (self.open.get(&key).copied(), sees) {
                    (None, true) => {
                        let distance_m = observer.position.distance(target.position);
                        self.open.insert(key, (tick, distance_m));
                    }
                    (Some((from_tick, distance_m)), false) => {
                        self.open.remove(&key);
                        self.closed.push((
                            target.id,
                            SpottingRecord {
                                observer: observer.id,
                                distance_m,
                                from_tick,
                                to_tick: tick,
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    /// The battle's end: every open span closes on this tick; nothing is logged after it.
    pub fn finish(&mut self, tick: u64) {
        if self.finished {
            return;
        }
        self.finished = true;
        let mut open: Vec<_> = std::mem::take(&mut self.open).into_iter().collect();
        open.sort_by_key(|((target, observer), _)| (target.0, observer.0));
        for ((target, observer), (from_tick, distance_m)) in open {
            self.closed
                .push((target, SpottingRecord { observer, distance_m, from_tick, to_tick: tick }));
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// One crew's log — every enemy that saw THIS hull, in the order the spans closed — and
    /// nothing at all while the battle runs: the observer is named after the battle only.
    pub fn for_target(&self, target: TankId) -> Vec<SpottingRecord> {
        if !self.finished {
            return Vec::new();
        }
        self.closed.iter().filter(|(t, _)| *t == target).map(|(_, record)| *record).collect()
    }
}
