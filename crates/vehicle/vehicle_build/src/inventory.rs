//! The part inventory (Forge 2.0 K3): which CLASSES of part a vehicle carries, against the
//! classes its dossier says the real tank has.
//!
//! K3's closing condition is an inventory gate per vehicle: "each roster vehicle carries every
//! part class of the fleet part library its dossier lists". A class is coarser than a part key
//! (`headlight`, `headlight_bracket`, `headlight_guard`, `headlight_lens` are one class,
//! `Headlights`) and finer than a submesh; it is the unit a dossier talks in. The expected list
//! lives beside the crate as RON, one file per vehicle, each row citing the dossier section it
//! comes from — a vehicle whose dossier has no part list says so (`dossier: Pending`) and the
//! gate reports that as the first debt, because a list invented here would be the instrument
//! calibrated on nothing.

use std::collections::BTreeSet;

use game_core::VehicleKind;
use serde::{Deserialize, Serialize};

use crate::description::VehicleDescription;
use crate::part::{GeneratorKind, PartKey};

/// The part classes of the fleet part library. Append-only: inventory files serialize it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PartClass {
    /// The lower hull tub (belly, sides between the tracks).
    HullTub,
    /// The upper hull: glacis, sponsons, roof plates.
    UpperHull,
    /// The stern plate with its plugs, bolt rows and covers.
    SternPlate,
    /// Plate joints as visible beads or seams.
    WeldSeams,
    /// Engine-deck panels, bolts, radiators under them.
    EngineDeck,
    /// The louvred / grilled air path on the deck.
    DeckGrille,
    /// Transmission or final-drive access covers.
    TransmissionCovers,
    /// Final-drive housings at the hull's drive end.
    FinalDrives,
    /// Fenders, mudguards, track guards and their brackets/ribs.
    Fenders,
    /// Bins and tanks riding the fenders or hull sides.
    FenderStowage,
    /// The bow splash board.
    SplashBoard,
    /// The turret's outer shell (cast dome or welded plates) and its seams.
    TurretShell,
    /// The ring collar / seat between hull and turret.
    TurretRing,
    /// Rails, hangers and hooks on the turret's outside.
    TurretRails,
    /// Stowage riding the turret (bustle bin, ring bins).
    TurretStowage,
    /// The commander's cupola with its blocks and hatch.
    Cupola,
    /// Crew hatches other than the cupola's.
    Hatches,
    /// The mantlet: internal or external casting, its cover and frame.
    Mantlet,
    /// The main gun tube.
    GunBarrel,
    /// Muzzle furniture: brake, evacuator, counterweight.
    MuzzleFurniture,
    /// A fixed bow / course machine gun port or ball.
    CourseMg,
    /// An anti-aircraft machine gun with its mount.
    AaMachineGun,
    /// The radio aerial and its base.
    Aerial,
    /// Turret / hull ventilator domes.
    Ventilator,
    /// Exhausts: ports, stacks, covers, louvres.
    Exhaust,
    /// Headlights with brackets, guards, lenses.
    Headlights,
    /// Tow hooks / shackles.
    TowHooks,
    /// The tow cable with its hardware.
    TowCable,
    /// Smoke canisters / dischargers.
    SmokeCanisters,
    /// The unditching beam / log.
    UnditchingBeam,
    /// Spare track links carried on the hull or turret.
    SpareTracks,
    /// Pioneer tools clamped to the hull.
    Tools,
    /// Suspension hardware visible outside: pivot bosses, bump stops, torsion-bar ends.
    SuspensionHardware,
    /// Interior: liners, seats, baskets, racks, breech, engine, radio, sights, components.
    Interior,
    /// The coaxial machine gun.
    CoaxMachineGun,
    /// Periscopes and vision devices outside the cupola: driver's, loader's, turret crown.
    Periscopes,
    /// Side skirts hung outside the track (Schürzen, bazooka plates) — spaced armour, honest both ways.
    Skirts,
    /// A whole recipe submesh wrapped as one part (a sketch vehicle, Forge 2.0 K1).
    RecipeSketch,
}

impl PartClass {
    /// The class of a part key, by the key's name. `None` for a name no row knows — a test
    /// keeps that set empty for every description, so a new part must name its class here.
    pub fn of(key: PartKey) -> Option<Self> {
        let name = key.name;
        let starts = |p: &str| name.starts_with(p);
        Some(match name {
            "lower_tub" => Self::HullTub,
            "upper_hull" => Self::UpperHull,
            "hull_plate_seam" | "deck_weld_bead" => Self::WeldSeams,
            "engine_deck_panel" | "engine_deck_bolts" | "radiator_core" | "radiator_fin" => {
                Self::EngineDeck
            }
            "deck_grille" => Self::DeckGrille,
            "transmission_cover" | "transmission_drum" => Self::TransmissionCovers,
            "final_drive_housing" => Self::FinalDrives,
            "stern_plate_bolts" | "stern_plug" => Self::SternPlate,
            "splash_board" => Self::SplashBoard,
            "turret_shell" | "turret_casting_seam" => Self::TurretShell,
            "turret_ring_collar" => Self::TurretRing,
            "turret_rail" => Self::TurretRails,
            "course_mg_port" => Self::CourseMg,
            "gun_barrel" => Self::GunBarrel,
            "driver_hatch" | "loader_hatch" => Self::Hatches,
            "damage_component" | "driver_seat" | "interior_liner" | "turret_inner_skin" => {
                Self::Interior
            }
            _ if starts("recipe_") => Self::RecipeSketch,
            _ if name.contains("periscope") => Self::Periscopes,
            _ if starts("sg43_coax") => Self::CoaxMachineGun,
            _ if starts("cupola") => Self::Cupola,
            _ if starts("mudguard") => Self::Fenders,
            _ if starts("skirt") => Self::Skirts,
            _ if starts("bow_rack")
                || starts("breech_guard")
                || starts("bulkhead_")
                || starts("d10_")
                || starts("driver_")
                || starts("loader_")
                || starts("radio_")
                || starts("sg43_")
                || starts("tsh2_")
                || starts("turret_drive")
                || starts("turret_extinguisher")
                || starts("turret_handwheel")
                || starts("turret_rear_round")
                || starts("turret_seat")
                || starts("v54_") =>
            {
                Self::Interior
            }
            _ if starts("dshk") => Self::AaMachineGun,
            _ if starts("exhaust") => Self::Exhaust,
            _ if starts("fender") => Self::Fenders,
            _ if starts("tank_lid") || starts("stowage_bin") || starts("fuel_tank") => {
                Self::FenderStowage
            }
            _ if starts("gun_mantlet") || starts("mantlet") => Self::Mantlet,
            _ if starts("muzzle") => Self::MuzzleFurniture,
            _ if starts("headlight") => Self::Headlights,
            _ if starts("smoke_canister") => Self::SmokeCanisters,
            _ if starts("suspension") || starts("torsion_bar") => Self::SuspensionHardware,
            _ if starts("tow_cable") => Self::TowCable,
            _ if starts("tow_hook") => Self::TowHooks,
            _ if starts("turret_aerial") => Self::Aerial,
            _ if starts("turret_basket") => Self::Interior,
            _ if starts("turret_ventilator") => Self::Ventilator,
            _ if starts("unditching_beam") => Self::UnditchingBeam,
            _ if starts("spare_track") => Self::SpareTracks,
            _ if starts("tool_") => Self::Tools,
            _ => return None,
        })
    }
}

/// Whether the dossier carries a part list at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DossierPartList {
    /// The dossier lists the vehicle's parts (its form rules / part-construction tables).
    Complete,
    /// It does not yet; the string says what is owed.
    Pending(String),
}

/// One expected class with the dossier section that names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedPart {
    pub class: PartClass,
    pub source: String,
}

/// A vehicle's expected inventory, as authored in `inventory/<slug>.inventory.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySpec {
    vehicle: String,
    dossier: DossierPartList,
    /// `Locked`: every expected class must be carried (the benchmark). `Target`: missing
    /// classes are reported as debt.
    #[serde(default)]
    locked: bool,
    expected: Vec<ExpectedPart>,
}

impl InventorySpec {
    pub fn parse(ron: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(ron)
    }

    pub fn vehicle(&self) -> &str {
        &self.vehicle
    }

    pub fn dossier(&self) -> &DossierPartList {
        &self.dossier
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn expected(&self) -> &[ExpectedPart] {
        &self.expected
    }

    pub fn expected_classes(&self) -> BTreeSet<PartClass> {
        self.expected.iter().map(|e| e.class).collect()
    }
}

/// The authored inventory for `kind`. EXHAUSTIVE like the blueprint loader: a new vehicle
/// states its inventory file here — even one that says `Pending`.
pub fn inventory_for(kind: VehicleKind) -> InventorySpec {
    let text = match kind {
        VehicleKind::T54_1951 => include_str!("../inventory/t54_1951.inventory.ron"),
        VehicleKind::TigerI => include_str!("../inventory/tiger_i_ausf_e.inventory.ron"),
        VehicleKind::TigerII => include_str!("../inventory/tiger_ii_ausf_b.inventory.ron"),
        VehicleKind::Jagdtiger => include_str!("../inventory/jagdtiger.inventory.ron"),
        VehicleKind::PantherII => include_str!("../inventory/panther_ii.inventory.ron"),
        VehicleKind::IS3 => include_str!("../inventory/is3.inventory.ron"),
        VehicleKind::Centurion => include_str!("../inventory/centurion_mk3.inventory.ron"),
        VehicleKind::T34_85 => include_str!("../inventory/t34_85.inventory.ron"),
    };
    let spec = InventorySpec::parse(text)
        .unwrap_or_else(|e| panic!("inventory/{}.inventory.ron: {e}", kind.slug()));
    assert_eq!(spec.vehicle, kind.slug(), "an inventory file names the vehicle it belongs to");
    spec
}

/// What a description carries, class by class, and the keys no class knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedInventory {
    pub classes: BTreeSet<PartClass>,
    pub unclassified: Vec<&'static str>,
}

impl VehicleDescription {
    /// The classes this description's parts fall into. A `Recipe` part is the `RecipeSketch`
    /// class whatever its key says: a sketch carries no part class of the library.
    pub fn inventory(&self) -> CarriedInventory {
        let mut classes = BTreeSet::new();
        let mut unclassified = Vec::new();
        for part in &self.parts {
            if part.generator == GeneratorKind::Recipe {
                classes.insert(PartClass::RecipeSketch);
                continue;
            }
            match PartClass::of(part.key) {
                Some(class) => {
                    classes.insert(class);
                }
                None => unclassified.push(part.key.name),
            }
        }
        unclassified.sort_unstable();
        unclassified.dedup();
        CarriedInventory { classes, unclassified }
    }
}

/// The inventory report for `kind`: expected, carried, missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryReport {
    pub kind: VehicleKind,
    pub locked: bool,
    pub dossier_pending: Option<String>,
    pub expected: BTreeSet<PartClass>,
    pub carried: BTreeSet<PartClass>,
    pub missing: BTreeSet<PartClass>,
    pub unclassified: Vec<&'static str>,
}

impl InventoryReport {
    pub fn new(description: &VehicleDescription) -> Self {
        let spec = inventory_for(description.kind);
        let carried = description.inventory();
        let expected = spec.expected_classes();
        let missing = expected.difference(&carried.classes).copied().collect();
        Self {
            kind: description.kind,
            locked: spec.locked(),
            dossier_pending: match spec.dossier() {
                DossierPartList::Complete => None,
                DossierPartList::Pending(why) => Some(why.clone()),
            },
            expected,
            carried: carried.classes,
            missing,
            unclassified: carried.unclassified,
        }
    }

    pub fn is_sketch(&self) -> bool {
        self.carried.contains(&PartClass::RecipeSketch)
    }

    pub fn summary_line(&self) -> String {
        format!(
            "{:?}: {} of {} expected classes carried{}{}",
            self.kind,
            self.expected.len() - self.missing.len(),
            self.expected.len(),
            if self.is_sketch() { " (a recipe sketch)" } else { "" },
            self.dossier_pending
                .as_ref()
                .map(|why| format!(" — no part list in the dossier: {why}"))
                .unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_family_the_library_uses_has_a_class() {
        for (name, class) in [
            ("lower_tub", PartClass::HullTub),
            ("headlight_guard", PartClass::Headlights),
            ("dshk_barrel", PartClass::AaMachineGun),
            ("gun_mantlet_cover", PartClass::Mantlet),
            ("recipe_turret", PartClass::RecipeSketch),
            ("turret_ventilator_collar", PartClass::Ventilator),
        ] {
            assert_eq!(PartClass::of(PartKey::new(name)), Some(class), "{name}");
        }
        assert_eq!(PartClass::of(PartKey::new("a_part_nobody_named")), None);
    }

    #[test]
    fn every_vehicle_has_an_inventory_file_that_names_it() {
        for kind in VehicleKind::PLAYABLE {
            let spec = inventory_for(kind);
            assert_eq!(spec.vehicle(), kind.slug());
            if spec.locked() {
                assert!(matches!(spec.dossier(), DossierPartList::Complete));
                assert!(!spec.expected().is_empty());
            }
            for row in spec.expected() {
                assert!(!row.source.trim().is_empty(), "{kind:?} {:?} cites nothing", row.class);
            }
        }
    }
}
