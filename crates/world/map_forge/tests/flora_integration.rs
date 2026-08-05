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
    // The hero oak is a landmark, not a forest, and the density is MEASURED, not taste: on
    // the min spec (MX330, Ostrogorsk avenue view, flora_frame_probe) one baked instance
    // costs ~0.26 ms/frame against ~5 ms of headroom over the 11.5 ms tree-less baseline.
    // That buys ~14 trees per map — hence one mirrored pair per scatter and a two-deep
    // avenue. The ceiling lifts when trees move to the instanced path with runtime LOD.
    let expected_pairs = [
        (MapId::ProkhorovkaHill252_2, 3),
        (MapId::BystraValley, 3),
        (MapId::OrlinyPereval, 2),
        (MapId::Ostrogorsk, 3),
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
                    *pairs <= 2,
                    "{map_id:?}: a hero-oak scatter outgrew the sparse cap ({pairs} pairs)"
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
    // The boulevard avenue survives the hero-flora program as a FORM — two flanking rows in
    // their authored lanes — but its DEPTH is now a budget decision: sixteen deep (64 trees
    // after mirroring) measured +19.97 ms/frame on the min spec, four times the headroom.
    const MEASURED_MAX_DEPTH: u32 = 4;
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
