//! The part-inventory gate (Forge 2.0 K3): each vehicle against the part classes its dossier
//! lists.
//!
//! K3 closes vehicle by vehicle when "each roster vehicle carries every part class of the
//! fleet part library its dossier lists". This is that sentence as a test. The benchmark is
//! `locked` and must carry everything it lists; a sketch reports its debt (every class, since
//! a wrapped recipe carries no library class at all); a vehicle whose dossier has no part list
//! reports THAT as its first debt, because the list is the dossier's to write, not this file's.

use game_core::VehicleKind;
use vehicle_build::{InventoryReport, PartClass};
use vehicle_forge::authoritative_description;

#[test]
fn every_locked_inventory_is_complete_and_every_vehicle_reports() {
    let mut gated = 0;
    let mut debts = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let description = authoritative_description(kind).expect("describes");
        let report = InventoryReport::new(&description);
        assert!(
            report.unclassified.is_empty(),
            "{kind:?}: parts with no class — name them in `PartClass::of`: {:?}",
            report.unclassified
        );
        if report.locked {
            assert!(!report.is_sketch(), "{kind:?}: a locked inventory cannot be a recipe sketch");
            assert!(
                report.missing.is_empty(),
                "{kind:?}: the dossier lists classes the bake does not carry: {:?}",
                report.missing
            );
        } else {
            let mut line = report.summary_line();
            if !report.missing.is_empty() {
                line.push_str(&format!(" — missing {:?}", report.missing));
            }
            debts.push(line);
        }
        gated += 1;
    }
    assert_eq!(gated, VehicleKind::PLAYABLE.len(), "every vehicle carries an inventory file");
    for line in &debts {
        println!("INVENTORY DEBT: {line}");
    }
}

#[test]
fn the_benchmark_carries_its_whole_dossier_and_nothing_unnamed() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::T54_1951).unwrap());
    assert!(report.locked);
    assert!(report.missing.is_empty(), "{:?}", report.missing);
    assert!(report.expected.len() >= 25, "the benchmark's dossier lists the whole tank");
    assert!(!report.carried.contains(&PartClass::RecipeSketch));
}

#[test]
fn a_sketch_carries_exactly_the_recipe_class() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::TigerII).unwrap());
    assert!(!report.locked);
    assert!(report.is_sketch());
    assert_eq!(report.carried.len(), 1, "a wrapped recipe is one class: {:?}", report.carried);
    assert_eq!(report.missing, report.expected, "so every listed class is debt");
    assert!(report.dossier_pending.is_none(), "the Tiger II's dossier lists its parts");
}

/// The Tiger I is the first MIXED sketch (K3-2b): its recipe pieces still stand, and the
/// library's fittings from the STT sheet ride on them — four of its 23 classes carried, five
/// with the track guards (K3-2e).
#[test]
fn the_tiger_carries_its_library_fittings_over_the_recipe() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::TigerI).unwrap());
    assert!(!report.locked);
    assert!(report.is_sketch(), "the recipe pieces still stand");
    for class in [
        PartClass::Hatches,
        PartClass::Headlights,
        PartClass::TowHooks,
        PartClass::Cupola,
        PartClass::Fenders,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's now");
        assert!(!report.missing.contains(&class));
    }
    assert!(report.missing.contains(&PartClass::TurretShell), "the turret is still the recipe's");
    println!("{}", report.summary_line());
}

#[test]
fn every_dossier_with_a_part_list_is_read_into_its_inventory() {
    for (kind, at_least) in [
        (VehicleKind::TigerII, 15),
        (VehicleKind::PantherII, 11),
        (VehicleKind::Jagdtiger, 10),
        (VehicleKind::IS3, 9),
        (VehicleKind::Centurion, 9),
    ] {
        let report = InventoryReport::new(&authoritative_description(kind).unwrap());
        assert!(report.dossier_pending.is_none(), "{kind:?} lists its parts");
        assert!(report.expected.len() >= at_least, "{kind:?}: {}", report.expected.len());
        assert_eq!(report.missing, report.expected, "{kind:?} is a sketch: every row is debt");
    }
}

#[test]
fn a_vehicle_without_a_dossier_part_list_says_so() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::T34_85).unwrap());
    assert!(report.dossier_pending.is_some(), "the T-34-85 dossier says `Needs dossier`");
    assert!(report.expected.is_empty(), "nothing invented in its place");
}
