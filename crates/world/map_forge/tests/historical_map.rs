use map_forge::battlefield;
use terrain::{MapFeatureKind, MapId};

#[test]
fn prokhorovka_map_is_historical_open_and_roughly_1000m_square() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);

    assert_eq!(map.id, "prokhorovka_hill_252_2");
    assert_eq!(map.size_m, [1000.0, 1000.0]);
    assert!(map.historical_basis.contains("Prokhorovka"));
    assert!(map.design_notes.iter().any(|note| note.contains("open")));
    assert_eq!(map.spawn_zones.len(), 2);
    assert!(map.strategic_points.len() >= 4);
    assert!(map.static_cover.len() >= 4);
}

#[test]
fn prokhorovka_heightmap_has_hills_ditch_and_embankment() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let stats = map.heightmap.stats();

    assert!(stats.range_m() > 20.0, "map must not be flat: {stats:?}");
    assert!(map.feature(MapFeatureKind::Hill, "Hill 252.2").is_some());
    assert!(map.feature(MapFeatureKind::RailEmbankment, "railway embankment").is_some());
    assert!(map.feature(MapFeatureKind::AntiTankDitch, "anti-tank ditch").is_some());
    assert!(map.feature(MapFeatureKind::Lowland, "Psel lowland").is_some());
}

#[test]
fn prokhorovka_static_cover_has_runtime_collision_bounds() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let farm_cover = map
        .static_cover
        .iter()
        .find(|cover| cover.name.contains("Farm"))
        .expect("farm cover object");

    assert!(farm_cover.half_extents_m[0] > 1.0);
    assert!(farm_cover.half_extents_m[1] > 1.0);
    assert!(farm_cover.half_extents_m[2] > 1.0);
    assert!(
        farm_cover.center[1]
            > map.heightmap.sample_height(farm_cover.center[0], farm_cover.center[2]).unwrap()
    );
}

#[test]
fn prokhorovka_heightmap_is_mirror_symmetric_across_central_axis() {
    // The railway embankment sits on the central east-west axis, so the two halves must be
    // identical: height_at(x, z) == height_at(x, extent - z). Sampling a dense grid locks
    // the symmetry that keeps both teams' starting ground equivalent (competitive balance).
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;
    let [extent_x, extent_z] = hm.extent_m();

    let steps = 50;
    let mut max_delta = 0.0f32;
    for ix in 0..=steps {
        for iz in 0..=steps {
            let x = extent_x * ix as f32 / steps as f32;
            let z = extent_z * iz as f32 / steps as f32;
            let here = hm.sample_height(x, z).expect("inside map");
            let mirrored = hm.sample_height(x, extent_z - z).expect("inside map");
            max_delta = max_delta.max((here - mirrored).abs());
        }
    }

    assert!(max_delta < 1e-3, "north/south halves must mirror; max delta was {max_delta}");
}
