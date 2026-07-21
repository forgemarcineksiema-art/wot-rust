//! Migration note: this gate originally proved the blueprint-compiled battlefields equal the
//! historical code generators bit for bit (40401 height samples plus every cover, scenery,
//! road and gameplay entry, per map). It shipped green, the legacy generators were deleted,
//! and `MAP_GOLDEN_HASHES` took over as the deliberate-change review lock.
//!
//! What stays here as the everyday contract: compilation is deterministic and the RON
//! document round-trips losslessly.

use map_forge::{blueprint_for, compile};
use terrain::MapId;

#[test]
fn compilation_is_deterministic() {
    for id in MapId::ALL {
        let first = compile(&blueprint_for(*id)).0;
        let second = compile(&blueprint_for(*id)).0;
        assert_eq!(first, second, "{id:?} compilation is not deterministic");
    }
}

#[test]
fn blueprint_ron_round_trips_losslessly() {
    for id in MapId::ALL {
        let blueprint = blueprint_for(*id);
        let ron = blueprint.to_ron();
        let parsed = map_forge::blueprint::MapBlueprint::from_ron(&ron)
            .expect("serialized blueprint parses");
        assert_eq!(parsed, blueprint, "RON round-trip drifted for {id:?}");
        let (a, _) = compile(&parsed);
        let (b, _) = compile(&blueprint);
        assert_eq!(a, b, "RON round-trip changed the map for {id:?}");
    }
}
