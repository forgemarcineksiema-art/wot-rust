//! The steppe's dressing: what grows on the Prokhorovka grassland. All render-only (see
//! `scenery`), all mirrored, all seeded — the honest LOS blockers remain the TreeLine cover
//! boxes; these bushes and trees only make the open field read as a field instead of a lawn.
//!
//! The planting plan follows the map's anatomy: steppe bushes scattered across the rolling
//! grass (thinning out in the deliberately barren western killzone), a handful of oaks
//! around the Oktyabrskiy farm, in-fill inside the Psel treeline screens, and boulders on
//! the hill shoulders.

use crate::prokhorovka::HALF_M;
use crate::scenery::{
    ScatterRegion, SceneryInstance, SceneryKind, inside_any_cover, scatter_mirrored,
};
use crate::{HeightMap, Road, StaticCoverObject};

const SCENERY_SEED: u64 = 0x9C0B_252A;

pub(crate) fn steppe_scenery(
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
    roads: &[Road],
) -> Vec<SceneryInstance> {
    let mut out = Vec::new();
    // The shared refusal rule: nothing grows on the embankment or its crossing lanes, inside
    // the spawn circles, on a road, or through a cover footprint.
    let exclude = |x: f32, z: f32| -> bool {
        // The railbed and its shoulders stay bare so the wall and its gaps read clean.
        if (z - HALF_M).abs() < 22.0 {
            return true;
        }
        // Spawn circles (mirrored pair on the axis at z = 150 / 850).
        let spawn_south = (x - HALF_M).powi(2) + (z - (HALF_M - 350.0)).powi(2);
        let spawn_north = (x - HALF_M).powi(2) + (z - (HALF_M + 350.0)).powi(2);
        if spawn_south < 80.0 * 80.0 || spawn_north < 80.0 * 80.0 {
            return true;
        }
        if roads.iter().any(|road| road.distance_to(x, z) < road.width_m * 0.5 + 2.0) {
            return true;
        }
        inside_any_cover(cover, x, z, 2.5)
    };

    // Steppe bushes over the rolling grass: the eastern and central bands carry the bulk;
    // the western Psel field stays nearly bare — it is designed as a coverless killzone and
    // the dressing must not suggest otherwise.
    let bush_bands: [(ScatterRegion, usize); 3] = [
        (ScatterRegion { x: (340.0, 640.0), z: (60.0, 470.0) }, 42),
        (ScatterRegion { x: (640.0, 950.0), z: (60.0, 470.0) }, 48),
        (ScatterRegion { x: (240.0, 340.0), z: (60.0, 470.0) }, 8),
    ];
    for (band_index, (band, pairs)) in bush_bands.into_iter().enumerate() {
        scatter_mirrored(
            SCENERY_SEED.wrapping_add(band_index as u64),
            SceneryKind::Bush,
            pairs,
            band,
            HALF_M,
            heightmap,
            &exclude,
            &mut out,
        );
    }

    // A few farm oaks around Oktyabrskiy (the barns sit at x≈488, z≈470/530).
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(10),
        SceneryKind::Oak,
        5,
        ScatterRegion { x: (430.0, 545.0), z: (420.0, 466.0) },
        HALF_M,
        heightmap,
        &exclude,
        &mut out,
    );
    // Treeline in-fill: the Psel screens' solid boxes get the trees their volume stands for.
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(11),
        SceneryKind::Oak,
        7,
        ScatterRegion { x: (110.0, 150.0), z: (348.0, 352.0) },
        HALF_M,
        heightmap,
        &|_, _| false,
        &mut out,
    );
    // Boulders on the hill shoulders.
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(12),
        SceneryKind::Rock,
        6,
        ScatterRegion { x: (680.0, 920.0), z: (240.0, 430.0) },
        HALF_M,
        heightmap,
        &exclude,
        &mut out,
    );
    out
}
