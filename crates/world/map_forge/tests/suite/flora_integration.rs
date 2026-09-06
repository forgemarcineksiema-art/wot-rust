//! Procedural-flora map-integration locks (Świat 2.0, 2026-08-06): imported CC0 flora is
//! OUT — every battlefield tree is a `world_forge` species. Retired kinds
//! (`map_forge::RETIRED_KINDS`: the imports, the willow, the pine) keep their wire identity
//! (append-only enum) but are never authored; the oak scatters that replace the hero oak
//! carry the same seeds, and their trunks are still gameplay solids.

use map_forge::blueprint::{MapBlueprint, SceneryOp};
use map_forge::blueprint_for;
use terrain::{MapId, SceneryKind};

fn kind(op: &SceneryOp) -> SceneryKind {
    match op {
        SceneryOp::Scatter { kind, .. }
        | SceneryOp::Row { kind, .. }
        | SceneryOp::Fixed { kind, .. } => *kind,
    }
}

fn scatter_pairs(blueprint: &MapBlueprint, wanted: SceneryKind) -> usize {
    blueprint
        .scenery
        .iter()
        .filter_map(|op| match op {
            SceneryOp::Scatter { kind, pairs, .. } if *kind == wanted => Some(*pairs),
            _ => None,
        })
        .sum()
}

#[test]
fn every_shipped_map_scatters_procedural_oaks() {
    // The original hero-oak replacements stay three pairs a scatter. Orliny and Mazurski
    // carry denser belts that were pine until 2026-09-04 (the owner: "the pine is out
    // entirely") and are oak now — same seeds and counts, a species swap, not a density
    // change. The boulevard avenue stays shallow because its trees stand metres from the
    // camera. The instanced LOD ladder in `scene_build::tree_lod` draws them.
    let expected_pairs = [
        (MapId::ProkhorovkaHill252_2, 9),
        (MapId::BystraValley, 9),
        (MapId::OrlinyPereval, 16),
        (MapId::Ostrogorsk, 9),
        (MapId::MazurskiPrzesmyk, 14),
    ];

    for (map_id, tree_pairs) in expected_pairs {
        let blueprint = blueprint_for(map_id);
        assert_eq!(
            scatter_pairs(&blueprint, SceneryKind::Oak),
            tree_pairs,
            "{map_id:?}: oak scatter drifted"
        );
        assert!(
            tree_pairs > 0,
            "{map_id:?}: every shipped map must exercise the procedural oak + trunk-cover runtime"
        );
        assert!(
            blueprint.scenery.iter().all(|op| !map_forge::RETIRED_KINDS.contains(&kind(op))),
            "{map_id:?}: retired kinds must never be authored"
        );
    }
}

#[test]
fn ostrogorsk_avenue_is_an_oak_row_within_the_measured_depth() {
    // The boulevard avenue survives as a FORM — two flanking rows in their authored lanes —
    // but its DEPTH stays a budget decision: these trees stand metres from the camera, where
    // a tree draws its full near mesh and costs fill, not triangles.
    const MEASURED_MAX_DEPTH: u32 = 3;
    let blueprint = blueprint_for(MapId::Ostrogorsk);

    let avenue = blueprint
        .scenery
        .iter()
        .find_map(|op| match op {
            SceneryOp::Row { kind: SceneryKind::Oak, xs, count, .. } if xs == &[448.0, 472.0] => {
                Some(*count)
            }
            _ => None,
        })
        .expect("the boulevard keeps its flanking oak rows");
    assert!(
        avenue <= MEASURED_MAX_DEPTH,
        "the avenue outgrew what the min spec affords: {avenue} deep per lane"
    );
}
