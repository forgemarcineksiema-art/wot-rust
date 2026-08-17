//! Immersja A2.1 locks: a planted row is a row of PLANTS and a mirror twin is its own
//! plant — while a pair's SCALE stays shared, because an Oak's trunk becomes a cover box
//! scaled by the instance and fairness demands identical twins. Determinism of the whole
//! compile is already locked by `goldens.rs`; these lock the VARIETY the compile carries.

use map_forge::{battlefield, blueprint_for};
use terrain::{MapId, SceneryKind};

fn angular_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d)
}

/// The 24 Ostrogorsk lampposts used to be literal clones (one scale, one yaw for the whole
/// row). Now every pair is cast with its own scale and every column turns its own few
/// degrees — while the arm stays over the road, never across it.
#[test]
fn a_row_of_lampposts_is_cast_one_by_one_and_the_arms_stay_on_the_road() {
    let map = battlefield(MapId::Ostrogorsk);
    let lamps: Vec<_> = map.scenery.iter().filter(|s| s.kind == SceneryKind::Lamppost).collect();
    assert!(lamps.len() >= 12, "the avenue keeps its lampposts, got {}", lamps.len());

    let mut scales: Vec<u32> = lamps.iter().map(|s| s.scale.to_bits()).collect();
    scales.sort_unstable();
    scales.dedup();
    assert!(scales.len() >= 4, "a row must carry distinct castings, got {}", scales.len());

    let mut yaws: Vec<u32> = lamps.iter().map(|s| s.yaw_rad.to_bits()).collect();
    yaws.sort_unstable();
    yaws.dedup();
    assert!(yaws.len() >= 4, "every column turns its own degrees, got {}", yaws.len());

    // The arm stays over the road: every compiled lamppost sits within the jitter band
    // (±0.175 rad) of SOME authored lamppost yaw (or its mirror) from the blueprint.
    let blueprint = blueprint_for(MapId::Ostrogorsk);
    let authored: Vec<f32> = blueprint
        .scenery
        .iter()
        .filter_map(|op| match op {
            map_forge::blueprint::SceneryOp::Row { kind, yaw_rad, .. }
            | map_forge::blueprint::SceneryOp::Fixed { kind, yaw_rad, .. }
                if *kind == SceneryKind::Lamppost =>
            {
                Some(*yaw_rad)
            }
            _ => None,
        })
        .collect();
    assert!(!authored.is_empty(), "Ostrogorsk authors its lampposts");
    for lamp in &lamps {
        let closest = authored
            .iter()
            .flat_map(|y| [*y, -*y])
            .map(|y| angular_distance(lamp.yaw_rad, y))
            .fold(f32::MAX, f32::min);
        assert!(
            closest <= 0.18,
            "a lamppost arm never swings off its road: {} rad from every authored yaw",
            closest
        );
    }
}

/// Scattered oaks: the mirrored twin keeps the pair's scale (the trunk cover box under it
/// is scaled by the instance — the two halves must play identically) but grows its own
/// yaw — the old `-yaw` reflection that stamped one forest twice is retired.
#[test]
fn oak_twins_share_scale_for_fair_trunks_but_pose_as_their_own_trees() {
    for id in [MapId::BystraValley, MapId::ProkhorovkaHill252_2] {
        let map = battlefield(id);
        let oaks: Vec<_> = map.scenery.iter().filter(|s| s.kind == SceneryKind::Oak).collect();
        if oaks.len() < 4 {
            continue;
        }
        // A pair shares its exact f32 scale; different pairs practically never collide.
        let mut by_scale: std::collections::HashMap<u32, Vec<&terrain::SceneryInstance>> =
            std::collections::HashMap::new();
        for oak in &oaks {
            by_scale.entry(oak.scale.to_bits()).or_default().push(oak);
        }
        let mut checked_pairs = 0;
        for group in by_scale.values().filter(|g| g.len() == 2) {
            let (a, b) = (group[0], group[1]);
            assert_eq!(a.scale.to_bits(), b.scale.to_bits(), "twins share the casting");
            assert_ne!(
                a.yaw_rad.to_bits(),
                b.yaw_rad.to_bits(),
                "{id:?}: a twin poses as its own tree"
            );
            assert_ne!(
                a.yaw_rad.to_bits(),
                (-b.yaw_rad).to_bits(),
                "{id:?}: the -yaw reflection is retired"
            );
            checked_pairs += 1;
        }
        assert!(checked_pairs >= 2, "{id:?}: the lock must actually see pairs");
    }
}
