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
    let report = InventoryReport::new(&authoritative_description(VehicleKind::IS3).unwrap());
    assert!(!report.locked);
    assert!(report.is_sketch());
    assert_eq!(report.carried.len(), 1, "a wrapped recipe is one class: {:?}", report.carried);
    assert_eq!(report.missing, report.expected, "so every listed class is debt");
    assert!(report.dossier_pending.is_none(), "the IS-3's dossier lists its parts");
}

/// The Jagdtiger is the fourth (K3, 2026-09-06) and the first casemate: 9 of its 10 classes —
/// the leaned prism hull, the casemate as `TurretShell`, the cast collar as `Mantlet`, the PaK 44
/// with its plain muzzle, the periscope housing, the shoe racks, the flat bow guards, the
/// Kugelblende. The stern plate's furniture is the debt left.
#[test]
fn the_jagdtiger_carries_its_dossier_but_the_stern_plate() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::Jagdtiger).unwrap());
    assert!(!report.locked);
    assert!(!report.is_sketch(), "no recipe piece stands on the shipped Jagdtiger");
    for class in [
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::TurretShell,
        PartClass::Mantlet,
        PartClass::GunBarrel,
        PartClass::Periscopes,
        PartClass::SpareTracks,
        PartClass::Fenders,
        PartClass::CourseMg,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's");
    }
    assert!(!report.carried.contains(&PartClass::Cupola), "a casemate has no cupola");
    assert_eq!(report.missing, [PartClass::SternPlate].into_iter().collect());
    println!("{}", report.summary_line());
}

/// The Tiger II is the second vehicle the library builds whole (K3, 2026-09-06): 14 of its 15
/// classes carried — the leaned prism hull, the Schürzen, the bow flaps, the Henschel turret,
/// the Turmblende, the KwK 43, the fittings, the open stacks, the periscope hood. The stern
/// plate's own furniture is the debt left.
#[test]
fn the_tiger_ii_carries_its_dossier_but_the_stern_plate() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::TigerII).unwrap());
    assert!(!report.locked);
    assert!(!report.is_sketch(), "no recipe piece stands on the shipped Tiger II");
    for class in [
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::Skirts,
        PartClass::Fenders,
        PartClass::TurretShell,
        PartClass::TurretRing,
        PartClass::Mantlet,
        PartClass::Cupola,
        PartClass::Hatches,
        PartClass::Headlights,
        PartClass::GunBarrel,
        PartClass::MuzzleFurniture,
        PartClass::Exhaust,
        PartClass::Periscopes,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's");
        assert!(!report.missing.contains(&class));
    }
    assert_eq!(report.missing, [PartClass::SternPlate].into_iter().collect());
    println!("{}", report.summary_line());
}

/// The Panther II is the third (K3, 2026-09-06), by the Tiger II's file: 10 of its 11 classes —
/// the leaned prism hull, the G turret, the G-Blende, the KwK 42, the cupola, the Kugelblende at
/// its station, the twin periscope hoods, the curved sweeps, the ONE glacis light. The stern
/// plate's furniture is the debt left.
#[test]
fn the_panther_ii_carries_its_dossier_but_the_stern_plate() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::PantherII).unwrap());
    assert!(!report.locked);
    assert!(!report.is_sketch(), "no recipe piece stands on the shipped Panther II");
    for class in [
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::TurretShell,
        PartClass::Mantlet,
        PartClass::Cupola,
        PartClass::GunBarrel,
        PartClass::CourseMg,
        PartClass::Periscopes,
        PartClass::Fenders,
        PartClass::Headlights,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's");
    }
    assert_eq!(report.missing, [PartClass::SternPlate].into_iter().collect());
    println!("{}", report.summary_line());
}

/// The Tiger I is the first MIXED sketch (K3-2b): its recipe pieces still stand, and the
/// library's parts ride on them — the fittings (K3-2b), the guards (K3-2e), the slab hull (4a),
/// the gun (4c) and the welded turret (4b): 13 of its 23 classes carried.
#[test]
fn the_tiger_carries_its_library_fittings_over_the_recipe() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::TigerI).unwrap());
    assert!(!report.locked);
    assert!(!report.is_sketch(), "step 4d: no recipe piece stands on the shipped Tiger");
    for class in [
        PartClass::Hatches,
        PartClass::Headlights,
        PartClass::TowHooks,
        PartClass::Cupola,
        PartClass::Fenders,
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::GunBarrel,
        PartClass::Mantlet,
        PartClass::MuzzleFurniture,
        PartClass::TurretShell,
        PartClass::TurretRing,
        PartClass::TurretStowage,
        PartClass::EngineDeck,
        PartClass::DeckGrille,
        PartClass::Exhaust,
        PartClass::SpareTracks,
        PartClass::CourseMg,
        PartClass::Periscopes,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's now");
        assert!(!report.missing.contains(&class));
    }
    // Still missing after step 4d: the classes no part authors yet — the stern plate's own
    // furniture, weld seams, tow cables, the aerial, the suspension hardware.
    assert!(report.missing.contains(&PartClass::TowCable), "the tow cables are still debt");
    println!("{}", report.summary_line());
}

#[test]
fn every_dossier_with_a_part_list_is_read_into_its_inventory() {
    for (kind, at_least) in
        [(VehicleKind::T34_85, 22), (VehicleKind::IS3, 9), (VehicleKind::Centurion, 9)]
    {
        let report = InventoryReport::new(&authoritative_description(kind).unwrap());
        assert!(report.dossier_pending.is_none(), "{kind:?} lists its parts");
        assert!(report.expected.len() >= at_least, "{kind:?}: {}", report.expected.len());
        assert_eq!(report.missing, report.expected, "{kind:?} is a sketch: every row is debt");
    }
}

/// The T-34-85 was the one vehicle whose dossier read `Needs dossier` (the inventory said
/// `Pending`, invented nothing); its research pass landed 2026-09-06, so every playable vehicle
/// now lists its parts — and the `Pending` mechanism stays covered by the spec's own unit test.
#[test]
fn every_playable_vehicle_s_dossier_lists_its_parts() {
    for kind in VehicleKind::PLAYABLE {
        let report = InventoryReport::new(&authoritative_description(kind).unwrap());
        assert!(report.dossier_pending.is_none(), "{kind:?}: the dossier lists its parts");
        assert!(!report.expected.is_empty(), "{kind:?}: at least one class is named");
    }
    let report = InventoryReport::new(&authoritative_description(VehicleKind::T34_85).unwrap());
    assert!(report.expected.len() >= 22, "the T-34-85's 22 rows: {}", report.expected.len());
}
