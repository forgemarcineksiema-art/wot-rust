//! The drive-in animation: when the player switches vehicles the new tank rolls onto the
//! turntable from the back doorway over about a second and a half, its tracks scrolling with the
//! travelled distance and dust kicking up behind it, instead of teleporting into place. Pure
//! state here (position along the entry lane + track scroll + a dust cadence); the render loop
//! reads the pose and emits the dust.

use super::GarageState;

const DRIVE_IN_DURATION_S: f32 = 1.5;
/// Where the tank starts, back toward the bay gate (it drives forward to `z = 0`) — the longer
/// hall earns a longer roll-in.
const DRIVE_IN_START_Z: f32 = -13.0;
/// Emit one dust puff per this much travelled track.
const DUST_INTERVAL_M: f32 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DriveIn {
    /// Seconds into the roll-in, or `None` once parked.
    elapsed: Option<f32>,
    /// Accumulated track scroll distance (drives the wheel spin / link scroll).
    track_m: f32,
    /// Previous frame's z, to accumulate the per-frame travel.
    last_z: f32,
    /// Travel banked toward the next dust puff.
    dust_since_m: f32,
}

impl Default for DriveIn {
    fn default() -> Self {
        Self { elapsed: None, track_m: 0.0, last_z: 0.0, dust_since_m: 0.0 }
    }
}

impl DriveIn {
    /// Cubic ease-out z for a normalized time `t` in `[0, 1]`: fast off the mark, settling gently
    /// onto the turntable.
    fn eased_z(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        DRIVE_IN_START_Z * (1.0 - ease)
    }
}

impl GarageState {
    /// Begin the roll-in for the freshly selected vehicle.
    pub(super) fn start_drive_in(&mut self) {
        self.drive_in = DriveIn {
            elapsed: Some(0.0),
            track_m: 0.0,
            last_z: DRIVE_IN_START_Z,
            dust_since_m: 0.0,
        };
    }

    /// Advance the roll-in one frame, banking travelled distance for the tracks and dust.
    pub(in crate::app) fn tick_drive_in(&mut self, dt: f32) {
        let Some(e) = self.drive_in.elapsed else {
            return;
        };
        let e = e + dt;
        let z = DriveIn::eased_z(e / DRIVE_IN_DURATION_S);
        let travelled = (z - self.drive_in.last_z).abs();
        self.drive_in.track_m += travelled;
        self.drive_in.dust_since_m += travelled;
        self.drive_in.last_z = z;
        self.drive_in.elapsed = (e < DRIVE_IN_DURATION_S).then_some(e);
    }

    /// The parked-vehicle z along the entry lane (0 once settled) and the accumulated track scroll.
    pub(in crate::app) fn drive_in_pose(&self) -> (f32, f32) {
        let z = match self.drive_in.elapsed {
            Some(e) => DriveIn::eased_z(e / DRIVE_IN_DURATION_S),
            None => 0.0,
        };
        (z, self.drive_in.track_m)
    }

    #[cfg(test)]
    pub(in crate::app) fn is_driving_in(&self) -> bool {
        self.drive_in.elapsed.is_some()
    }

    /// Whether a dust puff is due this frame (consumes one interval of banked travel). Only fires
    /// while the tank is still rolling.
    pub(in crate::app) fn poll_drive_dust(&mut self) -> bool {
        if self.drive_in.elapsed.is_some() && self.drive_in.dust_since_m >= DUST_INTERVAL_M {
            self.drive_in.dust_since_m -= DUST_INTERVAL_M;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advance(garage: &mut GarageState, seconds: f32) {
        for _ in 0..(seconds * 60.0) as u32 {
            garage.tick_drive_in(1.0 / 60.0);
        }
    }

    #[test]
    fn the_tank_rolls_in_from_the_doorway_and_settles_on_the_turntable() {
        let mut garage = GarageState::default();
        garage.start_drive_in();

        let (z0, track0) = garage.drive_in_pose();
        assert!(z0 < -6.0, "it starts back toward the doorway, got {z0}");
        assert_eq!(track0, 0.0, "no track scroll before it moves");
        assert!(garage.is_driving_in());

        advance(&mut garage, DRIVE_IN_DURATION_S + 0.2);

        let (z1, track1) = garage.drive_in_pose();
        assert!(z1.abs() < 1.0e-3, "it parks on the turntable centre, got {z1}");
        assert!(track1 > 8.0, "the tracks scrolled the full entry distance, got {track1}");
        assert!(!garage.is_driving_in(), "the roll-in is over");
    }

    #[test]
    fn dust_puffs_pace_with_the_travel_and_stop_once_parked() {
        let mut garage = GarageState::default();
        garage.start_drive_in();

        let mut puffs = 0;
        for _ in 0..(DRIVE_IN_DURATION_S * 60.0) as u32 + 20 {
            garage.tick_drive_in(1.0 / 60.0);
            if garage.poll_drive_dust() {
                puffs += 1;
            }
        }
        // ~13 m of travel at one puff per 0.6 m is a healthy trail, not a single blip or a storm.
        assert!((12..=28).contains(&puffs), "expected a paced dust trail, got {puffs}");
        assert!(!garage.poll_drive_dust(), "no dust once parked");
    }

    #[test]
    fn a_settled_garage_is_not_driving_in() {
        let garage = GarageState::default();
        assert!(!garage.is_driving_in(), "the initial parked view does not animate");
        assert_eq!(garage.drive_in_pose(), (0.0, 0.0));
    }
}
