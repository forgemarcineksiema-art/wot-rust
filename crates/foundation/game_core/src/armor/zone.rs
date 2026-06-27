use serde::{Deserialize, Serialize};

use super::{ArmorFacet, ArmorFacing, ArmorProfile, PenetrationResult};
use crate::ShellSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ArmorZone {
    #[default]
    UpperGlacis,
    LowerPlate,
    HullSide,
    HullRear,
    TurretFront,
    Mantlet,
    TurretSide,
    TurretRear,
    Roof,
    LeftTrack,
    RightTrack,
}

impl ArmorZone {
    pub fn facing(self) -> ArmorFacing {
        match self {
            ArmorZone::UpperGlacis | ArmorZone::LowerPlate => ArmorFacing::HullFront,
            ArmorZone::HullSide | ArmorZone::LeftTrack | ArmorZone::RightTrack => {
                ArmorFacing::HullSide
            }
            ArmorZone::HullRear => ArmorFacing::HullRear,
            ArmorZone::TurretFront | ArmorZone::Mantlet | ArmorZone::Roof => {
                ArmorFacing::TurretFront
            }
            ArmorZone::TurretSide => ArmorFacing::TurretSide,
            ArmorZone::TurretRear => ArmorFacing::TurretRear,
        }
    }
}

impl ArmorProfile {
    pub fn plate(&self, zone: ArmorZone) -> ArmorFacet {
        match zone {
            ArmorZone::UpperGlacis => self.facet(ArmorFacing::HullFront),
            ArmorZone::LowerPlate => derived(self.facet(ArmorFacing::HullFront), 0.72, 0.45, 1.10),
            ArmorZone::HullSide => self.facet(ArmorFacing::HullSide),
            ArmorZone::HullRear => self.facet(ArmorFacing::HullRear),
            ArmorZone::TurretFront => self.facet(ArmorFacing::TurretFront),
            ArmorZone::Mantlet => derived(self.facet(ArmorFacing::TurretFront), 1.18, 0.55, 0.90),
            ArmorZone::TurretSide => self.facet(ArmorFacing::TurretSide),
            ArmorZone::TurretRear => self.facet(ArmorFacing::TurretRear),
            ArmorZone::Roof => roof_plate(self),
            ArmorZone::LeftTrack | ArmorZone::RightTrack => track_plate(self),
        }
    }
}

fn derived(base: ArmorFacet, thickness_mul: f32, slope_mul: f32, weakspot_mul: f32) -> ArmorFacet {
    ArmorFacet::new(
        base.nominal_thickness_mm * thickness_mul,
        base.slope_degrees * slope_mul,
        base.weakspot_multiplier * weakspot_mul,
    )
}

fn roof_plate(profile: &ArmorProfile) -> ArmorFacet {
    let turret = profile.facet(ArmorFacing::TurretFront);
    ArmorFacet::new((turret.nominal_thickness_mm * 0.12).clamp(12.0, 35.0), 0.0, 1.0)
}

fn track_plate(profile: &ArmorProfile) -> ArmorFacet {
    let side = profile.facet(ArmorFacing::HullSide);
    ArmorFacet::new((side.nominal_thickness_mm * 0.35).clamp(18.0, 35.0), 0.0, 1.0)
}

pub fn resolve_penetration_at_distance_on_zone(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    zone: ArmorZone,
    impact_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    super::resolve_penetration_at_distance_on_facet(
        shell,
        armor.plate(zone),
        impact_angle_degrees,
        distance_m,
    )
}
