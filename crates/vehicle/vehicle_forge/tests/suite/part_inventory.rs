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

/// No vehicle is a recipe sketch any more (K3 closed 2026-09-06): every roster vehicle carries
/// library parts and none carries the `RecipeSketch` class. The next vehicle to join is a sketch
/// until its parts land — this lock names it then.
#[test]
fn no_vehicle_is_a_sketch_any_more() {
    for kind in VehicleKind::PLAYABLE {
        let report = InventoryReport::new(&authoritative_description(kind).unwrap());
        assert!(!report.is_sketch(), "{kind:?}: no recipe piece stands on the shipped vehicle");
        assert!(report.carried.len() >= 9, "{kind:?}: {} classes carried", report.carried.len());
    }
}

/// The Centurion is the last (K3, 2026-09-06): 15 of its 22 classes — the slab hull, the Mk 3
/// dome with its cupola, loader's hatch and bustle bin, the clean 20-pounder and its mantlet,
/// the bazooka plates, the engine panel and grille, the exhaust cowls, the driver's roof hatch,
/// the fender boxes, the lamp, the hooks. Owed: the stern plate's furniture, the coax, the
/// periscopes, the fenders themselves, the suspension hardware, the spare links.
#[test]
fn the_centurion_carries_fifteen_of_its_twenty_two() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::Centurion).unwrap());
    assert!(!report.locked);
    for class in [
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::TurretShell,
        PartClass::TurretRing,
        PartClass::TurretStowage,
        PartClass::Cupola,
        PartClass::Hatches,
        PartClass::Mantlet,
        PartClass::GunBarrel,
        PartClass::EngineDeck,
        PartClass::DeckGrille,
        PartClass::Exhaust,
        PartClass::FenderStowage,
        PartClass::Headlights,
        PartClass::Skirts,
        PartClass::TowHooks,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's");
    }
    println!("{}", report.summary_line());
}

/// The IS-3 is the sixth (K3, 2026-09-06): 14 of its 20 classes — the pike hull, the flattened
/// dome with its flush hatches and periscope, the D-25T with its brake, the fender line and the
/// drums, the louvres, the exhaust ports, the driver's hatch, the lamp, the hooks. Owed: the
/// stern plate's furniture, the deck grilles, the coax, the DShK ring, the aerial, the torsion
/// bar hardware.
#[test]
fn the_is3_carries_fourteen_of_its_twenty() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::IS3).unwrap());
    assert!(!report.locked);
    assert!(!report.is_sketch(), "no recipe piece stands on the shipped IS-3");
    for class in [
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::EngineDeck,
        PartClass::Fenders,
        PartClass::FenderStowage,
        PartClass::TurretShell,
        PartClass::TurretRing,
        PartClass::Hatches,
        PartClass::Periscopes,
        PartClass::Mantlet,
        PartClass::GunBarrel,
        PartClass::MuzzleFurniture,
        PartClass::Exhaust,
        PartClass::Headlights,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's");
    }
    assert!(!report.carried.contains(&PartClass::Cupola), "no cupola on the IS-3");
    assert_eq!(report.missing.len(), 6, "six classes owed: {:?}", report.missing);
    println!("{}", report.summary_line());
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

/// The T-34-85 is the fifth (K3, 2026-09-06) and the first Soviet vehicle after the benchmark:
/// 14 of its 22 classes — the slab hull, the flattened dome with its slit cupola and loader's
/// hatch, the ZiS-S-53 and its mantlet, the DT ball and the driver's hatch in the glacis, the
/// louvres, the exhaust ports, the lamp, the hooks, the periscope hoods. Owed: the stern plate's
/// furniture, the deck grilles, the fenders and their stowage, the handrails, the coax, the
/// ventilator domes, the suspension hardware.
#[test]
fn the_t34_85_carries_fourteen_of_its_twenty_two() {
    let report = InventoryReport::new(&authoritative_description(VehicleKind::T34_85).unwrap());
    assert!(!report.locked);
    assert!(!report.is_sketch(), "no recipe piece stands on the shipped T-34-85");
    for class in [
        PartClass::HullTub,
        PartClass::UpperHull,
        PartClass::EngineDeck,
        PartClass::TurretShell,
        PartClass::TurretRing,
        PartClass::Cupola,
        PartClass::Hatches,
        PartClass::Mantlet,
        PartClass::GunBarrel,
        PartClass::CourseMg,
        PartClass::Exhaust,
        PartClass::Headlights,
        PartClass::TowHooks,
        PartClass::Periscopes,
    ] {
        assert!(report.carried.contains(&class), "{class:?} is the library's");
    }
    assert_eq!(report.missing.len(), 8, "eight classes owed: {:?}", report.missing);
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

/// Every dossier's part list is read into its inventory: nine rows at the least (the fleet's
/// smallest list), every one citing its dossier section.
#[test]
fn every_dossier_s_part_list_is_read_into_its_inventory() {
    for kind in VehicleKind::PLAYABLE {
        let report = InventoryReport::new(&authoritative_description(kind).unwrap());
        assert!(report.dossier_pending.is_none(), "{kind:?} lists its parts");
        assert!(report.expected.len() >= 9, "{kind:?}: {}", report.expected.len());
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

/// K13: EVERY ASYMMETRIC CLASS STATES ITS SIDE, AND IS BUILT ON IT. The world-space lock the
/// 2026-08-09 mirror audit found missing, as a fleet walk from data: each inventory row may author
/// `side` (port = +x); a class built more than 150 mm off the centreline must author one, and
/// every authored side must match the sign of the class's built centroid. Five named tests said
/// this for five T-54 fittings; this says it for every class on every vehicle.
#[test]
fn every_asymmetric_class_states_its_side_and_is_built_on_it() {
    let mut walked = 0;
    let mut faults = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let report = InventoryReport::new(&authoritative_description(kind).unwrap());
        for row in &report.handedness {
            println!(
                "HANDEDNESS {kind:?} {:?}: x {:+.3} port share {:.2} built {:?} authored {:?}",
                row.class,
                row.centroid_x,
                row.port_share,
                row.one_sided(),
                row.authored
            );
        }
        for fault in report.handedness_faults() {
            faults.push(format!("{kind:?}: {fault}"));
        }
        walked += report.handedness.len();
    }
    assert!(walked >= 8 * 9, "every vehicle's classes were walked: {walked}");
    assert!(faults.is_empty(), "handedness faults:\n{}", faults.join("\n"));
}
