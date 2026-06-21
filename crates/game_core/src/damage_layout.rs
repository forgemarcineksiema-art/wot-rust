use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{HitboxProfile, ModuleSlot, VehicleKind};

/// A deterministic, vehicle-authored module volume in tank-local coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModuleVolume {
    pub slot: ModuleSlot,
    pub min: Vec3,
    pub max: Vec3,
    pub priority: u8,
    pub requires_penetration: bool,
}

impl ModuleVolume {
    pub fn contains(self, point: Vec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }
}

/// Gameplay-owned layout mapping local hit positions to installed vehicle modules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DamageLayout {
    volumes: Vec<ModuleVolume>,
}

impl DamageLayout {
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        match kind {
            VehicleKind::T54_1951 => Self::t54_1951(),
            _ => Self::default(),
        }
    }

    pub fn t54_1951() -> Self {
        Self {
            volumes: vec![
                ModuleVolume {
                    slot: ModuleSlot::Gun,
                    min: Vec3::new(-0.46, 0.48, 0.70),
                    max: Vec3::new(0.46, 1.10, 1.38),
                    priority: 30,
                    requires_penetration: true,
                },
                ModuleVolume {
                    slot: ModuleSlot::AmmoRack,
                    min: Vec3::new(-0.78, 0.18, -0.88),
                    max: Vec3::new(0.78, 1.02, -0.18),
                    priority: 20,
                    requires_penetration: true,
                },
                ModuleVolume {
                    slot: ModuleSlot::Engine,
                    min: Vec3::new(-1.22, -0.55, -2.60),
                    max: Vec3::new(1.22, 0.64, -1.18),
                    priority: 20,
                    requires_penetration: true,
                },
                ModuleVolume {
                    slot: ModuleSlot::Suspension,
                    min: Vec3::new(-1.75, -1.16, -3.15),
                    max: Vec3::new(1.75, -0.38, 3.15),
                    priority: 10,
                    requires_penetration: false,
                },
                ModuleVolume {
                    slot: ModuleSlot::Turret,
                    min: Vec3::new(-1.00, 0.65, -1.01),
                    max: Vec3::new(1.00, 1.10, 1.09),
                    priority: 10,
                    requires_penetration: true,
                },
            ],
        }
    }

    pub fn volumes(&self) -> &[ModuleVolume] {
        &self.volumes
    }

    pub fn is_empty(&self) -> bool {
        self.volumes.is_empty()
    }

    pub fn impacted_module(&self, penetrated: bool, local_hit: Vec3) -> Option<ModuleSlot> {
        self.volumes
            .iter()
            .filter(|volume| {
                (!volume.requires_penetration || penetrated) && volume.contains(local_hit)
            })
            .max_by_key(|volume| volume.priority)
            .map(|volume| volume.slot)
    }

    pub fn fits_within(&self, hitbox: HitboxProfile) -> bool {
        self.volumes.iter().all(|volume| {
            [volume.min, volume.max].into_iter().all(|p| {
                p.x.abs() <= hitbox.half_width_m
                    && p.z.abs() <= hitbox.half_length_m
                    && p.y.abs() <= hitbox.half_height_m
            })
        })
    }
}
