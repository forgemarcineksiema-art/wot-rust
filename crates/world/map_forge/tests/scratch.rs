//! The dev playtest vessel (map-editor D2): `MapId::Scratch` carries no shipped content —
//! a document is installed per process, and before one arrives the placeholder keeps every
//! Scratch-touching path total.

use terrain::MapId;

#[test]
fn scratch_serves_the_placeholder_then_the_installed_document() {
    // Scratch is a dev vessel, not a shipped map: not in ALL, no golden, no rotation.
    assert!(!MapId::ALL.contains(&MapId::Scratch));

    // Before an install, the placeholder compiles clean (battlefield() asserts that).
    let placeholder = map_forge::battlefield(MapId::Scratch);
    assert_eq!(placeholder.id, "scratch");
    assert!(!placeholder.spawn_zones.is_empty());

    // An unparsable install is refused and changes nothing.
    assert!(map_forge::set_scratch_source("(nonsense".into()).is_err());

    // A real install wins for the process lifetime (install-once contract).
    let mut source = map_forge::blueprint_for(MapId::Scratch).to_ron();
    source = source.replace("id: \"scratch\"", "id: \"scratch_installed\"");
    map_forge::set_scratch_source(source).expect("installs");
    assert_eq!(map_forge::battlefield(MapId::Scratch).id, "scratch_installed");
    assert_eq!(map_forge::cached_blueprint(MapId::Scratch).meta.id, "scratch_installed");
}
