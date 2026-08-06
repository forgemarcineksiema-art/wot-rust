//! Imported-flora map-integration locks (FL-5, rewritten for the hero-flora program,
//! 2026-08-05): `dab-hero` — the textured hero oak behind `FloraTree` — is the only imported
//! asset, authored SPARSELY through deterministic map ops, and the retired `FloraPine` /
//! `FloraBush` kinds never reach a battlefield again.

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
fn the_hero_oak_is_scattered_sparsely_across_every_shipped_map() {
    // The density is MEASURED, not taste, and the LOD ladder is what moved it: with trees
    // baked into the statics every copy cost ~0.59 ms/frame on the min spec and ten a map was
    // the ceiling. Instanced, a DISTANT tree costs ~0.03 ms — nineteen times less — so the
    // scatters got generous while the boulevard avenue, which stands in the camera's face,
    // stayed shallow. Ostrogorsk measures 26 trees at 15.62 ms of the 16.667 ms gate.
    let expected_pairs = [
        (MapId::ProkhorovkaHill252_2, 9),
        (MapId::BystraValley, 9),
        (MapId::OrlinyPereval, 6),
        (MapId::Ostrogorsk, 9),
    ];

    for (map_id, tree_pairs) in expected_pairs {
        let blueprint = blueprint_for(map_id);
        assert_eq!(
            scatter_pairs(&blueprint, SceneryKind::FloraTree),
            tree_pairs,
            "{map_id:?}: hero-oak scatter drifted"
        );
        assert!(
            tree_pairs > 0,
            "{map_id:?}: every shipped map must exercise the imported-flora runtime"
        );
        for op in &blueprint.scenery {
            if let SceneryOp::Scatter { kind: SceneryKind::FloraTree, pairs, .. } = op {
                assert!(
                    *pairs <= 3,
                    "{map_id:?}: a hero-oak scatter outgrew its measured cap ({pairs} pairs)"
                );
            }
        }
        // Retired with the stylized download pack: the kinds keep their wire identity
        // (append-only enum) but bake to nothing, so an authored instance is a dead entry.
        assert!(
            blueprint
                .scenery
                .iter()
                .all(|op| !matches!(kind(op), SceneryKind::FloraPine | SceneryKind::FloraBush)),
            "{map_id:?}: retired imported kinds must never be authored"
        );
    }
}

#[test]
fn ostrogorsk_avenue_is_a_hero_oak_row_within_the_measured_depth() {
    // The boulevard avenue survives as a FORM — two flanking rows in their authored lanes —
    // but its DEPTH stays a budget decision even after the LOD ladder: these trees stand
    // metres from the camera, where a tree draws its full near mesh and costs fill, not
    // triangles. Sixteen deep measured +19.97 ms/frame on the min spec.
    const MEASURED_MAX_DEPTH: u32 = 3;
    let blueprint = blueprint_for(MapId::Ostrogorsk);

    let avenue = blueprint
        .scenery
        .iter()
        .find_map(|op| match op {
            SceneryOp::Row { kind: SceneryKind::FloraTree, xs, count, .. }
                if xs == &[448.0, 472.0] =>
            {
                Some(*count)
            }
            _ => None,
        })
        .expect("the boulevard keeps its flanking hero-oak rows");
    assert!(
        avenue <= MEASURED_MAX_DEPTH,
        "the avenue outgrew what the min spec affords: {avenue} deep per lane"
    );
}
