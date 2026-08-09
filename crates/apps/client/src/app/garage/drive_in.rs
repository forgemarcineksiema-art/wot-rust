//! The drive-in animation: when the player switches vehicles the new tank rolls onto the
//! turntable from the back doorway, then pivot-turns in place to the hero park pose — the way a
//! tracked vehicle actually parks — its tracks scrolling with the travelled distance and dust
//! kicking up behind it, instead of teleporting into place. Pure state here (position + heading
//! along the entry lane, per-side track scroll, a dust cadence); the render loop reads the pose
//! and emits the dust, and the audio link reads the track speed for the engine bed.
//!
//! Two things this replaces, both audit defects: the hull used to slide down the +Z lane holding
//! its PARK yaw — 71.6° off its direction of travel, a 36-tonne tank strafing sideways — and the
//! old cubic ease-OUT opened at 26 m/s (94 km/h) on the very first frame.

use super::GarageState;

/// The straight roll from the gateway to the turntable centre. Smoothstep: the tank pulls away
/// from 0 m/s, peaks at `1.5 * lane / duration ≈ 10.3 m/s` (≈ 37 km/h — a real tank hurrying,
/// not a shell), and brakes to a stop on the mark. Retuned with the A1 nave: the lane grew from
/// 13 m to ~18 m, and the duration follows it to keep the peak under the hurrying-tank ceiling
/// (`the_first_frame_moves_like_a_tank_not_a_shell`).
const ROLL_DURATION_S: f32 = 2.6;
// Where the tank starts is derived from the GATE, not dialled — `hangar::drive_in_start_z()`
// puts the hull one hull-length inside the ajar gate opening, however long the hall is. The
// old literal here (−13.0) was tuned to a 36 m hall; in the 44 m nave it would have
// materialised the tank five metres into the room with the gate nowhere behind it.
use scene_build::hangar::drive_in_start_z;
/// Average pivot rate for the park turn (smoothstep peaks at 1.5×: ~77°/s — brisk, but a pivot
/// that took the mechanically pure ~2.5 s would make every carousel click drag).
const PIVOT_RATE_RAD_S: f32 = 0.9;
/// Nominal half-gauge for the pivot's opposite track scroll. Presentation only — per-vehicle
/// exactness would demand the hitbox here for centimetres nobody can read off spinning links.
const PIVOT_TRACK_RADIUS_M: f32 = 0.75;
/// Emit one dust puff per this much travelled track.
const DUST_INTERVAL_M: f32 = 0.6;

/// The signed short-way pivot from the travel heading (0, facing +Z down the lane) to the park
/// yaw, and the seconds the turn takes at the pivot rate.
fn pivot_arc() -> (f32, f32) {
    let delta = (scene_build::hangar::HERO_PARK_YAW - 0.0).rem_euclid(std::f32::consts::TAU);
    let delta = if delta > std::f32::consts::PI { delta - std::f32::consts::TAU } else { delta };
    (delta, delta.abs() / PIVOT_RATE_RAD_S)
}

/// What the render loop needs each frame: where the hull is, which way it faces, and how far
/// each track has rolled (the pivot scrolls them in OPPOSITE directions — neutral steer).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) struct DriveInPose {
    pub z: f32,
    pub yaw_rad: f32,
    pub track_left_m: f32,
    pub track_right_m: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DriveIn {
    /// Seconds into the animation (roll then pivot), or `None` once parked.
    elapsed: Option<f32>,
    /// Accumulated per-side track scroll (equal while rolling, opposite while pivoting).
    track_left_m: f32,
    track_right_m: f32,
    /// Previous frame's z and yaw, to accumulate per-frame travel.
    last_z: f32,
    last_yaw: f32,
    /// Travel banked toward the next dust puff.
    dust_since_m: f32,
    /// Fastest track's speed last frame — the engine bed's authority while in the hangar.
    track_speed_mps: f32,
}

impl Default for DriveIn {
    fn default() -> Self {
        Self {
            elapsed: None,
            track_left_m: 0.0,
            track_right_m: 0.0,
            last_z: 0.0,
            last_yaw: scene_build::hangar::HERO_PARK_YAW,
            dust_since_m: 0.0,
            track_speed_mps: 0.0,
        }
    }
}

/// Smoothstep `3t² − 2t³`: starts and ends at zero speed — an engine pulling away and brakes
/// settling, for both the roll and the pivot.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl DriveIn {
    /// The (z, yaw) sample of the animation timeline at `elapsed` seconds.
    fn sample(elapsed: f32) -> (f32, f32) {
        let (pivot_delta, pivot_duration) = pivot_arc();
        if elapsed < ROLL_DURATION_S {
            // Rolling down the lane, facing the way it travels.
            (drive_in_start_z() * (1.0 - smoothstep(elapsed / ROLL_DURATION_S)), 0.0)
        } else {
            // Parked on the mark, pivoting to the hero pose.
            let t = (elapsed - ROLL_DURATION_S) / pivot_duration.max(1.0e-3);
            (0.0, pivot_delta * smoothstep(t))
        }
    }

    fn total_duration() -> f32 {
        ROLL_DURATION_S + pivot_arc().1
    }
}

impl GarageState {
    /// Begin the roll-in for the freshly selected vehicle.
    pub(super) fn start_drive_in(&mut self) {
        self.drive_in = DriveIn {
            elapsed: Some(0.0),
            track_left_m: 0.0,
            track_right_m: 0.0,
            last_z: drive_in_start_z(),
            last_yaw: 0.0,
            dust_since_m: 0.0,
            track_speed_mps: 0.0,
        };
    }

    /// Advance the animation one frame, banking travelled distance for the tracks and dust.
    pub(in crate::app) fn tick_drive_in(&mut self, dt: f32) {
        let Some(e) = self.drive_in.elapsed else {
            return;
        };
        let e = e + dt;
        let (z, yaw) = DriveIn::sample(e);
        // Straight travel scrolls both tracks together; the pivot scrolls them in opposite
        // directions about the nominal half-gauge (which side leads is presentation, not a
        // simulated steer — the lock asserts "opposite", not "which").
        let travelled = (z - self.drive_in.last_z).abs();
        let swept = (yaw - self.drive_in.last_yaw).abs() * PIVOT_TRACK_RADIUS_M;
        self.drive_in.track_left_m += travelled + swept;
        self.drive_in.track_right_m += travelled - swept;
        self.drive_in.dust_since_m += travelled + swept;
        self.drive_in.track_speed_mps = if dt > 0.0 { (travelled + swept) / dt } else { 0.0 };
        self.drive_in.last_z = z;
        self.drive_in.last_yaw = yaw;
        if e >= DriveIn::total_duration() {
            self.drive_in.elapsed = None;
            self.drive_in.track_speed_mps = 0.0;
        } else {
            self.drive_in.elapsed = Some(e);
        }
    }

    /// The parked-vehicle pose along the entry lane; settled = the hero park pose, tracks still.
    pub(in crate::app) fn drive_in_pose(&self) -> DriveInPose {
        let (z, yaw_rad) = match self.drive_in.elapsed {
            Some(e) => DriveIn::sample(e),
            None => (0.0, scene_build::hangar::HERO_PARK_YAW),
        };
        DriveInPose {
            z,
            yaw_rad,
            track_left_m: self.drive_in.track_left_m,
            track_right_m: self.drive_in.track_right_m,
        }
    }

    /// The fastest track's current speed — 0 once parked. The hangar's engine bed follows this,
    /// so the roll-in and the park pivot are audible and the parked hall stays quiet.
    pub(in crate::app) fn drive_in_track_speed_mps(&self) -> f32 {
        self.drive_in.track_speed_mps
    }

    #[cfg(test)]
    pub(in crate::app) fn is_driving_in(&self) -> bool {
        self.drive_in.elapsed.is_some()
    }

    /// Whether a dust puff is due this frame (consumes one interval of banked travel). Only fires
    /// while the tank is still moving.
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
    fn the_tank_rolls_in_from_the_doorway_and_settles_in_the_hero_pose() {
        let mut garage = GarageState::default();
        garage.start_drive_in();

        let pose = garage.drive_in_pose();
        assert!(pose.z < -6.0, "it starts back toward the doorway, got {}", pose.z);
        assert_eq!(pose.track_left_m, 0.0, "no track scroll before it moves");
        assert!(garage.is_driving_in());

        advance(&mut garage, DriveIn::total_duration() + 0.2);

        let pose = garage.drive_in_pose();
        assert!(pose.z.abs() < 1.0e-3, "it parks on the turntable centre, got {}", pose.z);
        assert!(
            (pose.yaw_rad - scene_build::hangar::HERO_PARK_YAW).abs() < 1.0e-3,
            "it settles in the hero park pose, got yaw {}",
            pose.yaw_rad
        );
        assert!(pose.track_left_m > 8.0, "the tracks scrolled the entry distance");
        assert!(!garage.is_driving_in(), "the animation is over");
        assert_eq!(garage.drive_in_track_speed_mps(), 0.0, "parked = engine quiet");
    }

    #[test]
    fn the_hull_faces_its_direction_of_travel_while_rolling_then_pivots_to_park() {
        // THE audit defect: the hull used to hold the park yaw down the whole lane — 71.6° off
        // its direction of travel, a tank strafing sideways. While z still moves, the hull must
        // face the lane (+Z = yaw 0); the park yaw arrives in the pivot, with the hull at rest.
        let mut garage = GarageState::default();
        garage.start_drive_in();

        let mut last_z = garage.drive_in_pose().z;
        for _ in 0..((DriveIn::total_duration() + 0.2) * 60.0) as u32 {
            garage.tick_drive_in(1.0 / 60.0);
            let pose = garage.drive_in_pose();
            if (pose.z - last_z).abs() > 1.0e-4 {
                assert_eq!(
                    pose.yaw_rad, 0.0,
                    "while travelling the lane the hull faces the lane (z {} -> {})",
                    last_z, pose.z
                );
            }
            if pose.yaw_rad != 0.0 {
                assert!(
                    pose.z.abs() < 1.0e-3,
                    "the pivot happens ON the mark, not while sliding (z {})",
                    pose.z
                );
            }
            last_z = pose.z;
        }
    }

    #[test]
    fn the_first_frame_moves_like_a_tank_not_a_shell() {
        // The old cubic ease-out opened at 26 m/s (94 km/h) on frame one. Smoothstep pulls away
        // from zero; the whole animation must stay under a hurrying-tank ceiling.
        let mut garage = GarageState::default();
        garage.start_drive_in();

        let mut max_speed = 0.0f32;
        let mut last_z = garage.drive_in_pose().z;
        for _ in 0..((DriveIn::total_duration() + 0.2) * 60.0) as u32 {
            garage.tick_drive_in(1.0 / 60.0);
            let z = garage.drive_in_pose().z;
            max_speed = max_speed.max((z - last_z).abs() * 60.0);
            last_z = z;
        }
        assert!(max_speed < 11.5, "peak {max_speed:.1} m/s — a tank, not a shell");
        assert!(max_speed > 6.0, "it still actually hurries, got {max_speed:.1} m/s");
    }

    #[test]
    fn the_park_pivot_scrolls_the_tracks_in_opposite_directions() {
        let mut garage = GarageState::default();
        garage.start_drive_in();
        advance(&mut garage, ROLL_DURATION_S + 0.05);
        let before = garage.drive_in_pose();

        advance(&mut garage, 0.5);
        let after = garage.drive_in_pose();

        let left = after.track_left_m - before.track_left_m;
        let right = after.track_right_m - before.track_right_m;
        assert!(left.abs() > 1.0e-3, "the pivot rolls the tracks");
        assert!(
            (left + right).abs() < 1.0e-3,
            "neutral steer: one track forward, the other back (left {left}, right {right})"
        );
        assert!(
            garage.drive_in_track_speed_mps() > 0.1,
            "the pivot is audible: the engine bed reads a moving track"
        );
    }

    #[test]
    fn dust_puffs_pace_with_the_travel_and_stop_once_parked() {
        let mut garage = GarageState::default();
        garage.start_drive_in();

        let mut puffs = 0;
        for _ in 0..(DriveIn::total_duration() * 60.0) as u32 + 20 {
            garage.tick_drive_in(1.0 / 60.0);
            if garage.poll_drive_dust() {
                puffs += 1;
            }
        }
        // One puff per interval of travelled track, DERIVED from the lane the gate actually
        // sets (the old 12..=30 literal was tuned to a 13 m lane and broke the moment the
        // lane followed the hall): the whole roll plus the pivot sweep, ±20% for banking.
        let travel = drive_in_start_z().abs() + pivot_arc().0.abs() * PIVOT_TRACK_RADIUS_M;
        let expected = travel / DUST_INTERVAL_M;
        let (lo, hi) = ((expected * 0.8) as i32, (expected * 1.2) as i32 + 1);
        assert!(
            (lo..=hi).contains(&puffs),
            "expected a paced dust trail of ~{expected:.0} puffs, got {puffs}"
        );
        assert!(!garage.poll_drive_dust(), "no dust once parked");
    }

    #[test]
    fn a_settled_garage_is_not_driving_in() {
        let garage = GarageState::default();
        assert!(!garage.is_driving_in(), "the initial parked view does not animate");
        let pose = garage.drive_in_pose();
        assert_eq!((pose.z, pose.track_left_m), (0.0, 0.0));
        assert_eq!(pose.yaw_rad, scene_build::hangar::HERO_PARK_YAW, "parked = hero pose");
    }
}
