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
    /// A thin side skirt hung outside the track: a spaced standoff SCREEN, resolved like the
    /// track band (it strips its LOS off the shell — a multiple for HEAT — before the hull side
    /// plate behind it), but it is sheet metal, NOT running gear: a skirt hit never degrades the
    /// track. Appended last — the zone rides the wire inside damage events.
    Skirt,
}

impl ArmorZone {
    pub fn facing(self) -> ArmorFacing {
        match self {
            ArmorZone::UpperGlacis | ArmorZone::LowerPlate => ArmorFacing::HullFront,
            ArmorZone::HullSide
            | ArmorZone::LeftTrack
            | ArmorZone::RightTrack
            | ArmorZone::Skirt => ArmorFacing::HullSide,
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
            ArmorZone::Skirt => skirt_plate(),
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

/// A skirt is thin sheet (historically 5–10 mm): almost no steel to a kinetic round, but the
/// STANDOFF it creates is what the HEAT screen factor prices — the jet detonates a track-width
/// early and the side plate behind eats a spent slug.
fn skirt_plate() -> ArmorFacet {
    ArmorFacet::new(8.0, 0.0, 1.0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ShellSpec, VehicleKind};

    #[test]
    fn a_skirt_is_thin_sheet_that_screens_a_heat_jet_like_the_track_band() {
        let armor = VehicleKind::T54_1951.spec().hull;
        let plate = armor.plate(ArmorZone::Skirt);
        assert!(plate.nominal_thickness_mm <= 10.0, "sheet metal, not a hull plate");
        assert!(
            plate.nominal_thickness_mm < armor.plate(ArmorZone::LeftTrack).nominal_thickness_mm,
            "thinner than the track band it hangs beside"
        );

        // The spaced resolve prices the skirt exactly like the track band: a HEAT jet loses a
        // MULTIPLE of the sheet's LOS (the standoff kills it), a kinetic round only the sheet.
        let heat = ShellSpec::heat(100.0, 900.0, 280.0, 320);
        let ap = ShellSpec::armor_piercing(100.0, 895.0, 185.0, 320);
        let heat_result = super::super::resolve_penetration_through_track(
            &heat,
            &armor,
            ArmorZone::Skirt,
            0.0,
            0.0,
            100.0,
        );
        let ap_result = super::super::resolve_penetration_through_track(
            &ap,
            &armor,
            ArmorZone::Skirt,
            0.0,
            0.0,
            100.0,
        );
        let side = armor.facet(ArmorFacing::HullSide).nominal_thickness_mm;
        let heat_screen = heat_result.effective_armor_mm - side;
        let ap_screen = ap_result.effective_armor_mm - side;
        assert!(
            heat_screen > ap_screen * 1.5,
            "the standoff must punish HEAT harder: {heat_screen} vs {ap_screen}"
        );
    }
}
