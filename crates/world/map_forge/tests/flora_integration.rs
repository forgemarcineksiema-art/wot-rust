//! Imported Flora 2.0 map-integration locks (FL-5): accepted textured trees are authored
//! through deterministic map ops, while the rejected bush asset never reaches a battlefield.

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
fn accepted_imported_flora_is_scattered_across_every_shipped_map() {
    let expected_pairs = [
        (MapId::ProkhorovkaHill252_2, 18, 0),
        (MapId::BystraValley, 50, 0),
        (MapId::OrlinyPereval, 24, 40),
        (MapId::Ostrogorsk, 25, 0),
    ];

    for (map_id, tree_pairs, pine_pairs) in expected_pairs {
        let blueprint = blueprint_for(map_id);
        assert_eq!(
            scatter_pairs(&blueprint, SceneryKind::FloraTree),
            tree_pairs,
            "{map_id:?}: textured broadleaf scatter drifted"
        );
        assert_eq!(
            scatter_pairs(&blueprint, SceneryKind::FloraPine),
            pine_pairs,
            "{map_id:?}: textured pine scatter drifted"
        );
        assert!(
            tree_pairs + pine_pairs > 0,
            "{map_id:?}: every shipped map must exercise the imported-flora runtime"
        );
        assert!(
            blueprint.scenery.iter().all(|op| kind(op) != SceneryKind::FloraBush),
            "{map_id:?}: the rejected bush source must never be authored"
        );
    }
}

#[test]
fn ostrogorsk_avenue_and_elevator_pair_use_accepted_imports() {
    let blueprint = blueprint_for(MapId::Ostrogorsk);

    assert!(blueprint.scenery.iter().any(|op| matches!(
        op,
        SceneryOp::Row {
            kind: SceneryKind::FloraTree,
            xs,
            count: 16,
            ..
        } if xs == &[448.0, 472.0]
    )));
    assert!(blueprint.scenery.iter().any(|op| matches!(
        op,
        SceneryOp::Fixed {
            kind: SceneryKind::FloraPine,
            spots,
            ..
        } if spots == &[[900.0, 215.0], [930.0, 250.0]]
    )));
}
