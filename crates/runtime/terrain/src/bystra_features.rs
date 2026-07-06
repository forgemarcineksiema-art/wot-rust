use crate::bystra::{
    FORD_OFFSET_M, HALF_M, QUARRY_X_M, TOWN_CENTER_X_M, WINDMILL_X_M, bystra_river_center_x,
};
use crate::map_build::grounded_feature;
use crate::{HeightMap, MapFeature, MapFeatureKind};

/// Descriptive feature metadata (labels/tests, not physics) — the named anatomy of the valley.
pub(crate) fn valley_features(heightmap: &HeightMap) -> Vec<MapFeature> {
    vec![
        grounded_feature(
            heightmap,
            MapFeatureKind::Lowland,
            "Bystra river",
            bystra_river_center_x(HALF_M),
            HALF_M,
            500.0,
            "the river runs along the axis of advance; its current is drowning-deep everywhere \
             except the two ford sills",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Crossing,
            "Kamienna stone bridge",
            bystra_river_center_x(HALF_M),
            HALF_M,
            40.0,
            "causeway deck ~1.4 m over the water, walled by parapets — the town-gate crossing",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Crossing,
            "southern ford",
            bystra_river_center_x(HALF_M - FORD_OFFSET_M),
            HALF_M - FORD_OFFSET_M,
            35.0,
            "shallow sill: slow, exposed, and honest",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Crossing,
            "northern ford",
            bystra_river_center_x(HALF_M + FORD_OFFSET_M),
            HALF_M + FORD_OFFSET_M,
            35.0,
            "shallow sill: slow, exposed, and honest",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Hill,
            "Windmill Hill",
            WINDMILL_X_M,
            HALF_M,
            140.0,
            "the western high ground: hull-down shelves on its river-facing shoulder command \
             the bridge and both fords",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Farm,
            "town of Kamienna",
            TOWN_CENTER_X_M,
            HALF_M,
            190.0,
            "a mirrored block grid on the eastern bench; church and market square on the axis",
        ),
        grounded_feature(
            heightmap,
            MapFeatureKind::Ridge,
            "quarry ridge",
            QUARRY_X_M,
            HALF_M,
            160.0,
            "the eastern valley wall: perches over the town, a sheltered quarry bowl on the axis",
        ),
    ]
}
