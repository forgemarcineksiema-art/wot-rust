use crate::map_build::grounded_cover;
use crate::{HeightMap, StaticCoverKind, StaticCoverObject};

pub(crate) fn static_cover_objects(heightmap: &HeightMap) -> Vec<StaticCoverObject> {
    // Mirror-symmetric cover (north/south pairs and on-axis objects) so both halves play
    // identically. Positions sit on the 1000m symmetric skeleton from `prokhorovka.rs`.
    vec![
        grounded_cover(
            heightmap,
            "oktyabrskiy_barn_south",
            "Oktyabrskiy Farm south barn",
            StaticCoverKind::FarmBuilding,
            [488.0, 470.0],
            [8.0, 3.2, 5.0],
        ),
        grounded_cover(
            heightmap,
            "oktyabrskiy_barn_north",
            "Oktyabrskiy Farm north barn",
            StaticCoverKind::FarmBuilding,
            [488.0, 530.0],
            [8.0, 3.2, 5.0],
        ),
        grounded_cover(
            heightmap,
            "rail_crossing_cover_west",
            "western crossing log cover",
            StaticCoverKind::RailCover,
            [250.0, 500.0],
            [13.0, 1.4, 3.0],
        ),
        grounded_cover(
            heightmap,
            "rail_crossing_cover_east",
            "eastern crossing log cover",
            StaticCoverKind::RailCover,
            [750.0, 500.0],
            [13.0, 1.4, 3.0],
        ),
        grounded_cover(
            heightmap,
            "psel_treeline_south",
            "Psel treeline screen (south)",
            StaticCoverKind::TreeLine,
            [130.0, 350.0],
            [22.0, 5.0, 3.0],
        ),
        grounded_cover(
            heightmap,
            "psel_treeline_north",
            "Psel treeline screen (north)",
            StaticCoverKind::TreeLine,
            [130.0, 650.0],
            [22.0, 5.0, 3.0],
        ),
        grounded_cover(
            heightmap,
            "hill_wreck_south",
            "south hill knocked-out tank",
            StaticCoverKind::Wreck,
            [700.0, 360.0],
            [3.4, 1.6, 6.2],
        ),
        grounded_cover(
            heightmap,
            "hill_wreck_north",
            "north hill knocked-out tank",
            StaticCoverKind::Wreck,
            [700.0, 640.0],
            [3.4, 1.6, 6.2],
        ),
    ]
}
