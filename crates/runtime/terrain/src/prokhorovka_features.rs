use crate::prokhorovka::{DITCH_OFFSET_M, HALF_M, HILL_OFFSET_M, HILL_X_M};
use crate::prokhorovka_layout::position;
use crate::{HeightMap, MapFeature, MapFeatureKind};

pub(crate) fn map_features(heightmap: &HeightMap) -> Vec<MapFeature> {
    vec![
        feature(
            heightmap,
            MapFeatureKind::Hill,
            "Hill 252.2",
            HILL_X_M,
            HALF_M - HILL_OFFSET_M,
            130.0,
            "dominant eastern high ground (south massif)",
        ),
        feature(
            heightmap,
            MapFeatureKind::Hill,
            "Hill 252.2 north massif",
            HILL_X_M,
            HALF_M + HILL_OFFSET_M,
            130.0,
            "mirrored northern high ground",
        ),
        feature(
            heightmap,
            MapFeatureKind::RailEmbankment,
            "railway embankment",
            HALF_M,
            HALF_M,
            480.0,
            "central east-west embankment dividing the sectors",
        ),
        feature(
            heightmap,
            MapFeatureKind::AntiTankDitch,
            "anti-tank ditch (south)",
            HALF_M,
            HALF_M - DITCH_OFFSET_M,
            480.0,
            "defensive ditch on the southern approach",
        ),
        feature(
            heightmap,
            MapFeatureKind::AntiTankDitch,
            "anti-tank ditch (north)",
            HALF_M,
            HALF_M + DITCH_OFFSET_M,
            480.0,
            "defensive ditch on the northern approach",
        ),
        feature(
            heightmap,
            MapFeatureKind::Lowland,
            "Psel lowland",
            120.0,
            HALF_M,
            150.0,
            "open western flank along the Psel",
        ),
        feature(
            heightmap,
            MapFeatureKind::Farm,
            "Oktyabrskiy State Farm",
            HALF_M,
            HALF_M,
            55.0,
            "contested central objective on the embankment",
        ),
    ]
}

fn feature(
    heightmap: &HeightMap,
    kind: MapFeatureKind,
    name: &str,
    x: f32,
    z: f32,
    radius_m: f32,
    note: &str,
) -> MapFeature {
    MapFeature {
        kind,
        name: name.to_string(),
        center: position(heightmap, x, z),
        radius_m,
        note: note.to_string(),
    }
}
