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
        // The farmyards (B4 cz.3): a barn alone is a box in a field — a ZAGRODA is a barn, a
        // cottage and a shed closing a yard. Both halves get the identical cluster, mirrored.
        grounded_cover(
            heightmap,
            "oktyabrskiy_cottage_south",
            "Oktyabrskiy Farm south cottage",
            StaticCoverKind::FarmBuilding,
            [470.0, 458.0],
            [4.0, 2.6, 3.2],
        ),
        grounded_cover(
            heightmap,
            "oktyabrskiy_cottage_north",
            "Oktyabrskiy Farm north cottage",
            StaticCoverKind::FarmBuilding,
            [470.0, 542.0],
            [4.0, 2.6, 3.2],
        ),
        grounded_cover(
            heightmap,
            "oktyabrskiy_shed_south",
            "Oktyabrskiy Farm south shed",
            StaticCoverKind::FarmBuilding,
            [503.0, 455.0],
            [2.6, 2.2, 2.4],
        ),
        grounded_cover(
            heightmap,
            "oktyabrskiy_shed_north",
            "Oktyabrskiy Farm north shed",
            StaticCoverKind::FarmBuilding,
            [503.0, 545.0],
            [2.6, 2.2, 2.4],
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

#[cfg(test)]
mod tests {
    use crate::prokhorovka_hill_252_2;

    /// B4 cz.3: the Oktyabrskiy farm is a ZAGRODA, not a lone box — barn, cottage and shed
    /// close each yard, and both halves get the identical cluster mirrored across z = 500.
    #[test]
    fn the_farmyards_close_their_yards_in_mirror() {
        let map = prokhorovka_hill_252_2();
        for piece in ["barn", "cottage", "shed"] {
            let south = map
                .static_cover
                .iter()
                .find(|c| c.id == format!("oktyabrskiy_{piece}_south"))
                .unwrap_or_else(|| panic!("south {piece} stands"));
            let north = map
                .static_cover
                .iter()
                .find(|c| c.id == format!("oktyabrskiy_{piece}_north"))
                .unwrap_or_else(|| panic!("north {piece} stands"));
            assert_eq!(south.center[0], north.center[0], "{piece}: same x");
            assert!(
                (south.center[2] - (1000.0 - north.center[2])).abs() < 1.0e-3,
                "{piece}: mirrored across the axis"
            );
            assert_eq!(south.half_extents_m, north.half_extents_m, "{piece}: same body");
        }
    }
}
