//! The shipped-map catalog: every battlefield the game may play, as an embedded blueprint
//! document. `include_str!` keeps the bake hermetic (no filesystem at runtime) and both ends
//! of the wire compile the SAME document — the world never crosses the network, only the
//! agreement on which blueprint to compile does (`terrain::MapId`).

use terrain::{BattlefieldMap, MapId};

use crate::blueprint::MapBlueprint;
use crate::compile::compile;

/// The blueprint document behind a shipped map.
pub fn blueprint_for(map: MapId) -> MapBlueprint {
    let source = match map {
        MapId::ProkhorovkaHill252_2 => {
            include_str!("../blueprints/prokhorovka-hill-252-2.map.ron")
        }
        MapId::BystraValley => include_str!("../blueprints/bystra-valley.map.ron"),
    };
    MapBlueprint::from_ron(source).expect("shipped blueprints parse")
}

/// Compile the shipped map. Deterministic: every call, on any machine, yields the same
/// battlefield. Panics if the catalog ever ships a map that fails its own contracts — that
/// is a build-time bug, caught by the catalog tests.
pub fn battlefield(map: MapId) -> BattlefieldMap {
    let blueprint = blueprint_for(map);
    let (battlefield, report) = compile(&blueprint);
    assert!(
        !report.has_errors(),
        "shipped map {} fails its contracts: {:?}",
        blueprint.meta.id,
        report.errors().map(|entry| entry.message.clone()).collect::<Vec<_>>()
    );
    battlefield
}
