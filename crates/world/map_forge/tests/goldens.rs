//! The review gate: every shipped map compiles clean, deterministic, and on its golden
//! hash. A change here is a deliberate map change — bless it consciously, never by accident.

use map_forge::{battlefield, battlefield_hash, blueprint_for, compile, map_golden_hashes};
use terrain::MapId;

#[test]
fn every_shipped_map_compiles_clean_deterministic_and_on_its_golden() {
    let goldens = map_golden_hashes();
    assert_eq!(MapId::SHIPPED.len(), goldens.len(), "every shipped map owns a golden");
    // The game PLAYS the valley by default, and the compiled default proves it.
    assert_eq!(battlefield(MapId::default()).id, "bystra_valley");
    for (name, golden) in goldens {
        let id = MapId::SHIPPED
            .iter()
            .copied()
            .find(|id| blueprint_for(*id).meta.id == name.as_str())
            .unwrap_or_else(|| panic!("{name} is shipped"));
        let blueprint = blueprint_for(id);
        let (first, report) = compile(&blueprint);
        assert!(
            !report.has_errors(),
            "{name}: shipped with contract errors: {:?}",
            report.errors().map(|e| e.message.clone()).collect::<Vec<_>>()
        );
        let second = compile(&blueprint).0;
        assert_eq!(first, second, "{name}: compilation is not deterministic");
        assert_eq!(first, battlefield(id), "{name}: catalog and compiler disagree");
        assert_eq!(
            battlefield_hash(&first),
            golden,
            "{name}: the map changed — bless the golden DELIBERATELY (0x{:016x})",
            battlefield_hash(&first)
        );
    }
}

/// Inny Poziom F2: the species gate (`report::check_species_mix`) only bites on a DRESSED map
/// — `DRESSED_MAP_TREES` or more — so a contract fixture with three oaks is not called a
/// monoculture. That leaves one way to dodge it: ship a map with eleven trees. This closes
/// it: every shipped map is dressed, so the gate judges every shipped map. The species table
/// prints for the dossiers, with the hash the golden file records.
#[test]
fn every_shipped_map_is_dressed_enough_for_the_species_gate() {
    for id in MapId::SHIPPED {
        let map = battlefield(*id);
        let counts = map_forge::species_counts(&map);
        let total: usize = counts.iter().map(|(_, count)| count).sum();
        println!(
            "SPECIES {}: {total} trees {counts:?} hash 0x{:016x}",
            map.id,
            battlefield_hash(&map)
        );
        // The dressing's own warnings are a shipping error HERE: a scatter that grew a tree
        // through a barn is shippable by the report's rule (an author may mean it) and never
        // by ours (no shipped map means it). The scenery check used to fire on every oak
        // standing in its own bole box, which made this line unwritable; F2 fixed the check.
        let (_, report) = compile(&blueprint_for(*id));
        let through_cover: Vec<String> = report
            .warnings()
            .filter(|entry| entry.check == "scenery")
            .map(|entry| format!("{} at {:?}", entry.message, entry.at))
            .collect();
        assert!(
            through_cover.is_empty(),
            "{}: dressing grows through cover — give the scatter a cover_margin_m: {through_cover:?}",
            map.id
        );
        assert!(
            total >= map_forge::DRESSED_MAP_TREES,
            "{}: {total} trees — undressed, and the species gate would not judge it",
            map.id
        );
    }
}
