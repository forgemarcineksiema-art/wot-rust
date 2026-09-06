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

/// No shipped map plants a retired kind: the imported flora of Świat 2.0, the WILLOW since
/// 2026-09-03 (the owner: "I don't want a willow at all") and the PINE since 2026-09-04 (the
/// owner: "the pine is out entirely") — a blueprint that named one would draw nothing where
/// the sim believes a tree stands.
#[test]
fn no_shipped_map_plants_a_retired_kind() {
    for id in MapId::SHIPPED {
        let map = battlefield(*id);
        for instance in &map.scenery {
            assert!(
                !map_forge::RETIRED_KINDS.contains(&instance.kind),
                "{}: plants the retired {:?} at {:?}",
                map.id,
                instance.kind,
                instance.position
            );
        }
        for (kind, _) in map_forge::cached_blueprint(*id)
            .horizon
            .as_ref()
            .map(|horizon| horizon.flora.clone())
            .unwrap_or_default()
        {
            assert!(!map_forge::RETIRED_KINDS.contains(&kind), "{}: horizon {kind:?}", map.id);
        }
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

/// Inny Poziom Z3: every shipped map ships its destruction, counted. The per-map floor in
/// `DESTRUCTIBLE_FLOOR` is what the map has today; the count prints for the dossiers, and a map
/// that falls under its floor is refused by the report (`destructible_floor`).
#[test]
fn every_shipped_map_keeps_its_destructible_floor() {
    let mut judged = 0usize;
    for id in MapId::SHIPPED {
        let map = battlefield(*id);
        let count = map_forge::destructible_count(&map);
        let mut by_kind: Vec<(String, usize)> = Vec::new();
        for object in &map.static_cover {
            if object.kind.max_health().is_none() {
                continue;
            }
            let name = format!("{:?}", object.kind);
            match by_kind.iter_mut().find(|(kind, _)| *kind == name) {
                Some((_, n)) => *n += 1,
                None => by_kind.push((name, 1)),
            }
        }
        println!("DESTRUCTIBLE {}: {count} {by_kind:?}", map.id);
        let (_, floor) = map_forge::DESTRUCTIBLE_FLOOR
            .iter()
            .find(|(name, _)| *name == map.id.as_str())
            .unwrap_or_else(|| panic!("{} has no destructible floor", map.id));
        assert!(
            count >= *floor,
            "{}: {count} destructible objects under the floor {floor}",
            map.id
        );
        judged += 1;
    }
    assert_eq!(judged, MapId::SHIPPED.len());
}
