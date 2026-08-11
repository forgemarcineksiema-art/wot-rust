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
    /// A thin side skirt hung OUTSIDE the track band: the outermost layer of a spaced stack,
    /// never a replacement for what stands behind it. A shell resolved on this zone crosses the
    /// skirt, then the belt, then the hull side plate — see
    /// [`resolve_penetration_through_screens`](super::resolve_penetration_through_screens).
    /// Charging the skirt ALONE made bolting Schürzen onto a hull reduce its flank armour, and
    /// left the skirted vehicles unable to lose a module to a flank shot at all.
    /// Appended last — the zone rides the wire inside damage events.
    Skirt,
    /// The hull's engine deck and fighting-compartment roof. Split out from [`ArmorZone::Roof`],
    /// which used to mean BOTH the turret roof and the hull deck: one zone, one derived
    /// thickness (a fleet formula off the turret front), and a hull-deck hit that reported
    /// `TurretFront` on the wire. They are different plates on different structures — the T-54's
    /// deck is 20 mm where its turret roof is 30 — so they are different zones.
    /// Appended after `Skirt`, same rule.
    HullDeck,
    /// The commander's cupola drum, standing PROUD of the turret roof as its own convex volume.
    ///
    /// Before this zone existed the drum was unmodelled: a shell aimed at it from the front
    /// travelled above the casting and resolved against the ROOF plane at a grazing angle —
    /// auto-bounce off the most famous weakspot in tank warfare — while the facet-wide
    /// `weakspot_multiplier` smeared the compensation across the whole turret front. The drum
    /// catches that shell on its cylinder wall at a honest near-flat angle; the weakness IS the
    /// roundness, not an invented number. Appended after `HullDeck`, same wire rule.
    Cupola,
    /// A port cut through the bow plates — the driver's visor or the hull MG mount — riding the
    /// glacis as an aimable patch. Same philosophy as [`ArmorZone::Cupola`]: the weakness is
    /// GEOMETRY (the ball or visor presents FLAT where the plate rakes), not a scalar. Authored
    /// per vehicle only where the feature is VISIBLE. Appended after `Cupola`, same wire rule.
    GlacisPort,
}

impl ArmorZone {
    /// Every place a shell can land. A zone absent from here is a zone no armour-coverage test can
    /// ask about, which is how a hull ends up with an unmodelled face.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [ArmorZone; 15] = [
        ArmorZone::UpperGlacis,
        ArmorZone::LowerPlate,
        ArmorZone::HullSide,
        ArmorZone::HullRear,
        ArmorZone::TurretFront,
        ArmorZone::Mantlet,
        ArmorZone::TurretSide,
        ArmorZone::TurretRear,
        ArmorZone::Roof,
        ArmorZone::LeftTrack,
        ArmorZone::RightTrack,
        ArmorZone::Skirt,
        ArmorZone::HullDeck,
        ArmorZone::Cupola,
        ArmorZone::GlacisPort,
    ];

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
            // The deck belongs to the HULL — reporting a deck hit as "turret front" was a wire
            // lie every consumer of the damage event inherited.
            ArmorZone::HullDeck => ArmorFacing::HullFront,
            ArmorZone::TurretSide => ArmorFacing::TurretSide,
            ArmorZone::TurretRear => ArmorFacing::TurretRear,
            // The drum turns with the turret and is shot at from ahead — front aspect.
            ArmorZone::Cupola => ArmorFacing::TurretFront,
            ArmorZone::GlacisPort => ArmorFacing::HullFront,
        }
    }
}

impl ArmorProfile {
    pub fn plate(&self, zone: ArmorZone) -> ArmorFacet {
        match zone {
            ArmorZone::UpperGlacis => self.facet(ArmorFacing::HullFront),
            // Authored beats formula (the turret-roof idiom): a stated lower plate keeps the
            // module's full thickness at its stated angle. The 1.10 weakspot multiplier stays -
            // the plate is still the aimed spot on a bow-on hull; what changes is that its
            // geometry stops being an arbitrary fraction of a different plate's.
            ArmorZone::LowerPlate => match self.lower_front {
                Some(facet) => facet,
                None => derived(self.facet(ArmorFacing::HullFront), 0.72, 0.45, 1.10),
            },
            ArmorZone::HullSide => self.facet(ArmorFacing::HullSide),
            ArmorZone::HullRear => self.facet(ArmorFacing::HullRear),
            ArmorZone::TurretFront => self.facet(ArmorFacing::TurretFront),
            ArmorZone::Mantlet => derived(self.facet(ArmorFacing::TurretFront), 1.18, 0.55, 0.90),
            ArmorZone::TurretSide => self.facet(ArmorFacing::TurretSide),
            ArmorZone::TurretRear => self.facet(ArmorFacing::TurretRear),
            ArmorZone::Roof => roof_plate(self),
            ArmorZone::HullDeck => deck_plate(self),
            ArmorZone::LeftTrack | ArmorZone::RightTrack => track_plate(self),
            ArmorZone::Skirt => skirt_plate(),
            // The drum is side-grade casting standing UNSLOPED: no invented multiplier — the
            // weakness the player aims at is the roundness (a cylinder always offers a flat
            // patch) and the grade, both of which are the real vehicle's.
            ArmorZone::Cupola => derived(self.facet(ArmorFacing::TurretSide), 1.0, 0.0, 1.0),
            // The port is the bow plate's own steel presenting FLAT: losing the glacis rake is
            // the whole (real) weakness, so thickness and multiplier stay 1.0.
            ArmorZone::GlacisPort => derived(self.facet(ArmorFacing::HullFront), 1.0, 0.0, 1.0),
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

/// The TURRET roof. Authored when the vehicle's dossier states it (`TurretModule::roof_mm`),
/// otherwise the fleet formula — which gave the T-54 24 mm where its documents say 30.
fn roof_plate(profile: &ArmorProfile) -> ArmorFacet {
    if let Some(mm) = profile.turret_roof_mm {
        return ArmorFacet::new(mm, 0.0, 1.0);
    }
    let turret = profile.facet(ArmorFacing::TurretFront);
    ArmorFacet::new((turret.nominal_thickness_mm * 0.12).clamp(12.0, 35.0), 0.0, 1.0)
}

/// The HULL deck: thin plate over the engine and the fighting compartment, historically a third
/// of the roof of anything. Derived off the hull side (its own structure) rather than off the
/// turret front, which is what the shared `Roof` zone used to do.
fn deck_plate(profile: &ArmorProfile) -> ArmorFacet {
    let side = profile.facet(ArmorFacing::HullSide);
    ArmorFacet::new((side.nominal_thickness_mm * 0.25).clamp(15.0, 30.0), 0.0, 1.0)
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

/// The same resolution, against a plate that carries only PART of its zone's thickness.
///
/// A zone is a six-bucket abstraction and a cast turret is not a box: the T-54's wall is 160 mm
/// behind the cheeks and 65 mm at the rear. The armour volume already sweeps that casting as
/// per-azimuth sectors, so the sector the shell actually struck reports its share of the wall
/// and the shell resolves against the metal that is really there. `scale` of 1.0 is exactly
/// [`resolve_penetration_at_distance_on_zone`], which every flat plate keeps using.
pub fn resolve_penetration_at_distance_on_zone_scaled(
    shell: &ShellSpec,
    armor: &ArmorProfile,
    zone: ArmorZone,
    impact_angle_degrees: f32,
    distance_m: f32,
    scale: f32,
) -> PenetrationResult {
    let plate = armor.plate(zone);
    let scaled = ArmorFacet::new(
        plate.nominal_thickness_mm * scale.clamp(0.05, 4.0),
        plate.slope_degrees,
        plate.weakspot_multiplier,
    );
    super::resolve_penetration_at_distance_on_facet(shell, scaled, impact_angle_degrees, distance_m)
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
