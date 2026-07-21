//! The shipped-map catalog: every battlefield the game may play, as an embedded blueprint
//! document. `include_str!` keeps the bake hermetic (no filesystem at runtime) and both ends
//! of the wire compile the SAME document — the world never crosses the network, only the
//! agreement on which blueprint to compile does (`terrain::MapId`).

use std::sync::OnceLock;

use terrain::{BattlefieldMap, MapId};

use crate::blueprint::MapBlueprint;
use crate::compile::compile;

/// The scratch document for this process (`MapId::Scratch`, map-editor D2 dev path).
/// Installed once at startup — BEFORE any battle compiles Scratch — by whoever read the
/// out-of-band path (`WOT_MAP=<file>.map.ron`). Never networked, never in the rotation.
static SCRATCH_SOURCE: OnceLock<String> = OnceLock::new();

/// Install the scratch blueprint source. The source must parse (nothing is installed
/// otherwise — the caller surfaces the error); the first successful install wins for the
/// process lifetime, which is exactly the playtest contract: one process, one document.
pub fn set_scratch_source(source: String) -> Result<(), crate::ForgeError> {
    MapBlueprint::from_ron(&source)?;
    let _ = SCRATCH_SOURCE.set(source);
    Ok(())
}

/// The blueprint document behind a map: the shipped catalog for registered maps, the
/// installed (or placeholder) document for `Scratch`.
pub fn blueprint_for(map: MapId) -> MapBlueprint {
    let source = match map {
        MapId::ProkhorovkaHill252_2 => {
            include_str!("../blueprints/prokhorovka-hill-252-2.map.ron")
        }
        MapId::BystraValley => include_str!("../blueprints/bystra-valley.map.ron"),
        MapId::Scratch => SCRATCH_SOURCE
            .get()
            .map(String::as_str)
            .unwrap_or(include_str!("../blueprints/scratch-placeholder.map.ron")),
    };
    MapBlueprint::from_ron(source).expect("shipped blueprints parse")
}

/// The shipped blueprint, parsed once per process. For the consumers that read blueprint
/// DATA at scene/battle setup (ground palette, environment looks, the server's weather
/// table) — reparsing the document per call would be wasteful, per-frame use stays wrong.
pub fn cached_blueprint(map: MapId) -> &'static MapBlueprint {
    static ALL: OnceLock<Vec<MapBlueprint>> = OnceLock::new();
    static SCRATCH: OnceLock<MapBlueprint> = OnceLock::new();
    if map == MapId::Scratch {
        // Sound because set_scratch_source is install-once: the first battle to touch
        // Scratch freezes the document the whole process agreed on.
        return SCRATCH.get_or_init(|| blueprint_for(MapId::Scratch));
    }
    let all = ALL.get_or_init(|| MapId::ALL.iter().map(|id| blueprint_for(*id)).collect());
    let index = MapId::ALL.iter().position(|id| *id == map).expect("every shipped map is in ALL");
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
