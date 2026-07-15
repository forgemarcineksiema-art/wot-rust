use crate::bystra::{HALF_M, KNOLL_X_M, WINDMILL_X_M, bystra_river_center_x};
use crate::map_build::grounded_cover;
use crate::{HeightMap, StaticCoverKind, StaticCoverObject};

/// Kamienna's block grid: column x-positions (west→east up the bench) and mirrored row
/// offsets from the axis. The gap inside `|dz| < TOWN_ROAD_HALF_M` is the bridge road and the
/// market square — the town's central corridor stays drivable by construction.
const TOWN_COLUMNS_X_M: [f32; 4] = [690.0, 732.0, 774.0, 816.0];
const TOWN_ROW_OFFSETS_M: [f32; 3] = [38.0, 82.0, 126.0];

pub(crate) fn valley_cover_objects(heightmap: &HeightMap) -> Vec<StaticCoverObject> {
    // Mirror-symmetric cover: every object is on-axis or a north/south pair, sitting on the
    // symmetric skeleton from `bystra.rs`, so both halves fight the same town.
    let mut objects = vec![
        grounded_cover(
            heightmap,
            "kamienna_church",
            "Kamienna church",
            StaticCoverKind::FarmBuilding,
            [698.0, HALF_M],
            [7.0, 6.5, 9.0],
        ),
        grounded_cover(
            heightmap,
            "bystra_mill_south",
            "riverside mill (south wing)",
            StaticCoverKind::FarmBuilding,
            [620.0, HALF_M - 22.0],
            [5.0, 3.5, 4.0],
        ),
        grounded_cover(
            heightmap,
            "bystra_mill_north",
            "riverside mill (north wing)",
            StaticCoverKind::FarmBuilding,
            [620.0, HALF_M + 22.0],
            [5.0, 3.5, 4.0],
        ),
        // The mill-yard fences (Fizyczny Świat P10): a wooden run at each mill, mirror-paired.
        grounded_cover(
            heightmap,
            "bystra_mill_fence_south",
            "Bystra mill south yard fence",
            StaticCoverKind::WoodenFence,
            [611.0, HALF_M - 22.0],
            [0.25, 0.65, 6.0],
        ),
        grounded_cover(
            heightmap,
            "bystra_mill_fence_north",
            "Bystra mill north yard fence",
            StaticCoverKind::WoodenFence,
            [611.0, HALF_M + 22.0],
            [0.25, 0.65, 6.0],
        ),
        grounded_cover(
            heightmap,
            "windmill",
            "the windmill",
            StaticCoverKind::FarmBuilding,
            [WINDMILL_X_M, HALF_M],
            [3.2, 3.0, 3.2],
        ),
        bridge_parapet(heightmap, "bridge_parapet_south", -6.2),
        bridge_parapet(heightmap, "bridge_parapet_north", 6.2),
        grounded_cover(
            heightmap,
            "field_hedgerow_south",
            "field hedgerow screen (south)",
            StaticCoverKind::TreeLine,
            [300.0, HALF_M - 260.0],
            [26.0, 4.5, 3.0],
        ),
        grounded_cover(
            heightmap,
            "field_hedgerow_north",
            "field hedgerow screen (north)",
            StaticCoverKind::TreeLine,
            [300.0, HALF_M + 260.0],
            [26.0, 4.5, 3.0],
        ),
        grounded_cover(
            heightmap,
            "orchard_screen_south",
            "riverside orchard screen (south)",
            StaticCoverKind::TreeLine,
            [628.0, HALF_M - 200.0],
            [18.0, 4.5, 3.0],
        ),
        grounded_cover(
            heightmap,
            "orchard_screen_north",
            "riverside orchard screen (north)",
            StaticCoverKind::TreeLine,
            [628.0, HALF_M + 200.0],
            [18.0, 4.5, 3.0],
        ),
        grounded_cover(
            heightmap,
            "knoll_wall_south",
            "knoll stone wall (south)",
            StaticCoverKind::RailCover,
            [KNOLL_X_M + 30.0, HALF_M - 300.0],
            [12.0, 1.2, 2.2],
        ),
        grounded_cover(
            heightmap,
            "knoll_wall_north",
            "knoll stone wall (north)",
            StaticCoverKind::RailCover,
            [KNOLL_X_M + 30.0, HALF_M + 300.0],
            [12.0, 1.2, 2.2],
        ),
        grounded_cover(
            heightmap,
            "ford_wreck_south",
            "burned-out hull at the southern ford",
            StaticCoverKind::Wreck,
            [505.0, HALF_M - 180.0],
            [3.4, 1.6, 6.2],
        ),
        grounded_cover(
            heightmap,
            "ford_wreck_north",
            "burned-out hull at the northern ford",
            StaticCoverKind::Wreck,
            [505.0, HALF_M + 180.0],
            [3.4, 1.6, 6.2],
        ),
    ];
    objects.extend(town_blocks(heightmap));
    objects
}

/// The town's residential blocks: a deterministic grid of mirrored house pairs whose sizes
/// vary by grid parity so the skyline reads as a town, not a barracks. 4 columns × 3 mirrored
/// rows = 24 houses around the church and the market square.
fn town_blocks(heightmap: &HeightMap) -> Vec<StaticCoverObject> {
    let mut blocks = Vec::new();
    for (column, &x) in TOWN_COLUMNS_X_M.iter().enumerate() {
        for (row, &row_offset) in TOWN_ROW_OFFSETS_M.iter().enumerate() {
            let wide = (column + row) % 2 == 0;
            let half = if wide { [6.5, 3.6, 4.0] } else { [5.5, 3.0, 4.8] };
            for (side, sign) in [("south", -1.0_f32), ("north", 1.0_f32)] {
                blocks.push(grounded_cover(
                    heightmap,
                    &format!("town_house_c{column}_r{row}_{side}"),
                    &format!("Kamienna house (column {column}, row {row}, {side})"),
                    StaticCoverKind::FarmBuilding,
                    [x, HALF_M + sign * row_offset],
                    half,
                ));
            }
        }
    }
    blocks
}

/// A parapet wall running the length of the stone bridge's deck edge. It samples the deck
/// (the causeway IS the heightmap), so the wall rides the bridge, not the riverbed.
fn bridge_parapet(heightmap: &HeightMap, id: &str, dz: f32) -> StaticCoverObject {
    let x = bystra_river_center_x(HALF_M);
    grounded_cover(
        heightmap,
        id,
        "stone bridge parapet",
        StaticCoverKind::RailCover,
        [x, HALF_M + dz],
        [26.0, 0.55, 0.5],
    )
}
