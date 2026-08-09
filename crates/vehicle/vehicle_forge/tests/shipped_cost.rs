//! The cost contract on the geometry the GAME ships.
//!
//! `vehicle_recipes::tests::vehicle_budgets` bounds the procedural bake, but — exactly like the
//! mesh-quality gate next door — it cannot see `vehicle_build`, so the one vehicle that does NOT
//! bake through those recipes sat outside every fleet-level COST gate. On triangles that was
//! merely untidy: `vehicle_build`'s own tests cap the hybrid at 26,000. On VERTICES it was a real
//! hole. The procedural fleet has been capped at 11,000 vertices for as long as the table has
//! existed; the shipped T-54 measures 16,705, half again over that cap, and no test in the
//! workspace looked at the number.
//!
//! Keep both gates, for the reason `shipped_mesh_quality.rs` gives: the procedural one catches a
//! recipe regression on a vehicle the seam does not route yet, this one catches the source that
//! actually reaches the screen.

use game_core::VehicleKind;
use vehicle_forge::{CostEnvelope, authoritative_baked_vehicle, shipped_cost_ceiling};

fn tris_and_verts(kind: VehicleKind) -> (usize, usize) {
    let baked = authoritative_baked_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} bakes: {e}"));
    let tris = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();
    let verts = baked.submeshes().iter().map(|s| s.mesh.vertex_count()).sum();
    (tris, verts)
}

/// Every vehicle the game ships stays inside the envelope its own bake path sets.
///
/// Printed, not just asserted: re-measuring after a construction pass is the point of this
/// instrument, and `cargo test -p vehicle_forge --test shipped_cost -- --nocapture` is how the
/// numbers written beside the budget constants are taken.
#[test]
fn every_shipped_vehicle_stays_inside_its_cost_envelope() {
    for kind in VehicleKind::PLAYABLE {
        let (tris, verts) = tris_and_verts(kind);
        let ceiling = shipped_cost_ceiling(kind);
        eprintln!(
            "{kind:?}: {tris} tris / {}, {verts} verts / {}  [{:?}]",
            ceiling.tri_max, ceiling.vert_max, ceiling.envelope
        );

        assert!(
            tris <= ceiling.tri_max,
            "shipped {kind:?}: {tris} triangles over its {:?} ceiling of {} — raise the budget \
             with the measurement that justifies it, or give the triangles back",
            ceiling.envelope,
            ceiling.tri_max
        );
        assert!(
            verts <= ceiling.vert_max,
            "shipped {kind:?}: {verts} vertices over its {:?} ceiling of {} — vertices split on \
             smoothing groups, materials and UV seams, so this can break while triangles read \
             green",
            ceiling.envelope,
            ceiling.vert_max
        );
    }
}

/// The seam this file exists for: the fleet must actually contain both envelopes.
///
/// If every vehicle resolved to the same envelope, the routing above would be untested scaffolding
/// and the gate would silently become "the procedural table, applied to everything" — which is the
/// state that let the hybrid's vertex count go unmeasured in the first place.
#[test]
fn the_fleet_exercises_both_cost_envelopes() {
    let envelopes: Vec<CostEnvelope> =
        VehicleKind::PLAYABLE.into_iter().map(|k| shipped_cost_ceiling(k).envelope).collect();

    assert!(
        envelopes.contains(&CostEnvelope::HybridClass),
        "no vehicle routes to the hybrid envelope — the benchmark vehicle stopped being hybrid, \
         or the seam broke"
    );
    assert!(
        envelopes.contains(&CostEnvelope::ProceduralFleet),
        "no vehicle routes to the procedural envelope"
    );
}

/// The two envelopes must stay far apart, in the direction that makes them two envelopes.
///
/// A hybrid held to the lean recipe's numbers would fail instantly; a procedural vehicle handed
/// the hybrid's would lose its gate entirely. If these ever converge, one of them is wrong.
#[test]
fn the_hybrid_envelope_is_the_generous_one() {
    let hybrid = shipped_cost_ceiling(VehicleKind::T54_1951);
    let procedural = shipped_cost_ceiling(VehicleKind::T34_85);

    assert_eq!(hybrid.envelope, CostEnvelope::HybridClass, "the T-54 is the hybrid benchmark");
    assert_eq!(procedural.envelope, CostEnvelope::ProceduralFleet);
    assert!(
        hybrid.tri_max > procedural.tri_max * 4,
        "the hybrid carries multiples more geometry by design: {} vs {}",
        hybrid.tri_max,
        procedural.tri_max
    );
    assert!(hybrid.vert_max > procedural.vert_max, "and more vertices with it");
}
