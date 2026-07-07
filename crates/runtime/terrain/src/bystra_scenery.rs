//! The valley's dressing: where things grow in Dolina Bystrej. All render-only (see
//! `scenery`), all mirrored, all seeded — the same map generates the same trees everywhere.
//!
//! The planting plan follows the map's anatomy: willows lean over the riverbanks just outside
//! the corridor, poplar rows mark the western field lanes, oaks dot the farmland, fruit trees
//! in-fill the orchard screens and the town gardens, and the hedgerow `TreeLine` boxes get the
//! visual trees their solid gameplay volume stands for.

use crate::bystra::{HALF_M, RIVER_CORRIDOR_HALF_WIDTH_M, bystra_river_center_x};
use crate::scenery::{
    ScatterRegion, SceneryInstance, SceneryKind, inside_any_cover, scatter_mirrored,
};
use crate::{HeightMap, StaticCoverObject};

const SCENERY_SEED: u64 = 0xB57A_5CE0;

pub(crate) fn valley_scenery(
    heightmap: &HeightMap,
    cover: &[StaticCoverObject],
) -> Vec<SceneryInstance> {
    let mut out = Vec::new();
    // The shared refusal rule: nothing grows in the river or its banks' drive lanes, on the
    // crossing approaches, inside spawn circles, or through a cover footprint.
    let exclude = |x: f32, z: f32| -> bool {
        let river_d = (x - bystra_river_center_x(z)).abs();
        if river_d < RIVER_CORRIDOR_HALF_WIDTH_M + 2.0 {
            return true;
        }
        // Crossing approach lanes stay clear so the drives onto bridge/fords/planks read open.
        if river_d < RIVER_CORRIDOR_HALF_WIDTH_M + 30.0 {
            let dz = z - HALF_M;
            if dz.abs() < 16.0 || (dz.abs() - 180.0).abs() < 26.0 || (dz.abs() - 320.0).abs() < 14.0
            {
                return true;
            }
        }
        // Spawn circles (mirrored pair at (400, 150)/(400, 850)).
        let spawn_south = (x - 400.0).powi(2) + (z - 150.0).powi(2);
        let spawn_north = (x - 400.0).powi(2) + (z - 850.0).powi(2);
        if spawn_south < 70.0 * 70.0 || spawn_north < 70.0 * 70.0 {
            return true;
        }
        inside_any_cover(cover, x, z, 2.5)
    };

    // Willows on the banks: a narrow band hugging the corridor on both sides of the river.
    let willow_bands = [
        ScatterRegion { x: (470.0, 545.0), z: (60.0, 470.0) },
        ScatterRegion { x: (600.0, 660.0), z: (60.0, 470.0) },
    ];
    for (band_index, band) in willow_bands.into_iter().enumerate() {
        scatter_mirrored(
            SCENERY_SEED.wrapping_add(band_index as u64),
            SceneryKind::Willow,
            14,
            band,
            HALF_M,
            heightmap,
            &|x, z| {
                let river_d = (x - bystra_river_center_x(z)).abs();
                // Willows WANT the bank: reject anything not adjacent to the corridor.
                exclude(x, z) || river_d > RIVER_CORRIDOR_HALF_WIDTH_M + 16.0
            },
            &mut out,
        );
    }

    // Field oaks over the western farmland.
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(10),
        SceneryKind::Oak,
        22,
        ScatterRegion { x: (60.0, 460.0), z: (60.0, 460.0) },
        HALF_M,
        heightmap,
        &exclude,
        &mut out,
    );
    // Boulders around the knolls and the upland.
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(11),
        SceneryKind::Rock,
        10,
        ScatterRegion { x: (40.0, 260.0), z: (120.0, 470.0) },
        HALF_M,
        heightmap,
        &exclude,
        &mut out,
    );
    // Orchard in-fill: fruit trees inside the orchard screens' footprint (the solid TreeLine
    // keeps the gameplay volume; these give it leaves).
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(12),
        SceneryKind::FruitTree,
        16,
        ScatterRegion { x: (608.0, 648.0), z: (278.0, 322.0) },
        HALF_M,
        heightmap,
        &|x, z| inside_spawn_or_river(x, z),
        &mut out,
    );
    // Hedgerow in-fill for the field hedgerow screens.
    scatter_mirrored(
        SCENERY_SEED.wrapping_add(13),
        SceneryKind::Oak,
        12,
        ScatterRegion { x: (272.0, 328.0), z: (232.0, 248.0) },
        HALF_M,
        heightmap,
        &|x, z| inside_spawn_or_river(x, z),
        &mut out,
    );

    // Poplar rows along the western field lanes: deterministic rows, not scatter — windbreaks
    // are planted, not grown.
    for step in 0..7 {
        let z = 260.0 + step as f32 * 26.0;
        for x in [168.0, 194.0] {
            push_mirrored_fixed(&mut out, heightmap, SceneryKind::Poplar, x, z, 1.0);
        }
    }
    // Town gardens: a few fruit trees in the block gaps (fixed spots clear of the houses).
    for (x, z) in [(712.0, 440.0), (752.0, 438.0), (794.0, 442.0), (668.0, 462.0)] {
        push_mirrored_fixed(&mut out, heightmap, SceneryKind::FruitTree, x, z, 0.95);
    }
    out
}

/// The in-fill scatters run inside TreeLine footprints, so the cover exclusion must not apply
/// — only the hard refusals (river, spawns) do.
fn inside_spawn_or_river(x: f32, z: f32) -> bool {
    let river_d = (x - bystra_river_center_x(z)).abs();
    river_d < RIVER_CORRIDOR_HALF_WIDTH_M + 2.0
        || (x - 400.0).powi(2) + (z - 150.0).powi(2) < 70.0 * 70.0
        || (x - 400.0).powi(2) + (z - 850.0).powi(2) < 70.0 * 70.0
}

fn push_mirrored_fixed(
    out: &mut Vec<SceneryInstance>,
    heightmap: &HeightMap,
    kind: SceneryKind,
    x: f32,
    z: f32,
    scale: f32,
) {
    for (z, yaw) in [(z, 0.35), (HALF_M * 2.0 - z, -0.35)] {
        if let Some(ground) = heightmap.sample_height(x, z) {
            out.push(SceneryInstance { kind, position: [x, ground, z], yaw_rad: yaw, scale });
        }
    }
}
