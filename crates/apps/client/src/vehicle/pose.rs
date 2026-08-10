//! One source of truth for posing baked vehicle submeshes in the world.
//!
//! Both render paths — the dynamic per-vertex mesh build (`vehicle_geometry_mesh`) and the cached
//! instanced objects (`vehicle_render_objects`) — must agree exactly on where the hull, turret,
//! and gun sit for a snapshot, or the paths drift apart silently. This module owns that chain:
//! the hull yaws about its origin, the turret yaws about the turret-ring frame (casemates hold
//! yaw at zero), and the gun pitches about the trunnion while riding the turret.
//!
//! Translations follow the mount-frame *positions*; the mount-frame *bases* (identity today)
//! fold into part orientation only, matching how the instanced path has always applied them.

use game_core::math::rotate_around;
use game_core::{MountFrames, VehicleKind};
use glam::{Mat3, Vec3};
use net::TankSnapshot;

/// The static settle a parked vehicle's own mass earns (Hala 3.0 J1): the hull sits this much
/// lower than the neutral authoring pose, every road wheel carries it as spring compression,
/// and the belt's contact band stays flat on the deck. Derived from the stock loadout's real
/// mass over the fleet's band — 26 t settles the documented 2 cm, 70 t the documented 4 cm —
/// so a Jagdtiger visibly SITS where a T-54 rests. Presentation only; the battle's live
/// suspension and every gameplay surface are untouched.
pub(crate) fn rest_settle_m(kind: game_core::VehicleKind) -> f32 {
    let mass_t = kind.default_loadout().total_mass_kg() / 1000.0;
    let t = ((mass_t - 26.0) / (70.0 - 26.0)).clamp(0.0, 1.0);
    0.02 + 0.02 * t
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct VehiclePose {
    ground: Vec3,
    hull_rotation: Mat3,
    turret_rotation: Mat3,
    gun_pitch: Mat3,
    turret_ring: Vec3,
    trunnion: Vec3,
    turret_mount_basis: Mat3,
    gun_mount_basis: Mat3,
}

impl VehiclePose {
    pub fn from_snapshot(snapshot: &TankSnapshot) -> Self {
        Self::new_with_attitude(
            snapshot.vehicle,
            Vec3::from_array(snapshot.position),
            snapshot.yaw_rad,
            snapshot.turret_yaw_rad,
            snapshot.gun_pitch_rad,
            [snapshot.hull_pitch_rad, snapshot.hull_roll_rad, 0.0],
        )
    }

    #[cfg(test)]
    pub fn new(
        kind: VehicleKind,
        ground: Vec3,
        yaw_rad: f32,
        turret_yaw_rad: f32,
        gun_pitch_rad: f32,
    ) -> Self {
        Self::new_with_attitude(kind, ground, yaw_rad, turret_yaw_rad, gun_pitch_rad, [0.0; 3])
    }

    /// As [`Self::new`], with the sprung-hull attitude `[pitch (+nose up), roll (+right side up),
    /// heave]` folded into the hull frame — the turret, gun and running gear all ride it, since
    /// every other transform chains off the hull basis.
    pub fn new_with_attitude(
        kind: VehicleKind,
        ground: Vec3,
        yaw_rad: f32,
        turret_yaw_rad: f32,
        gun_pitch_rad: f32,
        attitude: [f32; 3],
    ) -> Self {
        let [pitch, roll, heave] = attitude;
        let mounts = MountFrames::for_vehicle(kind);
        // glam's Rx(+t) tips local +Z downward, so nose-up pitch enters negated; Rz(+t) lifts
        // local +X, matching roll's "+right side up" convention directly.
        let hull_rotation = Mat3::from_rotation_y(yaw_rad)
            * Mat3::from_rotation_x(-pitch)
            * Mat3::from_rotation_z(roll);
        Self {
            ground: ground + Vec3::Y * heave,
            hull_rotation,
            turret_rotation: Mat3::from_rotation_y(kind.effective_turret_yaw_rad(turret_yaw_rad)),
            gun_pitch: Mat3::from_rotation_x(-gun_pitch_rad),
            turret_ring: mounts.turret_ring.translation,
            trunnion: mounts.gun_trunnion.translation,
            turret_mount_basis: mounts.turret_ring.basis,
            gun_mount_basis: mounts.gun_trunnion.basis,
        }
    }

    /// World position of the hull pivot (the local-space origin on the ground plane).
    pub fn hull_translation(&self) -> Vec3 {
        self.ground
    }

    pub fn hull_basis(&self) -> Mat3 {
        self.hull_rotation
    }

    /// World position of the turret-ring pivot.
    pub fn turret_translation(&self) -> Vec3 {
        self.ground + self.hull_rotation * self.turret_ring
    }

    pub fn turret_basis(&self) -> Mat3 {
        self.hull_rotation * self.turret_rotation * self.turret_mount_basis
    }

    /// World position of the trunnion pivot, carried around the ring by turret traverse.
    pub fn gun_translation(&self) -> Vec3 {
        self.ground
            + self.hull_rotation
                * rotate_around(self.trunnion, self.turret_ring, self.turret_rotation)
    }

    pub fn gun_basis(&self) -> Mat3 {
        self.hull_rotation * self.turret_rotation * self.gun_mount_basis * self.gun_pitch
    }

    /// Map a hull-submesh point from authoring space to the world.
    pub fn hull_point(&self, point: Vec3) -> Vec3 {
        self.hull_translation() + self.hull_basis() * point
    }

    /// Map a turret-submesh point (authored around the ring) to the world.
    pub fn turret_point(&self, point: Vec3) -> Vec3 {
        self.turret_translation() + self.turret_basis() * (point - self.turret_ring)
    }

    /// Map a gun-submesh point (authored around the trunnion) to the world.
    pub fn gun_point(&self, point: Vec3) -> Vec3 {
        self.gun_translation() + self.gun_basis() * (point - self.trunnion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pivot chain must compose: a point on a pivot is carried rigidly by the stages above it.
    #[test]
    fn pivots_ride_their_parent_stages() {
        let pose =
            VehiclePose::new(VehicleKind::T54_1951, Vec3::new(4.0, 0.0, -2.0), 0.7, 0.4, 0.12);
        let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);

        // The ring point itself is unaffected by turret traverse.
        let ring_world = pose.turret_point(mounts.turret_ring.translation);
        assert!((ring_world - pose.hull_point(mounts.turret_ring.translation)).length() < 1.0e-5);

        // The trunnion point is unaffected by gun pitch but rides the turret.
        let trunnion_world = pose.gun_point(mounts.gun_trunnion.translation);
        assert!(
            (trunnion_world - pose.turret_point(mounts.gun_trunnion.translation)).length() < 1.0e-5
        );
    }

    /// Casemate vehicles hold turret yaw at zero, whatever the snapshot says.
    #[test]
    fn casemate_pose_ignores_turret_yaw() {
        let traversed = VehiclePose::new(VehicleKind::Jagdtiger, Vec3::ZERO, 0.3, 1.2, 0.05);
        let held = VehiclePose::new(VehicleKind::Jagdtiger, Vec3::ZERO, 0.3, 0.0, 0.05);
        let probe = Vec3::new(0.6, 2.0, 1.4);
        assert!((traversed.turret_point(probe) - held.turret_point(probe)).length() < 1.0e-6);
        assert!((traversed.gun_point(probe) - held.gun_point(probe)).length() < 1.0e-6);
    }
}
