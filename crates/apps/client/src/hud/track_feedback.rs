//! The thrown-track beat (interface program H4): what the damage panel pulses when the
//! player's own track is hit — a state machine over the damage events and the frame clock.
//! The re-seat bar it used to run locally is the server's clock now (W-4, the panel's
//! `TrackClock`); the callout text it used to print is the track's pulse.

use game_core::{DamageEvent, TankId, TrackSide};

/// How long the beat lasts.
pub(crate) const CALLOUT_TTL_S: f32 = 2.6;

#[derive(Debug, Default)]
pub(crate) struct TrackFeedback {
    callout: Option<Callout>,
}

#[derive(Debug, Clone, Copy)]
struct Callout {
    broke: bool,
    side: TrackSide,
    age_s: f32,
}

impl TrackFeedback {
    /// Raise a callout for any track the player TOOK this snapshot. A fresh throw always wins over
    /// a lingering mere-damage callout; a second damage does not stomp a still-fresh throw.
    pub(crate) fn ingest(&mut self, events: &[DamageEvent], player: TankId) {
        for event in events {
            if event.target != player {
                continue;
            }
            let Some(hit) = event.track_hit else { continue };
            let supersede = self.callout.is_none_or(|current| hit.broke || !current.broke);
            if supersede {
                self.callout = Some(Callout { broke: hit.broke, side: hit.side, age_s: 0.0 });
            }
        }
    }

    /// Watch the player's own broken sides (from the replicated broken mask) to run the re-seat
    /// bars: a side newly down starts its clock, a re-seated side clears it.
    pub(crate) fn tick(&mut self, dt: f32) {
        if let Some(callout) = &mut self.callout {
            callout.age_s += dt;
            if callout.age_s >= CALLOUT_TTL_S {
                self.callout = None;
            }
        }
    }

    /// Snapshot the drawable state for one frame.
    /// The live callout, for the damage panel's track pulse (H4).
    pub(crate) fn callout(&self) -> Option<CalloutView> {
        self.callout.map(|c| CalloutView { broke: c.broke, side: c.side, age_s: c.age_s })
    }
}

/// The callout as the panel reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalloutView {
    pub broke: bool,
    pub side: TrackSide,
    pub age_s: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{DamageEvent, TrackHit};

    fn track_event(target: u64, side: TrackSide, broke: bool) -> DamageEvent {
        DamageEvent {
            target: TankId(target),
            track_hit: Some(TrackHit { side, broke }),
            ..Default::default()
        }
    }

    #[test]
    fn a_taken_throw_raises_a_destroyed_callout_over_a_damage_one() {
        let mut fb = TrackFeedback::default();
        fb.ingest(&[track_event(1, TrackSide::Left, false)], TankId(1));
        assert!(fb.callout().is_some_and(|c| !c.broke), "damage callout first");
        fb.ingest(&[track_event(1, TrackSide::Left, true)], TankId(1));
        assert!(fb.callout().is_some_and(|c| c.broke), "a throw supersedes the damage callout");
    }

    #[test]
    fn other_tanks_track_hits_do_not_call_out_for_the_player() {
        let mut fb = TrackFeedback::default();
        fb.ingest(&[track_event(2, TrackSide::Left, true)], TankId(1));
        assert!(fb.callout().is_none(), "only the player's own tracks call out");
    }

    #[test]
    fn the_callout_fades_out_after_its_ttl() {
        let mut fb = TrackFeedback::default();
        fb.ingest(&[track_event(1, TrackSide::Right, true)], TankId(1));
        fb.tick(CALLOUT_TTL_S + 0.1);
        assert!(fb.callout().is_none(), "the callout ages out");
    }
}
