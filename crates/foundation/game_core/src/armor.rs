use serde::{Deserialize, Serialize};

use crate::{ShellSpec, ShellType};

mod facet;
mod zone;

pub use facet::{ArmorFacet, ArmorFacetProfile};
pub use zone::{ArmorZone, resolve_penetration_at_distance_on_zone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ArmorFacing {
    #[default]
    HullFront,
    HullSide,
    HullRear,
    TurretFront,
    TurretSide,
    TurretRear,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArmorProfile {
    pub hull_front_mm: f32,
    pub hull_side_mm: f32,
    pub hull_rear_mm: f32,
    pub turret_front_mm: f32,
    pub turret_side_mm: f32,
    pub turret_rear_mm: f32,
    #[serde(default)]
    pub facets: ArmorFacetProfile,
}

impl ArmorProfile {
    pub fn new(
        hull_front_mm: f32,
        hull_side_mm: f32,
        hull_rear_mm: f32,
        turret_front_mm: f32,
        turret_side_mm: f32,
        turret_rear_mm: f32,
    ) -> Self {
        Self::new_with_facets(
            ArmorFacet::new(hull_front_mm, 0.0, 1.0),
            ArmorFacet::new(hull_side_mm, 0.0, 1.0),
            ArmorFacet::new(hull_rear_mm, 0.0, 1.0),
            ArmorFacet::new(turret_front_mm, 0.0, 1.0),
            ArmorFacet::new(turret_side_mm, 0.0, 1.0),
            ArmorFacet::new(turret_rear_mm, 0.0, 1.0),
        )
    }

    pub fn new_with_facets(
        hull_front: ArmorFacet,
        hull_side: ArmorFacet,
        hull_rear: ArmorFacet,
        turret_front: ArmorFacet,
        turret_side: ArmorFacet,
        turret_rear: ArmorFacet,
    ) -> Self {
        Self {
            hull_front_mm: hull_front.nominal_thickness_mm,
            hull_side_mm: hull_side.nominal_thickness_mm,
            hull_rear_mm: hull_rear.nominal_thickness_mm,
            turret_front_mm: turret_front.nominal_thickness_mm,
            turret_side_mm: turret_side.nominal_thickness_mm,
            turret_rear_mm: turret_rear.nominal_thickness_mm,
            facets: ArmorFacetProfile {
                hull_front,
                hull_side,
                hull_rear,
                turret_front,
                turret_side,
                turret_rear,
            },
        }
    }

    pub fn nominal_thickness_mm(&self, facing: ArmorFacing) -> f32 {
        match facing {
            ArmorFacing::HullFront => self.hull_front_mm,
            ArmorFacing::HullSide => self.hull_side_mm,
            ArmorFacing::HullRear => self.hull_rear_mm,
            ArmorFacing::TurretFront => self.turret_front_mm,
            ArmorFacing::TurretSide => self.turret_side_mm,
            ArmorFacing::TurretRear => self.turret_rear_mm,
        }
    }

    pub fn facet(&self, facing: ArmorFacing) -> ArmorFacet {
        let facet = match facing {
            ArmorFacing::HullFront => self.facets.hull_front,
            ArmorFacing::HullSide => self.facets.hull_side,
            ArmorFacing::HullRear => self.facets.hull_rear,
            ArmorFacing::TurretFront => self.facets.turret_front,
            ArmorFacing::TurretSide => self.facets.turret_side,
            ArmorFacing::TurretRear => self.facets.turret_rear,
        };
        if facet.nominal_thickness_mm > 0.0 {
            facet
        } else {
            ArmorFacet::new(self.nominal_thickness_mm(facing), 0.0, 1.0)
        }
    }

    pub fn effective_thickness_mm(&self, facing: ArmorFacing, impact_angle_degrees: f32) -> f32 {
        effective_facet_thickness_mm(self.facet(facing), impact_angle_degrees)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PenetrationResult {
    pub penetrated: bool,
    pub ricocheted: bool,
    pub effective_armor_mm: f32,
    pub remaining_penetration_mm: f32,
    pub damage_hp: u32,
    pub module_damage_hp: u32,
}

pub fn resolve_penetration(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    facing: ArmorFacing,
    impact_angle_degrees: f32,
) -> PenetrationResult {
    resolve_penetration_at_distance(shell, armor, facing, impact_angle_degrees, 100.0)
}

pub fn resolve_penetration_at_distance(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    facing: ArmorFacing,
    impact_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    let facet = armor.facet(facing);
    resolve_penetration_at_distance_on_facet(shell, facet, impact_angle_degrees, distance_m)
}

pub(crate) fn resolve_penetration_at_distance_on_facet(
    shell: &ShellSpec,
    facet: ArmorFacet,
    impact_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    let normalized_angle = normalized_impact_angle(shell, impact_angle_degrees);
    let ricocheted = ricochets(shell, &facet, impact_angle_degrees);
    let effective_armor_mm = effective_facet_thickness_mm(facet, normalized_angle);
    let remaining_penetration_mm =
        shell.penetration_mm_at_distance(distance_m) - effective_armor_mm;
    let penetrated = !ricocheted && remaining_penetration_mm >= 0.0;
    let damage_hp = shell_damage_hp(shell, penetrated, ricocheted);

    PenetrationResult {
        penetrated,
        ricocheted,
        effective_armor_mm,
        remaining_penetration_mm,
        damage_hp,
        module_damage_hp: module_damage_hp(shell, penetrated, ricocheted),
    }
}

fn effective_facet_thickness_mm(facet: ArmorFacet, impact_angle_degrees: f32) -> f32 {
    let nominal = facet.nominal_thickness_mm * facet.weakspot_multiplier.clamp(0.1, 2.0);
    let clamped_angle = (impact_angle_degrees.abs() + facet.slope_degrees.abs()).clamp(0.0, 89.0);
    let cosine = clamped_angle.to_radians().cos().max(0.01);
    nominal / cosine
}

fn normalized_impact_angle(shell: &ShellSpec, impact_angle_degrees: f32) -> f32 {
    let normalization = match shell.shell_type {
        ShellType::ArmorPiercing => 5.0,
        ShellType::Apcr => 2.0,
        ShellType::Heat | ShellType::HighExplosive => 0.0,
    };
    (impact_angle_degrees.abs() - normalization).max(0.0)
}

fn ricochets(shell: &ShellSpec, facet: &ArmorFacet, impact_angle_degrees: f32) -> bool {
    match shell.shell_type {
        ShellType::ArmorPiercing | ShellType::Apcr => {
            impact_angle_degrees > 70.0 && shell.caliber_mm <= facet.nominal_thickness_mm * 3.0
        }
        ShellType::Heat => impact_angle_degrees > 85.0,
        ShellType::HighExplosive => false,
    }
}

fn shell_damage_hp(shell: &ShellSpec, penetrated: bool, ricocheted: bool) -> u32 {
    if penetrated {
        shell.damage_hp
    } else if !ricocheted && shell.shell_type == ShellType::HighExplosive {
        ((shell.damage_hp as f32) * 0.18).round().max(1.0) as u32
    } else {
        0
    }
}

fn module_damage_hp(shell: &ShellSpec, penetrated: bool, ricocheted: bool) -> u32 {
    if ricocheted {
        return 0;
    }
    let multiplier = match (shell.shell_type, penetrated) {
        (ShellType::ArmorPiercing, true) => 1.0,
        (ShellType::Apcr, true) => 0.75,
        (ShellType::Heat, true) => 0.85,
        (ShellType::HighExplosive, true) => 1.25,
        (ShellType::HighExplosive, false) => 0.65,
        _ => 0.0,
    };
    ((shell.damage_hp as f32) * multiplier).round() as u32
}
