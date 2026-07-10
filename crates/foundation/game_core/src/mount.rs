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
        if let Some(blueprint) = crate::VehicleBlueprint::for_vehicle(kind) {
            return blueprint.mount_frames();
        }
        match kind {
            VehicleKind::PrototypeMedium => Self {
                turret_ring: MountFrame::new(Vec3::new(0.0, 1.28, 0.0)),
                gun_trunnion: MountFrame::new(Vec3::new(0.0, 1.72, 1.00)),
                muzzle: MountFrame::new(Vec3::new(0.0, 1.72, 4.90)),
            },
            VehicleKind::T54_1951 => Self {
                turret_ring: MountFrame::new(Vec3::new(0.0, 1.30, 0.0)),
                gun_trunnion: MountFrame::new(Vec3::new(0.0, 1.80, 1.10)),
                muzzle: MountFrame::new(Vec3::new(0.0, 1.80, 5.20)),
            },
            VehicleKind::T55A => Self {
                turret_ring: MountFrame::new(Vec3::new(0.0, 1.30, 0.05)),
                gun_trunnion: MountFrame::new(Vec3::new(0.0, 1.78, 1.05)),
                muzzle: MountFrame::new(Vec3::new(0.0, 1.78, 5.30)),
            },
            VehicleKind::Jagdtiger => Self {
                turret_ring: MountFrame::new(Vec3::new(0.0, 1.05, 0.0)),
                gun_trunnion: MountFrame::new(Vec3::new(0.0, 2.05, 1.88)),
                muzzle: MountFrame::new(Vec3::new(0.0, 2.05, 7.85)),
            },
            VehicleKind::PantherII => Self {
                turret_ring: MountFrame::new(Vec3::new(0.0, 1.45, -0.05)),
                gun_trunnion: MountFrame::new(Vec3::new(0.0, 2.04, 1.30)),
                muzzle: MountFrame::new(Vec3::new(0.0, 2.04, 6.20)),
            },
            // Blueprint-migrated: mounts come from `blueprint.mount_frames()` above, always.
            VehicleKind::IS3 | VehicleKind::TigerI | VehicleKind::TigerII => {
                unreachable!("{kind:?} is blueprint-migrated")
            }
        }
    }
}

impl Default for MountFrames {
    fn default() -> Self {
        Self::for_vehicle(VehicleKind::PrototypeMedium)
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
