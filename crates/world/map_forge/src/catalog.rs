//! The shipped-map catalog: every battlefield the game may play, as an embedded blueprint
//! document. `include_str!` keeps the bake hermetic (no filesystem at runtime) and both ends
//! of the wire compile the SAME document — the world never crosses the network, only the
//! agreement on which blueprint to compile does (`terrain::MapId`).

use std::sync::OnceLock;

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

/// The shipped blueprint, parsed once per process. For the consumers that read blueprint
/// DATA at scene/battle setup (ground palette, environment looks, the server's weather
/// table) — reparsing the document per call would be wasteful, per-frame use stays wrong.
pub fn cached_blueprint(map: MapId) -> &'static MapBlueprint {
    static ALL: OnceLock<Vec<MapBlueprint>> = OnceLock::new();
    let all = ALL.get_or_init(|| MapId::ALL.iter().map(|id| blueprint_for(*id)).collect());
    let index = MapId::ALL.iter().position(|id| *id == map).expect("every MapId is registered");
    &all[index]
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
