use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::{ArmorZone, ShellType};

pub const MAX_ARMOR_BREACHES: usize = 12;
pub const MAX_APERTURE_LOBES: usize = 4;
pub const MAX_ARMOR_SCARS: usize = 24;
const CLEARANCE_SAMPLES: usize = 24;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum BreachFace {
    #[default]
    Ingress,
    Egress,
}

/// Stable semantic surface identity. The high byte is the frame, the low byte the armor zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ArmorSurfaceId(pub u16);

impl ArmorSurfaceId {
    pub const fn new(frame: ArmorFrame, zone: ArmorZone) -> Self {
        Self(((frame as u16) << 8) | zone as u16)
    }
}

/// Compact, deterministic outline. It is an ellipse perturbed by two seeded low-frequency waves,
/// never a regular black circle and never presentation-only: collision samples this same contour.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BreachContour {
    pub major_radius_m: f32,
    pub minor_radius_m: f32,
    pub rotation_rad: f32,
    pub irregularity: f32,
}

impl BreachContour {
    pub fn new(
        major_radius_m: f32,
        minor_radius_m: f32,
        rotation_rad: f32,
        irregularity: f32,
    ) -> Self {
        Self {
            major_radius_m: major_radius_m.max(0.005),
            minor_radius_m: minor_radius_m.max(0.005),
            rotation_rad,
            irregularity: irregularity.clamp(0.0, 0.22),
        }
    }

    fn contains(self, point: Vec2, seed: u64) -> bool {
        let (sin_r, cos_r) = self.rotation_rad.sin_cos();
        let p = Vec2::new(point.x * cos_r + point.y * sin_r, -point.x * sin_r + point.y * cos_r);
        let angle = p.y.atan2(p.x);
        let phase_a = crate::math::hash_unit(seed) * std::f32::consts::TAU;
        let phase_b = crate::math::hash_unit(seed.rotate_left(29)) * std::f32::consts::TAU;
        let rough = 1.0
            + self.irregularity
                * ((angle * 3.0 + phase_a).sin() * 0.62 + (angle * 5.0 + phase_b).sin() * 0.38);
        let ellipse =
            Vec2::new(p.x / (self.major_radius_m * rough), p.y / (self.minor_radius_m * rough));
        ellipse.length_squared() <= 1.0
    }

    /// Deterministic boundary sample used by rim and remesh geometry. Collision uses the same
    /// waves through `contains`, so the visible steel and reusable clearance agree.
    pub fn point_at(self, angle: f32, seed: u64) -> Vec2 {
        let phase_a = crate::math::hash_unit(seed) * std::f32::consts::TAU;
        let phase_b = crate::math::hash_unit(seed.rotate_left(29)) * std::f32::consts::TAU;
        let rough = 1.0
            + self.irregularity
                * ((angle * 3.0 + phase_a).sin() * 0.62 + (angle * 5.0 + phase_b).sin() * 0.38);
        let unrotated = Vec2::new(
            self.major_radius_m * rough * angle.cos(),
            self.minor_radius_m * rough * angle.sin(),
        );
        let (sin_r, cos_r) = self.rotation_rad.sin_cos();
        Vec2::new(
            unrotated.x * cos_r - unrotated.y * sin_r,
            unrotated.x * sin_r + unrotated.y * cos_r,
        )
    }
}

/// One physical perforation lobe. Nearby shots remain separate lobes so their union does not turn
/// into one dishonest oversized circle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ApertureLobe {
    pub entry_local: Vec3,
    pub exit_local: Vec3,
    pub entry_normal_local: Vec3,
    pub exit_normal_local: Vec3,
    pub direction_local: Vec3,
    pub thickness_m: f32,
    pub outer: BreachContour,
    pub inner: BreachContour,
    pub fracture_seed: u64,
}

impl ApertureLobe {
    pub fn characteristic_radius(self) -> f32 {
        self.outer.major_radius_m.max(self.outer.minor_radius_m)
    }

    fn contains(self, point_local: Vec3) -> bool {
        let (u, v) = surface_basis(self.entry_normal_local, self.direction_local);
        let delta = point_local - self.entry_local;
        self.outer.contains(Vec2::new(delta.dot(u), delta.dot(v)), self.fracture_seed)
    }
}

/// One reusable aperture group on one semantic plate. Its lobes are evaluated as a union.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorBreachDescriptor {
    pub breach_id: u64,
    pub surface: ArmorSurfaceId,
    pub frame: ArmorFrame,
    pub zone: ArmorZone,
    pub material: ArmorMaterial,
    pub face: BreachFace,
    pub shell_type: ShellType,
    pub created_tick: u64,
    pub impact_angle_degrees: f32,
    pub impact_energy_kj: f32,
    pub projectile_diameter_m: f32,
    pub residual_penetration_mm: f32,
}

/// One reusable aperture group on one semantic plate. Its lobes are evaluated as a union.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmorBreach {
    pub breach_id: u64,
    pub surface: ArmorSurfaceId,
    pub frame: ArmorFrame,
    pub zone: ArmorZone,
    pub material: ArmorMaterial,
    pub face: BreachFace,
    pub shell_type: ShellType,
    pub created_tick: u64,
    pub impact_angle_degrees: f32,
    pub impact_energy_kj: f32,
    pub projectile_diameter_m: f32,
    pub residual_penetration_mm: f32,
    lobes: Vec<ApertureLobe>,
}

impl ArmorBreach {
    pub fn new(descriptor: ArmorBreachDescriptor, lobe: ApertureLobe) -> Self {
        Self {
            breach_id: descriptor.breach_id,
            surface: descriptor.surface,
            frame: descriptor.frame,
            zone: descriptor.zone,
            material: descriptor.material,
            face: descriptor.face,
            shell_type: descriptor.shell_type,
            created_tick: descriptor.created_tick,
            impact_angle_degrees: descriptor.impact_angle_degrees,
            impact_energy_kj: descriptor.impact_energy_kj,
            projectile_diameter_m: descriptor.projectile_diameter_m,
            residual_penetration_mm: descriptor.residual_penetration_mm,
            lobes: vec![lobe],
        }
    }

    pub fn lobes(&self) -> &[ApertureLobe] {
        &self.lobes
    }

    pub fn reshape_as_egress(&mut self) {
        self.face = BreachFace::Egress;
        for lobe in &mut self.lobes {
            let entry_major = lobe.outer.major_radius_m;
            let entry_minor = lobe.outer.minor_radius_m;
            lobe.outer.major_radius_m = (entry_major * 1.55).min(0.42);
            lobe.outer.minor_radius_m = (entry_minor * 1.45).min(0.42);
            lobe.outer.irregularity = (lobe.outer.irregularity + 0.05).min(0.22);
            lobe.inner.major_radius_m = entry_major * 0.9;
            lobe.inner.minor_radius_m = entry_minor * 0.85;
        }
    }

    /// A projectile passes only when its complete circular cross-section lies inside the union.
    pub fn admits(&self, point_local: Vec3, projectile_radius_m: f32) -> bool {
        let Some(reference) = self.lobes.first().copied() else {
            return false;
        };
        if !point_inside_union(&self.lobes, point_local) {
            return false;
        }
        let (u, v) = surface_basis(reference.entry_normal_local, reference.direction_local);
        (0..CLEARANCE_SAMPLES).all(|sample| {
            let angle = sample as f32 / CLEARANCE_SAMPLES as f32 * std::f32::consts::TAU;
            let edge = point_local
                + u * (projectile_radius_m * angle.cos())
                + v * (projectile_radius_m * angle.sin());
            point_inside_union(&self.lobes, edge)
        })
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.surface == other.surface
            && self.lobes.iter().any(|a| {
                other.lobes.iter().any(|b| {
                    a.entry_local.distance(b.entry_local)
                        <= (a.characteristic_radius() + b.characteristic_radius()) * 0.82
                })
            })
    }

    fn merge(&mut self, mut other: Self) {
        self.residual_penetration_mm =
            self.residual_penetration_mm.max(other.residual_penetration_mm);
        self.impact_energy_kj = self.impact_energy_kj.max(other.impact_energy_kj);
        for lobe in other.lobes.drain(..) {
            if self.lobes.len() < MAX_APERTURE_LOBES {
                self.lobes.push(lobe);
            } else {
                merge_into_nearest_lobe(&mut self.lobes, lobe);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorScar {
    pub surface: ArmorSurfaceId,
    pub frame: ArmorFrame,
    pub local_position: Vec3,
    pub radius_m: f32,
    pub shell_type: ShellType,
    pub created_tick: u64,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorBreachAdd {
    Inserted,
    Merged,
    ScarOnly,
}

/// Persistent bounded damage state. Capacity never heals old steel: overflow becomes a scar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArmorBreachSet {
    breaches: Vec<ArmorBreach>,
    scars: Vec<ArmorScar>,
}

impl ArmorBreachSet {
    pub fn breaches(&self) -> &[ArmorBreach] {
        &self.breaches
    }

    pub fn scars(&self) -> &[ArmorScar] {
        &self.scars
    }

    pub fn add(&mut self, breach: ArmorBreach) -> ArmorBreachAdd {
        if let Some(existing) = self.breaches.iter_mut().find(|existing| existing.overlaps(&breach))
        {
            existing.merge(breach);
            return ArmorBreachAdd::Merged;
        }
        if self.breaches.len() < MAX_ARMOR_BREACHES {
            self.breaches.push(breach);
            return ArmorBreachAdd::Inserted;
        }
        let lobe = breach.lobes.first().copied();
        if let Some(lobe) = lobe {
            if self.scars.len() == MAX_ARMOR_SCARS {
                self.scars.remove(0);
            }
            self.scars.push(ArmorScar {
                surface: breach.surface,
                frame: breach.frame,
                local_position: lobe.entry_local,
                radius_m: lobe.characteristic_radius(),
                shell_type: breach.shell_type,
                created_tick: breach.created_tick,
                seed: lobe.fracture_seed,
            });
        }
        ArmorBreachAdd::ScarOnly
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

fn point_inside_union(lobes: &[ApertureLobe], point: Vec3) -> bool {
    lobes.iter().copied().any(|lobe| lobe.contains(point))
}

fn merge_into_nearest_lobe(lobes: &mut [ApertureLobe], incoming: ApertureLobe) {
    let Some((index, _)) = lobes.iter().enumerate().min_by(|(_, a), (_, b)| {
        a.entry_local
            .distance_squared(incoming.entry_local)
            .total_cmp(&b.entry_local.distance_squared(incoming.entry_local))
    }) else {
        return;
    };
    let target = &mut lobes[index];
    let distance = target.entry_local.distance(incoming.entry_local);
    target.outer.major_radius_m =
        target.outer.major_radius_m.max(distance + incoming.outer.major_radius_m).min(0.38);
    target.outer.minor_radius_m =
        target.outer.minor_radius_m.max(incoming.outer.minor_radius_m).min(0.38);
    target.inner.major_radius_m = target.inner.major_radius_m.max(incoming.inner.major_radius_m);
    target.inner.minor_radius_m = target.inner.minor_radius_m.max(incoming.inner.minor_radius_m);
}

pub fn surface_basis(normal: Vec3, direction: Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize_or_zero();
    let projected = (direction - n * direction.dot(n)).normalize_or_zero();
    let mut u = if projected != Vec3::ZERO { projected } else { n.cross(Vec3::Y) };
    if u.length_squared() < 1.0e-6 {
        u = n.cross(Vec3::X);
    }
    u = u.normalize_or_zero();
    (u, n.cross(u).normalize_or_zero())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn breach(x: f32, radius: f32, id: u64) -> ArmorBreach {
        let lobe = ApertureLobe {
            entry_local: Vec3::new(x, 0.0, 1.0),
            exit_local: Vec3::new(x, 0.0, 0.9),
            entry_normal_local: Vec3::Z,
            exit_normal_local: Vec3::NEG_Z,
            direction_local: Vec3::NEG_Z,
            thickness_m: 0.1,
            outer: BreachContour::new(radius, radius * 0.92, 0.0, 0.04),
            inner: BreachContour::new(radius * 1.35, radius * 1.2, 0.0, 0.08),
            fracture_seed: id,
        };
        ArmorBreach::new(
            ArmorBreachDescriptor {
                breach_id: id,
                surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::UpperGlacis),
                frame: ArmorFrame::Hull,
                zone: ArmorZone::UpperGlacis,
                material: ArmorMaterial::RolledSteel,
                face: BreachFace::Ingress,
                shell_type: ShellType::ArmorPiercing,
                created_tick: 10,
                impact_angle_degrees: 0.0,
                impact_energy_kj: 800.0,
                projectile_diameter_m: radius * 2.0,
                residual_penetration_mm: 80.0,
            },
            lobe,
        )
    }

    #[test]
    fn full_projectile_cross_section_must_clear_irregular_contour() {
        let b = breach(0.0, 0.08, 7);
        assert!(b.admits(Vec3::new(0.005, 0.0, 1.0), 0.04));
        assert!(!b.admits(Vec3::new(0.05, 0.0, 1.0), 0.04));
        assert!(!b.admits(Vec3::ZERO, 0.09));
    }

    #[test]
    fn overlapping_hits_form_a_union_instead_of_an_area_circle() {
        let mut set = ArmorBreachSet::default();
        assert_eq!(set.add(breach(-0.045, 0.07, 1)), ArmorBreachAdd::Inserted);
        assert_eq!(set.add(breach(0.045, 0.07, 2)), ArmorBreachAdd::Merged);
        assert_eq!(set.breaches().len(), 1);
        assert_eq!(set.breaches()[0].lobes().len(), 2);
        assert!(set.passage_at(ArmorFrame::Hull, Vec3::new(-0.045, 0.0, 1.0), 0.02).is_some());
        assert!(set.passage_at(ArmorFrame::Hull, Vec3::new(0.045, 0.0, 1.0), 0.02).is_some());
    }

    #[test]
    fn thirteenth_distant_hit_never_closes_the_oldest_opening() {
        let mut set = ArmorBreachSet::default();
        for i in 0..MAX_ARMOR_BREACHES {
            assert_eq!(set.add(breach(i as f32, 0.04, i as u64)), ArmorBreachAdd::Inserted);
        }
        let oldest = set.breaches()[0].breach_id;
        assert_eq!(set.add(breach(50.0, 0.04, 99)), ArmorBreachAdd::ScarOnly);
        assert_eq!(set.breaches().len(), MAX_ARMOR_BREACHES);
        assert_eq!(set.breaches()[0].breach_id, oldest);
        assert_eq!(set.scars().len(), 1);
    }
}
