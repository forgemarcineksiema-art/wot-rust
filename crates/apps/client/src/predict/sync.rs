use glam::Vec3;
use net::TankSnapshot;
use sim::AimingState;

use super::LocalPredictor;

const MAX_SMOOTH_AUTHORITATIVE_CORRECTION_M: f32 = 2.0;

impl LocalPredictor {
    /// Snap the predicted hull to an authoritative snapshot. In lockstep this matches the
    /// prediction, but it also seeds the first frame and corrects rare divergence.
    pub fn sync_to(&mut self, authoritative: &TankSnapshot) {
        self.sync_inner(authoritative, None, false);
    }

    pub fn sync_to_with_motion(
        &mut self,
        authoritative: &TankSnapshot,
        motion: net::AuthoritativeMotion,
    ) {
        self.sync_inner(authoritative, Some(motion), false);
    }

    pub fn snap_to_with_motion(
        &mut self,
        authoritative: &TankSnapshot,
        motion: net::AuthoritativeMotion,
    ) {
        self.sync_inner(authoritative, Some(motion), true);
    }

    fn sync_inner(
        &mut self,
        authoritative: &TankSnapshot,
        motion: Option<net::AuthoritativeMotion>,
        force_anchor: bool,
    ) {
        let was_seeded = self.seeded;
        let correction_m =
            (self.drive.kinematic.position - Vec3::from_array(authoritative.position)).length();
        if self.spec.kind != authoritative.vehicle {
            self.spec = authoritative.vehicle.spec();
        }
        self.drive.kinematic.position = Vec3::from_array(authoritative.position);
        self.drive.kinematic.yaw_rad = authoritative.yaw_rad;
        self.drive.kinematic.pitch_rad = authoritative.hull_pitch_rad;
        self.drive.kinematic.roll_rad = authoritative.hull_roll_rad;
        self.drive.aiming = AimingState {
            turret_yaw_rad: self.spec.effective_turret_yaw_rad(authoritative.turret_yaw_rad),
            turret_yaw_velocity_rad_s: authoritative.turret_yaw_velocity_rad_s,
            gun_pitch_rad: authoritative.gun_pitch_rad,
        };
        self.drive.aim_dispersion_mrad = authoritative.aim_dispersion_mrad;
        self.hit_points = authoritative.hit_points;
        self.module_hit_points = authoritative.module_hit_points;
        self.destroyed_modules_mask = authoritative.destroyed_modules_mask;
        self.tracks = game_core::TrackHealth::from_hp_pair(authoritative.track_hp);
        self.selected_ammo = authoritative.selected_ammo;
        if let Some(motion) = motion {
            self.drive.kinematic.velocity = Vec3::from_array(motion.velocity_mps);
            self.drive.kinematic.yaw_rate_rad_s = motion.hull_yaw_velocity_rad_s;
        }

        if force_anchor || !was_seeded || correction_m > MAX_SMOOTH_AUTHORITATIVE_CORRECTION_M {
            self.previous = self.current_pose();
            // The motion history snaps with the pose: blending speed across a teleport-sized
            // correction would fake a violent acceleration into the attitude cue.
            self.previous_forward_speed_mps = self.drive.kinematic.forward_speed();
            self.tick_accel_long_mps2 = 0.0;
        }
        self.seeded = true;
    }
}
