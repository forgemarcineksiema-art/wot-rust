use glam::{Mat3, Vec3};
use serde::{Deserialize, Serialize};

use crate::VehicleKind;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MountFrame {
    pub translation: Vec3,
    pub basis: Mat3,
}

impl MountFrame {
    pub const fn new(translation: Vec3) -> Self {
        Self { translation, basis: Mat3::IDENTITY }
    }

    pub const fn with_basis(translation: Vec3, basis: Mat3) -> Self {
        Self { translation, basis }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MountFrames {
    pub turret_ring: MountFrame,
    pub gun_trunnion: MountFrame,
    pub muzzle: MountFrame,
}

impl MountFrames {
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        // Migrated vehicles derive their mounts from the blueprint; the rest use the constants below.
        crate::VehicleBlueprint::for_vehicle(kind)
            .expect("every VehicleKind is blueprint-migrated")
            .mount_frames()
    }
}

impl Default for MountFrames {
    fn default() -> Self {
        Self::for_vehicle(VehicleKind::BENCHMARK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vehicle_has_finite_mount_frames() {
        for kind in &VehicleKind::ALL {
            let mounts = MountFrames::for_vehicle(*kind);
            assert!(mounts.turret_ring.translation.is_finite());
            assert!(mounts.turret_ring.basis.determinant().is_finite());
            assert!(mounts.gun_trunnion.translation.is_finite());
            assert!(mounts.muzzle.translation.is_finite());
        }
    }

    #[test]
    fn muzzle_z_exceeds_trunnion_z_for_every_vehicle() {
        for kind in &VehicleKind::ALL {
            let mounts = MountFrames::for_vehicle(*kind);
            assert!(
                mounts.muzzle.translation.z > mounts.gun_trunnion.translation.z + 2.5,
                "{kind:?} barrel too stubby"
            );
        }
    }
}
