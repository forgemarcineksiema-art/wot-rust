//! The reference-outline gate (Inny Poziom K0): a vehicle's bake against the dossier's drawing,
//! view by view, by intersection-over-union.
//!
//! `dimension_gate.rs` pins seventeen lengths; this file pins the shape between them. The same
//! FLOOR/TARGET mechanism applies: a `Locked` outline fails the gate under its `min_iou`, a
//! `Target` outline reports as debt — and every outline authored from tables and blueprint
//! extents starts as `Target`, because the instrument must not be calibrated on the thing it
//! measures. The overlay PNG (`cargo run -p tools -- outline-overlay --vehicle t54-1951 --out
//! target/forge/outlines`) is what the owner compares with the drawing before a view flips.

use game_core::VehicleKind;
use glam::Vec2;
use vehicle_forge::{
    AnchorStatus, OUTLINE_CELL_M, OutlineSpec, OutlineView, ReferencePack,
    authoritative_baked_vehicle, composed_triangles_for, measure_outline,
};

#[test]
fn every_locked_outline_holds_on_the_authoritative_bake() {
    let mut gated = 0;
    let mut debts = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let Some(pack) = ReferencePack::for_vehicle(kind) else { continue };
        if pack.outlines().is_empty() {
            continue;
        }
        let baked = authoritative_baked_vehicle(kind).expect("authoritative bake");
        let report = pack.measure_outlines(&baked);
        assert_eq!(report.len(), pack.outlines().len(), "{kind:?}: every view measured");
        for m in &report {
            assert!(m.iou().is_finite(), "{kind:?} {}: the instrument broke", m.view().label());
            match m.status() {
                AnchorStatus::Locked => assert!(
                    m.passed(),
                    "{kind:?}: {} — the bake left the drawing",
                    m.summary_line(&format!("{kind:?}"))
                ),
                AnchorStatus::Target => {
                    assert!(
                        m.holds_floor(),
                        "{kind:?}: {} — the bake moved AWAY from the drawing since the floor                          was measured; a construction PR that changes the silhouette re-measures                          the floor in `outlines/<slug>.outline.ron` with the number in its message",
                        m.summary_line(&format!("{kind:?}"))
                    );
                    if !m.passed() {
                        debts.push(m.summary_line(&format!("{kind:?}")));
                    }
                }
            }
        }
        gated += 1;
    }
    assert!(gated >= 1, "at least the T-54 pilot carries outlines — the gate must never go silent");
    for line in &debts {
        println!("OUTLINE DEBT: {line}");
    }
}

#[test]
fn the_t54_pilot_carries_all_three_views_and_reads_close_to_its_own_tables() {
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 pack");
    let views: Vec<OutlineView> = pack.outlines().iter().map(OutlineSpec::view).collect();
    for view in OutlineView::ALL {
        assert!(views.contains(&view), "the pilot outlines the {} view", view.label());
    }
    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("bake");
    for m in pack.measure_outlines(&baked) {
        println!("{}", m.summary_line("T54_1951"));
        // The outline was drawn from the dossier's numbers and the blueprint's extents: it must
        // already agree with the bake to a degree only a real shape can (a box would not — see
        // below), while the bar itself waits for the traced drawing.
        assert!(
            m.iou() >= 0.80,
            "{}: the tables and the bake disagree by more than a drawing could explain",
            m.summary_line("T54_1951")
        );
    }
}

/// The instrument discriminates: the bake's own bounding box is not the bake.
#[test]
fn a_bounding_box_outline_scores_well_under_the_bar() {
    let baked = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("bake");
    let tris = composed_triangles_for(&baked);
    for view in OutlineView::ALL {
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for tri in &tris {
            for p in tri {
                let q = view.project(*p);
                min = min.min(q);
                max = max.max(q);
            }
        }
        // One cell of slack: a boundary vertex claims the cell it lands in, whose centre can
        // sit just outside an exact box.
        let (min, max) = (min - Vec2::splat(OUTLINE_CELL_M), max + Vec2::splat(OUTLINE_CELL_M));
        let spec = OutlineSpec::new(
            view,
            vec![vec![[min.x, min.y], [max.x, min.y], [max.x, max.y], [min.x, max.y]]],
            0.95,
            AnchorStatus::Locked,
            vehicle_forge::ReferenceSource::new("box", "", "the bake's own bounds"),
        );
        let m = measure_outline(&tris, &spec);
        assert!(
            m.iou() < 0.90,
            "{}: a box scored {:.3} — the metric cannot tell a tank from its crate",
            view.label(),
            m.iou()
        );
        // Inside its own box up to the cells a boundary vertex claims (centre sampling).
        assert!(m.bake_inside() > 0.995, "the bake lies inside its box: {}", m.bake_inside());
    }
}

#[test]
fn the_outline_file_names_the_vehicle_it_belongs_to() {
    let set = vehicle_forge::t54_outline_set();
    assert_eq!(set.vehicle(), VehicleKind::T54_1951.slug());
    assert_eq!(set.views().len(), 3);
    for spec in set.views() {
        assert_eq!(spec.status(), AnchorStatus::Target, "traced against R1 before it locks");
        assert_eq!(spec.source().url(), "docs/vehicles/t-54.md");
        assert!(
            spec.floor_iou().is_some(),
            "{}: a Target view carries its floor",
            spec.view().label()
        );
    }
}
