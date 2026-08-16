//! The sniper (gunner's optics) camera, split from `controller.rs` for the reviewability
//! budget. Geometry rules: the eye sits ON the turret-ring axis (no lateral slide during
//! traverse), rides the FULL hull attitude (yaw + authoritative pitch/roll — on a slope the
//! optics are where the tank holds them), and takes the vertical micro-damper's smoothed
//! height so ruts do not slam a 3-degree sight picture 1:1. The aim direction itself stays
//! rigid: damping position jolts is comfort, damping the aim would be lag.
//!
//! The aim is a **world-space sight ray** (`desired_yaw`/`desired_pitch` are world azimuth and
//! world elevation — see `aim::DesiredAim`), so `gun_direction` already gives the correct look
//! on any slope: the sight points where the player aims in the world, and the gun (solved
//! elsewhere, hull-aware) carries the hull tilt. Composing the hull into the *view* here would
//! double-count it and send the sight into the sky on a nose-down hull.

use game_core::MountFrames;
use game_core::math::gun_direction;
use glam::Vec3;
use renderer_api::Camera;

use super::controller::BattleCameraController;
use super::{BattleCameraEnvironment, CameraSubject, collision};

/// Sniper sight height above the gun trunnion: the gunner's telescope, which on this generation
/// of tank is bolted to the gun's own cradle a hand's breadth over the bore (T-54 TSh-2-22,
/// T-34-85 TSh-16, Panther TZF-12a all sit in the same 0.10..0.15 m band).
///
/// It used to be 0.35 m — "roughly where the gunner's optics sit", a guess, never measured. That
/// guess is the whole of the defect reported from the game on 2026-08-07 and is what
/// `hud/reticle/seam_tests.rs` now ratchets. A shell sent 320 m leaves the muzzle about 2 mrad
/// above the line to its target, so height at the origin buys enormous reach along the ground:
/// **35 cm of eye is roughly 175 m of it**. The eye looked over folds the shell flew into,
/// mid-field ground the sight cleared by 20 cm ate the round, and the picture showed nothing —
/// the fold is BELOW the sight line by definition. Measured through the seam test over 30 000
/// placements per map, the share of sight-reachable hulls the gun cannot reach fell from
/// **11.8 to 3.8 per mille on Bystra and 19.9 to 5.2 on Prokhorovka** when this constant came
/// down from that guess to the real optic.
///
/// The trade is deliberate and is the honest one: the eye no longer peeks over a crest the gun
/// cannot shoot past, so hull-down exposure is what the GUN needs, not what the camera wanted.
pub(crate) const SNIPER_SIGHT_ABOVE_TRUNNION_M: f32 = 0.12;

/// The band a gunner's telescope of this era occupies over the bore. The sight height is a
/// *reference* number, not a taste knob, and this is the assertion that says so — the constant
/// above may be tuned inside the band a real optic lives in and nowhere else.
#[cfg(test)]
const OPTIC_BAND_ABOVE_BORE_M: (f32, f32) = (0.08, 0.16);

/// World position of the sniper eye from a base anchor and hull pose: the turret-ring axis at
/// optic height. It rides the ring, NOT the gun, so it is independent of turret traverse. That
/// invariance is why it is also the correct origin for seeding the sniper sight onto the
/// crosshair's world point: seeding from the muzzle skews the opening view toward the barrel line
/// whenever the turret is mid-traverse or reversed (the muzzle is metres off the ring axis then).
pub(crate) fn sniper_eye_from_base(
    vehicle: game_core::VehicleKind,
    base: Vec3,
    hull: game_core::math::HullPose,
) -> Vec3 {
    let mounts = MountFrames::for_vehicle(vehicle);
    let ring = mounts.turret_ring.translation;
    let sight_height = mounts.gun_trunnion.translation.y + SNIPER_SIGHT_ABOVE_TRUNNION_M;
    base + hull.basis() * Vec3::new(ring.x, sight_height, ring.z)
}

impl BattleCameraController {
    pub(super) fn sniper_camera(
        &self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Camera {
        // The smoothed anchor (vertical micro-damper, see `CameraSmoothing::advance`) supplies
        // the base height; x/z snap to the hull so aiming stays rigid.
        let base = self.smoothing.anchor.unwrap_or(subject.position_vec());
        let hull = game_core::math::HullPose {
            yaw_rad: subject.hull_yaw_rad,
            pitch_rad: subject.hull_pitch_rad,
            roll_rad: subject.hull_roll_rad,
        };
        let eye = sniper_eye_from_base(subject.vehicle, base, hull);
        let eye = collision::apply_terrain_clearance(
            eye,
            environment,
            self.settings().terrain_clearance_m,
        );
        let aim = gun_direction(subject.desired_yaw_rad, subject.desired_pitch_rad);
        let target = eye + aim * 1_000.0;

        Camera {
            eye: eye.to_array(),
            target: target.to_array(),
            vertical_fov_degrees: self.sniper_fov_degrees(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{BattleCameraMode, CameraSubject};

    fn subject(hull_pitch_rad: f32, hull_roll_rad: f32, desired_pitch_rad: f32) -> CameraSubject {
        CameraSubject {
            position: [0.0, 0.0, 0.0],
            vehicle: game_core::VehicleKind::T54_1951,
            hull_yaw_rad: 0.0,
            hull_pitch_rad,
            hull_roll_rad,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            view_yaw_rad: 0.0,
            desired_yaw_rad: 0.0,
            desired_pitch_rad,
            sprung_dive_rad: 0.0,
            sprung_heave_m: 0.0,
        }
    }

    /// The sniper eye is the gun's own telescope, so it must sit in the band a real one occupies
    /// over the bore — and, whatever the number, ON the gun axis in height terms rather than a
    /// hand above the deck.
    ///
    /// This is the guard on the constant that caused the 2026-08-07 report: at 0.35 m the eye
    /// looked over ground the shell flew into, and the only thing holding that number in place
    /// was a comment saying "roughly". A sight height is a reference measurement; the reticle's
    /// whole honesty rests on it, and `hud/reticle/seam_tests.rs` prices what raising it costs.
    #[test]
    fn the_sniper_eye_sits_where_a_real_gunners_telescope_does() {
        let (low, high) = OPTIC_BAND_ABOVE_BORE_M;
        assert!(
            (low..=high).contains(&SNIPER_SIGHT_ABOVE_TRUNNION_M),
            "the sniper sight must stand {low}..{high} m over the bore (T-54 TSh-2-22 and its \
             generation), got {SNIPER_SIGHT_ABOVE_TRUNNION_M}"
        );

        // And it is really applied: the eye rides the trunnion height plus that offset, on the
        // ring axis, carried by the hull.
        let mounts = MountFrames::for_vehicle(game_core::VehicleKind::T54_1951);
        let level = game_core::math::HullPose { yaw_rad: 0.0, pitch_rad: 0.0, roll_rad: 0.0 };
        let eye = sniper_eye_from_base(game_core::VehicleKind::T54_1951, Vec3::ZERO, level);
        assert!(
            (eye.y - (mounts.gun_trunnion.translation.y + SNIPER_SIGHT_ABOVE_TRUNNION_M)).abs()
                < 1.0e-6,
            "eye {eye:?} must be the trunnion plus the optic offset"
        );
    }

    /// The sight ray is world-space: the sniper view elevation equals the desired world pitch no
    /// matter how the hull is tilted. This is the regression for "on a slope the sniper flies to
    /// the sky" — the view must not gain the hull's pitch.
    #[test]
    fn sniper_view_elevation_is_world_space_and_ignores_hull_tilt() {
        let mut controller = BattleCameraController::default();
        controller.set_mode(BattleCameraMode::Sniper);
        let env = BattleCameraEnvironment::empty();
        let want_pitch = -0.10_f32;

        let elevation = |subject: &CameraSubject| {
            let cam = controller.render_camera(subject, &env);
            let dir = (Vec3::from_array(cam.target) - Vec3::from_array(cam.eye)).normalize();
            dir.y.asin()
        };

        let level = elevation(&subject(0.0, 0.0, want_pitch));
        let nose_down = elevation(&subject(-0.20, 0.0, want_pitch));
        let rolled = elevation(&subject(0.0, 0.25, want_pitch));

        assert!((level - want_pitch).abs() < 1.0e-4, "flat: {level}");
        assert!(
            (nose_down - want_pitch).abs() < 1.0e-4,
            "nose-down hull must not lift the sight: {nose_down}"
        );
        assert!(
            (rolled - want_pitch).abs() < 1.0e-4,
            "roll must not tilt the sight elevation: {rolled}"
        );
    }
}
