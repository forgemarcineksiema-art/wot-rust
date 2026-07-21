//! The review gate: every shipped map compiles clean, deterministic, and on its golden
//! hash. A change here is a deliberate map change — bless it consciously, never by accident.

use map_forge::{battlefield, battlefield_hash, blueprint_for, compile, map_golden_hashes};
use terrain::MapId;

#[test]
fn every_shipped_map_compiles_clean_deterministic_and_on_its_golden() {
    let goldens = map_golden_hashes();
    assert_eq!(MapId::ALL.len(), goldens.len(), "every shipped map owns a golden");
    // The game PLAYS the valley by default, and the compiled default proves it.
    assert_eq!(battlefield(MapId::default()).id, "bystra_valley");
    for (name, golden) in goldens {
        let id = MapId::ALL
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
