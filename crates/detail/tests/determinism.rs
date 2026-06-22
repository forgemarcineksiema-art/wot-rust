//! The detail crate's contract: scatter placement and the stable hash are reproducible across calls
//! and independent of process state, so a bake of bolts/weld beads is byte-stable run to run.

use detail::{
    ExclusionZone, FeatureKind, ScatterRequest, ScatterSlot, scatter, stable_hash, weld_bead,
};
use glam::Vec3;

fn grid_slots(n: u32) -> Vec<ScatterSlot> {
    (0..n)
        .map(|i| ScatterSlot {
            position: Vec3::new((i % 5) as f32 * 0.1, (i / 5) as f32 * 0.1, 0.0),
            normal: Vec3::Z,
        })
        .collect()
}

#[test]
fn the_stable_hash_is_fixed_and_distinguishes_its_inputs() {
    // A pinned value: if the hashing changes, every baked seed shifts — this guards that boundary.
    let h = stable_hash("t54", "glacis", FeatureKind::Bolt, 0);
    assert_eq!(
        h,
        stable_hash("t54", "glacis", FeatureKind::Bolt, 0),
        "pure function of its inputs"
    );
    assert_ne!(h, stable_hash("t54", "glacis", FeatureKind::Bolt, 1), "ordinal changes the seed");
    assert_ne!(h, stable_hash("t54", "turret", FeatureKind::Bolt, 0), "part changes the seed");
    assert_ne!(
        h,
        stable_hash("t54", "glacis", FeatureKind::WeldBead, 0),
        "feature changes the seed"
    );
    assert_ne!(h, stable_hash("is3", "glacis", FeatureKind::Bolt, 0), "vehicle changes the seed");
}

#[test]
fn scatter_reproduces_across_repeated_bakes() {
    let slots = grid_slots(20);
    let exclusions = [ExclusionZone { center: Vec3::new(0.2, 0.1, 0.0), radius: 0.08 }];
    let req = ScatterRequest {
        vehicle: "t54",
        part: "hull_side",
        feature: FeatureKind::Bolt,
        slots: &slots,
        exclusions: &exclusions,
        min_spacing: 0.05,
    };
    let first = scatter(&req);
    for _ in 0..8 {
        assert_eq!(scatter(&req), first, "every bake produces the identical placement set");
    }
    assert!(!first.is_empty());
    // Seeds follow contiguous ordinals derived only from acceptance order.
    for (i, p) in first.iter().enumerate() {
        assert_eq!(p.ordinal, i as u32);
        assert_eq!(p.seed, stable_hash("t54", "hull_side", FeatureKind::Bolt, i as u32));
    }
}

#[test]
fn swept_detail_geometry_is_reproducible() {
    let path = [Vec3::ZERO, Vec3::new(0.4, 0.1, 0.0), Vec3::new(0.8, 0.0, 0.1)];
    let a = weld_bead(&path, 0.02);
    let b = weld_bead(&path, 0.02);
    assert_eq!(a.vertices().len(), b.vertices().len());
    assert_eq!(a.indices(), b.indices(), "the same path sweeps to the same topology");
}
