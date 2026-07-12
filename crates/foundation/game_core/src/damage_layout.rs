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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleIntersection {
    pub slot: ModuleSlot,
    pub distance_t: f32,
    pub path_length_m: f32,
    pub priority: u8,
}

impl ModuleVolume {
    pub fn contains(self, point: Vec3) -> bool {
        const BOUNDARY_EPSILON_M: f32 = 1.0e-4;
        let epsilon = Vec3::splat(BOUNDARY_EPSILON_M);
        point.cmpge(self.min - epsilon).all() && point.cmple(self.max + epsilon).all()
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
                    min: Vec3::new(-0.46, 0.42, 0.48),
                    max: Vec3::new(0.46, 1.08, 1.28),
                    priority: 30,
                    requires_penetration: true,
                },
                ModuleVolume {
                    slot: ModuleSlot::AmmoRack,
                    min: Vec3::new(-0.92, -0.48, -0.92),
                    max: Vec3::new(0.92, 0.58, 0.08),
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
                    slot: ModuleSlot::Radio,
                    min: Vec3::new(-0.92, -0.15, 0.35),
                    max: Vec3::new(-0.25, 0.58, 1.15),
                    priority: 18,
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

    /// Intersect a complete through-flight with every authored module, nearest first.
    pub fn intersections(
        &self,
        penetrated: bool,
        start: Vec3,
        end: Vec3,
    ) -> Vec<ModuleIntersection> {
        let delta = end - start;
        let length = delta.length();
        let mut hits: Vec<_> = self
            .volumes
            .iter()
            .filter(|volume| !volume.requires_penetration || penetrated)
            .filter_map(|volume| {
                segment_aabb_interval(start, end, volume.min, volume.max).map(|(enter, exit)| {
                    ModuleIntersection {
                        slot: volume.slot,
                        distance_t: enter,
                        path_length_m: (exit - enter).max(0.0) * length,
                        priority: volume.priority,
                    }
                })
            })
            .collect();
        hits.sort_by(|a, b| {
            a.distance_t.total_cmp(&b.distance_t).then_with(|| b.priority.cmp(&a.priority))
        });
        hits.dedup_by_key(|hit| hit.slot);
        hits
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

fn segment_aabb_interval(start: Vec3, end: Vec3, min: Vec3, max: Vec3) -> Option<(f32, f32)> {
    let delta = end - start;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for axis in 0..3 {
        let origin = start[axis];
        let direction = delta[axis];
        if direction.abs() < 1.0e-7 {
            if origin < min[axis] || origin > max[axis] {
                return None;
            }
            continue;
        }
        let a = (min[axis] - origin) / direction;
        let b = (max[axis] - origin) / direction;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return None;
        }
    }
    Some((enter, exit))
}
