use crate::map_build::grounded_feature;
use crate::prokhorovka::{DITCH_OFFSET_M, HALF_M, HILL_OFFSET_M, HILL_X_M};
use crate::{HeightMap, MapFeature, MapFeatureKind};

pub(crate) fn map_features(heightmap: &HeightMap) -> Vec<MapFeature> {
    vec![
        grounded_feature(
            heightmap,
            MapFeatureKind::Hill,
            "Hill 252.2",
            HILL_X_M,
            HALF_M - HILL_OFFSET_M,
            130.0,
            "dominant eastern high ground (south massif)",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Hill,
            "Hill 252.2 north massif",
            HILL_X_M,
            HALF_M + HILL_OFFSET_M,
            130.0,
            "mirrored northern high ground",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::RailEmbankment,
            "railway embankment",
            HALF_M,
            HALF_M,
            480.0,
            "central east-west embankment dividing the sectors",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::AntiTankDitch,
            "anti-tank ditch (south)",
            HALF_M,
            HALF_M - DITCH_OFFSET_M,
            480.0,
            "defensive ditch on the southern approach",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::AntiTankDitch,
            "anti-tank ditch (north)",
            HALF_M,
            HALF_M + DITCH_OFFSET_M,
            480.0,
            "defensive ditch on the northern approach",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Lowland,
            "Psel lowland",
            120.0,
            HALF_M,
            150.0,
            "open western flank along the Psel",
        ),
        grounded_feature(
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
