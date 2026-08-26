//! Immersja A3.1 locks: the horizon numbers are POLICY, not leftovers. The 2026-08-03
//! world-scale audit found the enclosing hills reading as berms (24–60 m) and Prokhorovka
//! owning no horizon at all — the only map whose world ended at the apron. The raise is a
//! per-map judgment (a valley is carved from mass, a pass looks up at peaks, a steppe's
//! horizon is a LINE), and this table is its deliberate record.

use terrain::MapId;

#[test]
fn every_shipped_map_authors_its_horizon_at_its_blessed_height() {
    let blessed = [
        (MapId::BystraValley, 65.0_f32),
        (MapId::Ostrogorsk, 58.0),
        (MapId::OrlinyPereval, 130.0),
        (MapId::ProkhorovkaHill252_2, 22.0),
        // A lakeland's horizon is low rolling moraine - barely above the map's own
        // drumlins; the enclosure is a suggestion of more lake country, not a wall.
        (MapId::MazurskiPrzesmyk, 20.0),
    ];
    assert_eq!(blessed.len(), MapId::SHIPPED.len(), "every shipped map is in the table");
    for (id, hills_base_m) in blessed {
        let blueprint = map_forge::blueprint_for(id);
        let horizon = blueprint
            .horizon
            .as_ref()
            .unwrap_or_else(|| panic!("{id:?}: every shipped map closes its horizon"));
        assert_eq!(
            horizon.hills_base_m, hills_base_m,
            "{id:?}: the horizon height is a deliberate per-map judgment — change the \
             blueprint AND this record together"
        );
        // And the enclosure actually rises past the border instead of sinking under it:
        // the closing hills must stand above the terrain at the border's edge.
        let [width, depth] = blueprint.grid.size_m;
        let edge = map_forge::backdrop_height(&blueprint, width * 0.5, depth + 5.0);
        let far = map_forge::backdrop_height(&blueprint, width * 0.5, depth + 1_200.0);
        assert!(
            far > edge + 8.0,
            "{id:?}: the horizon must RISE toward its hills, got edge {edge} vs far {far}"
        );
    }
}
