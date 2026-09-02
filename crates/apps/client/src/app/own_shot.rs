//! The player's own shot in the frame after the trigger (Inny Poziom S13).
//!
//! Every channel of the shot — flash, light, smoke, deck dust, barrel stroke, hull rock, camera
//! nudge, report, breech cycle — used to be driven from `snapshot.shots_fired`, which arrives at
//! 20 Hz: between the trigger and the flash there were 0–50 ms of random delay even in the
//! local battle, and half a round trip on top of that remotely. The gun itself fired with no
//! network delay; the PICTURE of it did not. This is the local prediction of that one fact.
//!
//! The rule is the server's rule (`sim::combat::try_fire_shell`): a live hull, a gun and a rack
//! still standing, a loaded slot, and a reload that is either over or inside the buffer the
//! server holds a click for (`FIRE_BUFFER_S`). When the client's copy of that state says the
//! shot is accepted, the shot is fanned out NOW, from the predicted pose; when it says the click
//! is held, the fan-out is scheduled for the tick the reload ends. The replicated `ShotFired`
//! that follows is matched by count and skipped, so nothing plays twice — and a prediction the
//! server never confirms (a hull killed inside the window, a gun broken by a hit the client had
//! not seen) expires after a third of a second, realigning the count: one flash without a shell,
//! never a missing flash for the shell that did leave.

use game_core::TankId;

/// How long an unconfirmed prediction may wait for its replicated shot before it is written off
/// — three snapshot windows and a half, generous against jitter, short enough that a wrong
/// guess cannot swallow the next real shot.
pub(super) const PREDICTION_EXPIRY_S: f32 = 0.35;

#[derive(Debug, Default)]
pub(super) struct OwnShotPrediction {
    /// Seconds until a HELD click fires (the reload's remainder); `None` when nothing is held.
    held_delay_s: Option<f32>,
    /// Predictions fanned out and not yet matched by a replicated shot, oldest first (their age
    /// in seconds).
    unconfirmed: Vec<f32>,
    /// Shots predicted so far — the index the predicted shell borrows.
    predicted_total: u32,
    /// The client's own copy of the reload after a predicted shot: a second click inside the
    /// snapshot window that has not yet reported the reload must not predict a second flash.
    lockout_s: f32,
}

impl OwnShotPrediction {
    /// The index the next predicted shot borrows for its shell id (presentation only).
    pub fn next_index(&self) -> u32 {
        self.predicted_total
    }

    /// A shot fanned out now: remember it, so its replicated twin is skipped, and start the
    /// local reload lockout (`reload_s`, the gun's reload) that the snapshot will confirm.
    pub fn predicted(&mut self, reload_s: f32) {
        self.predicted_total += 1;
        self.unconfirmed.push(0.0);
        self.lockout_s = self.lockout_s.max(reload_s);
    }

    /// Seconds of reload the client itself still counts after its last predicted shot.
    pub fn lockout_s(&self) -> f32 {
        self.lockout_s
    }

    /// A held click: fire the prediction when the reload's remainder runs out. A second hold
    /// while one is pending keeps the earlier (sooner) one.
    pub fn hold(&mut self, delay_s: f32) {
        let delay_s = delay_s.max(0.0);
        self.held_delay_s = Some(self.held_delay_s.map_or(delay_s, |d| d.min(delay_s)));
    }

    /// Advance one fixed tick: age the unconfirmed predictions (writing off the expired), and
    /// report whether a held click is due to fire this tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.lockout_s = (self.lockout_s - dt).max(0.0);
        for age in &mut self.unconfirmed {
            *age += dt;
        }
        self.unconfirmed.retain(|age| *age < PREDICTION_EXPIRY_S);
        match self.held_delay_s {
            Some(delay) => {
                let remaining = delay - dt;
                // A tenth of a millisecond of float slack: 0.1 s of reload is six ticks, not seven.
                if remaining <= 1.0e-4 {
                    self.held_delay_s = None;
                    true
                } else {
                    self.held_delay_s = Some(remaining);
                    false
                }
            }
            None => false,
        }
    }

    /// The replicated shots of this snapshot, less the player's own that were already fanned
    /// out from a prediction: each own shot consumes the oldest unconfirmed prediction; own
    /// shots beyond the predictions (a shot the client never predicted) pass through and play.
    pub fn reconcile(
        &mut self,
        fired: Vec<crate::fx::FireEvent>,
        player: TankId,
    ) -> Vec<crate::fx::FireEvent> {
        fired
            .into_iter()
            .filter(|event| {
                if event.tank_id != player || self.unconfirmed.is_empty() {
                    return true;
                }
                self.unconfirmed.remove(0);
                false
            })
            .collect()
    }

    #[cfg(test)]
    pub fn unconfirmed_count(&self) -> usize {
        self.unconfirmed.len()
    }

    #[cfg(test)]
    pub fn held(&self) -> Option<f32> {
        self.held_delay_s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own(index: u64) -> crate::fx::FireEvent {
        crate::fx::FireEvent {
            tank_id: TankId(1),
            turret_yaw_rad: 0.0,
            muzzle: glam::Vec3::new(0.0, 1.8, index as f32),
            direction: glam::Vec3::Z,
            recoil_scale: 1.0,
            deck: glam::Vec3::new(0.0, 1.6, index as f32),
            hull_yaw_rad: 0.0,
        }
    }

    fn other() -> crate::fx::FireEvent {
        crate::fx::FireEvent { tank_id: TankId(7), ..own(0) }
    }

    /// A predicted shot skips exactly one replicated own shot; a stranger's shot always plays.
    #[test]
    fn a_prediction_skips_its_replicated_twin_and_nothing_else() {
        let mut own_shot = OwnShotPrediction::default();
        own_shot.predicted(8.0);
        let kept = own_shot.reconcile(vec![own(0), other()], TankId(1));
        assert_eq!(kept.len(), 1, "the own shot was already played, the stranger's plays");
        assert_eq!(kept[0].tank_id, TankId(7));
        assert_eq!(own_shot.unconfirmed_count(), 0);
        // A second replicated own shot with no prediction behind it plays.
        let kept = own_shot.reconcile(vec![own(1)], TankId(1));
        assert_eq!(kept.len(), 1, "an unpredicted own shot is not swallowed");
    }

    /// A prediction the server never confirms expires and cannot swallow the next real shot.
    #[test]
    fn an_unconfirmed_prediction_expires_instead_of_swallowing_the_next_shot() {
        let mut own_shot = OwnShotPrediction::default();
        own_shot.predicted(8.0);
        for _ in 0..30 {
            own_shot.tick(1.0 / 60.0);
        }
        assert_eq!(own_shot.unconfirmed_count(), 0, "written off after {PREDICTION_EXPIRY_S} s");
        let kept = own_shot.reconcile(vec![own(0)], TankId(1));
        assert_eq!(kept.len(), 1, "the real shot after an expired guess still plays");
    }

    /// A held click fires on the tick the reload's remainder runs out — and only once.
    #[test]
    fn a_held_click_fires_when_the_reload_ends() {
        let mut own_shot = OwnShotPrediction::default();
        own_shot.hold(0.1);
        let mut fired_on = None;
        for tick in 0..12 {
            if own_shot.tick(1.0 / 60.0) {
                fired_on = Some(tick);
                break;
            }
        }
        assert_eq!(fired_on, Some(5), "0.1 s of reload is six ticks: due on the sixth");
        assert!(own_shot.held().is_none(), "consumed");
        assert!(!own_shot.tick(1.0 / 60.0), "and it does not fire again");
    }
}
