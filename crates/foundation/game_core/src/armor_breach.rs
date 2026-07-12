use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::ArmorZone;

pub const MAX_ARMOR_BREACHES: usize = 12;

/// Pose frame owning a permanent armor opening. Mantlet is distinct because it follows gun pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ArmorFrame {
    #[default]
    Hull,
    Turret,
    Mantlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ArmorMaterial {
    #[default]
    RolledSteel,
    CastSteel,
}

/// Stable semantic surface identity. The high byte is the frame, the low byte the armor zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ArmorSurfaceId(pub u16);

impl ArmorSurfaceId {
    pub const fn new(frame: ArmorFrame, zone: ArmorZone) -> Self {
        Self(((frame as u16) << 8) | zone as u16)
    }
}

/// One real perforation, expressed in the local pose frame of the struck armor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorBreach {
    pub surface: ArmorSurfaceId,
    pub frame: ArmorFrame,
    pub zone: ArmorZone,
    pub material: ArmorMaterial,
    pub entry_local: Vec3,
    pub exit_local: Vec3,
    pub entry_normal_local: Vec3,
    pub exit_normal_local: Vec3,
    pub direction_local: Vec3,
    pub radius_m: f32,
    pub thickness_m: f32,
    pub residual_penetration_mm: f32,
}

impl ArmorBreach {
    /// A projectile body passes only when its complete circular cross-section clears the rim.
    pub fn admits(self, point_local: Vec3, projectile_radius_m: f32) -> bool {
        if self.radius_m <= projectile_radius_m {
            return false;
        }
        let axis = self.direction_local.normalize_or_zero();
        if axis == Vec3::ZERO {
            return false;
        }
        let delta = point_local - self.entry_local;
        let radial = delta - axis * delta.dot(axis);
        radial.length() + projectile_radius_m <= self.radius_m
    }
}

/// Persistent bounded breach state. Nearby hits on the same surface merge deterministically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArmorBreachSet {
    breaches: Vec<ArmorBreach>,
}

impl ArmorBreachSet {
    pub fn breaches(&self) -> &[ArmorBreach] {
        &self.breaches
    }

    pub fn add(&mut self, breach: ArmorBreach) {
        if let Some(existing) = self.breaches.iter_mut().find(|existing| {
            existing.surface == breach.surface
                && existing.entry_local.distance(breach.entry_local)
                    <= (existing.radius_m + breach.radius_m) * 0.65
        }) {
            let old_area = existing.radius_m * existing.radius_m;
            let new_area = breach.radius_m * breach.radius_m;
            let total = old_area + new_area;
            existing.entry_local =
                (existing.entry_local * old_area + breach.entry_local * new_area) / total;
            existing.exit_local =
                (existing.exit_local * old_area + breach.exit_local * new_area) / total;
            existing.radius_m = total.sqrt().min(0.38);
            existing.residual_penetration_mm =
                existing.residual_penetration_mm.max(breach.residual_penetration_mm);
            return;
        }
        if self.breaches.len() == MAX_ARMOR_BREACHES {
            self.breaches.remove(0);
        }
        self.breaches.push(breach);
    }

    pub fn passage_at(
        &self,
        frame: ArmorFrame,
        point_local: Vec3,
        projectile_radius_m: f32,
    ) -> Option<&ArmorBreach> {
        self.breaches
            .iter()
            .find(|breach| breach.frame == frame && breach.admits(point_local, projectile_radius_m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn breach(radius_m: f32, x: f32) -> ArmorBreach {
        ArmorBreach {
            surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::UpperGlacis),
            frame: ArmorFrame::Hull,
            zone: ArmorZone::UpperGlacis,
            material: ArmorMaterial::RolledSteel,
            entry_local: Vec3::new(x, 0.0, 1.0),
            exit_local: Vec3::new(x, 0.0, 0.9),
            entry_normal_local: Vec3::Z,
            exit_normal_local: Vec3::NEG_Z,
            direction_local: Vec3::NEG_Z,
            radius_m,
            thickness_m: 0.1,
            residual_penetration_mm: 80.0,
        }
    }

    #[test]
    fn full_projectile_cross_section_must_clear_the_rim() {
        let b = breach(0.08, 0.0);
        assert!(b.admits(Vec3::new(0.01, 0.0, 1.0), 0.05));
        assert!(!b.admits(Vec3::new(0.04, 0.0, 1.0), 0.05));
        assert!(!b.admits(Vec3::ZERO, 0.09));
    }

    #[test]
    fn set_merges_nearby_channels_and_caps_history() {
        let mut set = ArmorBreachSet::default();
        set.add(breach(0.07, 0.0));
        set.add(breach(0.07, 0.02));
        assert_eq!(set.breaches().len(), 1);
        for i in 1..=MAX_ARMOR_BREACHES + 2 {
            set.add(breach(0.03, i as f32));
        }
        assert_eq!(set.breaches().len(), MAX_ARMOR_BREACHES);
    }
}
