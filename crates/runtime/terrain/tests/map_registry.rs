use terrain::MapId;

#[test]
fn every_map_id_round_trips_through_its_slug() {
    for &id in MapId::ALL {
        assert_eq!(MapId::from_slug(id.slug()), Some(id), "slug round-trip broke for {id:?}");
    }
    assert_eq!(MapId::from_slug("no-such-map"), None);
}

#[test]
fn registry_builds_every_map_deterministically() {
    for &id in MapId::ALL {
        let first = id.battlefield();
        let second = id.battlefield();
        assert_eq!(first, second, "map generator for {id:?} is not deterministic");
        assert!(!first.spawn_zones.is_empty(), "map {id:?} has no spawn zones");
    }
}

#[test]
fn default_map_is_the_bystra_valley() {
    // The game PLAYS the valley: every entry point (practice duel, garage battles, tools)
    // lands on Bystra unless `WOT_MAP` explicitly overrides. Prokhorovka stays registered as
    // the test substrate (replay fixtures were recorded on it) but is out of the rotation.
    assert_eq!(MapId::default(), MapId::BystraValley);
    assert_eq!(MapId::default().battlefield().id, "bystra_valley");
}
