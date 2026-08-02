use terrain::MapId;

#[test]
fn every_map_id_round_trips_through_its_slug() {
    for &id in MapId::SHIPPED {
        assert_eq!(MapId::from_slug(id.slug()), Some(id), "slug round-trip broke for {id:?}");
    }
    assert_eq!(MapId::from_slug("no-such-map"), None);
}

#[test]
fn default_map_is_the_bystra_valley() {
    // The game PLAYS the valley: every entry point (practice duel, garage battles, tools)
    // lands on Bystra unless `WOT_MAP` explicitly overrides. Prokhorovka stays registered as
    // the test substrate (replay fixtures were recorded on it) but is out of the rotation.
    assert_eq!(MapId::default(), MapId::BystraValley);
    assert_eq!(MapId::default().slug(), "bystra-valley");
}
