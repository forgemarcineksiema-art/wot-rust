//! THE MESH-QUALITY CONTRACT, RUN OVER THE RUNNING GEAR.
//!
//! Two gates already audit vehicle meshes — `fleet_mesh_quality` over the procedural recipes and
//! `shipped_mesh_quality` over what the game draws — and both walk `BakedVehicle::submeshes()`.
//! The running gear is not in there. It is INSTANCED at draw time from unit meshes that live
//! outside the bake, so the wheels, the sprockets, the idlers, the swing arms and every one of
//! the ~180 track links per tank have never been audited by anything: not for degenerate
//! triangles, not for non-manifold edges, not for unit normals.
//!
//! That is the largest single body of geometry in the vehicle (~35k triangles on a T-54, more
//! than twice its baked LOD0) sitting outside every contract the project has.
//!
//! `fleet_running_gear` audits the gear's PLACEMENT (discs must not interpenetrate, the belt must
//! reach the ground); this file audits the gear's MESHES. Both are needed: a perfectly placed
//! wheel can still be a broken surface.

use game_core::VehicleKind;
use vehicle_geometry::{
    GearPart, GeometryMesh, MeshQualityReport, OPEN_OR_CLOSED_MESH, RunningGearKinematics,
    idler_unit_mesh, return_roller_unit_mesh, road_wheel_unit_mesh, running_gear_placements,
    sprocket_unit_mesh, swing_arm_unit_mesh, track_link_unit_mesh,
};

/// Winding debt that exists TODAY, booked in the FLOOR/TARGET form the project uses: the count
/// is a CEILING, so it can only ever shrink, and every entry names its cause.
///
/// **The whole fleet's road wheels and return rollers are wound inconsistently** — 22 edges on a
/// dished wheel, 44 on an openwork (spoked) one, 16 on every return roller, in every vehicle.
/// This is one systematic generator defect, not eight vehicle defects: the wheel is assembled
/// from independently-revolved pieces (ring, web, hub, tyre) plus extruded spoke arms, and the
/// seams between them disagree about which way the surface faces. Nothing had ever audited a
/// gear mesh, so it has been shipping since the wheels were written.
///
/// It is booked rather than fixed here because the fix is a rewrite of the wheel generator, and
/// that rewrite is already scheduled with its own reference dossier (Model Idealny W4/PR-23:
/// twin discs with a real axial gap, dished spider casting, hub bolt circle). Fixing it here
/// would move the whole fleet's geometry twice.
const RECORDED_GEAR_WINDING_CEILING: &[(&str, usize)] =
    &[("road_wheel", 44), ("return_roller", 16)];

fn gear_ceiling(part: &str) -> usize {
    RECORDED_GEAR_WINDING_CEILING
        .iter()
        .find(|(name, _)| *name == part)
        .map_or(0, |(_, allowed)| *allowed)
}

fn audit(mesh: &GeometryMesh, where_: &str) {
    let report: MeshQualityReport = mesh.quality_report(OPEN_OR_CLOSED_MESH);
    assert_eq!(
        report.invalid_indices, 0,
        "{where_}: {} indices point at no vertex",
        report.invalid_indices
    );
    assert_eq!(
        report.non_finite_vertices, 0,
        "{where_}: {} vertices carry NaN/inf data",
        report.non_finite_vertices
    );
    assert_eq!(
        report.non_manifold_edges, 0,
        "{where_}: {} edges are shared by three or more triangles",
        report.non_manifold_edges
    );
    assert_eq!(
        report.degenerate_triangles, 0,
        "{where_}: {} triangles have no area — they shade as black slivers and break tangents",
        report.degenerate_triangles
    );
    assert_eq!(
        report.non_unit_normals, 0,
        "{where_}: {} vertex normals are not unit length",
        report.non_unit_normals
    );
    // Winding is the one class with recorded debt (see the ceiling table above).
    let part = where_.rsplit(' ').next().unwrap_or("");
    let allowed = gear_ceiling(part);
    assert!(
        report.inconsistent_winding_edges <= allowed,
        "{where_}: {} inconsistently wound edges, past its recorded ceiling {allowed} — those \
         faces light backwards, and a wheel is seen from every angle on every frame",
        report.inconsistent_winding_edges
    );
    if report.inconsistent_winding_edges > 0 {
        println!(
            "GEAR MESH DEBT {where_}: {} inconsistently wound edges (ceiling {allowed}, target 0 \
             — Model Idealny W4/PR-23)",
            report.inconsistent_winding_edges
        );
    }
}

/// A ceiling that no longer bites is a lie about the state of the fleet. Every entry must still
/// be reached by at least one vehicle, or it gets deleted.
#[test]
fn every_recorded_gear_ceiling_is_still_earned() {
    for (part, allowed) in RECORDED_GEAR_WINDING_CEILING {
        let mut worst = 0;
        for kind in VehicleKind::PLAYABLE {
            let Some(kin) = RunningGearKinematics::for_vehicle(kind) else { continue };
            let mesh = match *part {
                "road_wheel" => road_wheel_unit_mesh(&kin),
                "return_roller" => return_roller_unit_mesh(&kin),
                other => panic!("unknown recorded gear part {other}"),
            };
            worst = worst.max(mesh.quality_report(OPEN_OR_CLOSED_MESH).inconsistent_winding_edges);
        }
        assert_eq!(
            worst, *allowed,
            "{part} now measures {worst} inconsistently wound edges, not the recorded {allowed} \
             — lower the ceiling (or delete the entry) in the same commit that improved it"
        );
    }
}

#[test]
fn every_running_gear_unit_mesh_obeys_the_quality_contract() {
    let mut audited = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(kin) = RunningGearKinematics::for_vehicle(kind) else { continue };
        for (name, mesh) in [
            ("road_wheel", road_wheel_unit_mesh(&kin)),
            ("idler", idler_unit_mesh(&kin)),
            ("sprocket", sprocket_unit_mesh(&kin)),
            ("track_link", track_link_unit_mesh(&kin)),
            ("swing_arm", swing_arm_unit_mesh(&kin)),
            ("return_roller", return_roller_unit_mesh(&kin)),
        ] {
            audit(&mesh, &format!("gear {kind:?} {name}"));
            audited += 1;
        }
    }
    assert!(audited >= 40, "the gate must actually cover the fleet's gear (audited {audited})");
}

/// The gear is not just meshes, it is INSTANCES — and the instance count is the thing that makes
/// its cost matter. Lock what each vehicle actually emits so a placement change that doubles the
/// draw list has to say so out loud.
#[test]
fn the_gear_instance_count_is_a_number_someone_chose() {
    for kind in VehicleKind::PLAYABLE {
        let Some(kin) = RunningGearKinematics::for_vehicle(kind) else { continue };
        let placements = running_gear_placements(&kin, 0.0, 0.0);
        assert!(!placements.is_empty(), "{kind:?} has running gear kinematics but no placements");

        // Every placement must name a part the renderer can resolve to a mesh, and both sides
        // must be represented — a belt on one side only is a placement bug no bounds test sees.
        let mut left = 0;
        let mut right = 0;
        for placement in &placements {
            match placement.part {
                GearPart::RoadWheel
                | GearPart::Idler
                | GearPart::Sprocket
                | GearPart::Link
                | GearPart::SwingArm
                | GearPart::ReturnRoller => {}
            }
            if placement.transform.w_axis.x < 0.0 {
                left += 1;
            } else {
                right += 1;
            }
        }
        assert_eq!(
            left, right,
            "{kind:?}: {left} gear instances on the port side vs {right} on the starboard — a \
             tank is symmetric about its centreline"
        );
        assert!(
            placements.len() <= 260,
            "{kind:?} emits {} gear instances per frame — past the point where this needs a \
             deliberate decision (and a measurement) rather than a default",
            placements.len()
        );
    }
}

/// M14, made visible: the collision box is WIDER than the widest thing the player can see.
///
/// The honesty doctrine says the collision box IS the visual footprint — "what blocks the shell
/// blocks the eye". On the T-54 the hitbox reaches ±1.75 m while the outermost geometry (the
/// track belt) reaches ±1.63: about 12% of phantom width that a hull can be rammed or blocked
/// by, and that no one can see.
///
/// This is NOT asserted to zero here on purpose. Narrowing the box changes ramming, terrain
/// contact and spotting all at once, so it belongs with the hull rebuild that already re-authors
/// the hitbox (Model Idealny W3/PR-14), not to a gate PR. What this test does is stop it getting
/// WORSE and put the number on the record every run.
#[test]
fn the_hitbox_does_not_grow_further_past_the_visible_vehicle() {
    /// Recorded 2026-07-29: today's worst offender, as a ceiling only.
    const PHANTOM_WIDTH_CEILING: f32 = 0.13;

    for kind in VehicleKind::PLAYABLE {
        let Some(blueprint) = game_core::VehicleBlueprint::for_vehicle(kind) else { continue };
        let Some(kin) = RunningGearKinematics::for_vehicle(kind) else { continue };
        // The widest thing drawn: the outer face of the belt band.
        let visible_half_width = kin.link_x + kin.band_half_width;
        let phantom = blueprint.hull.hitbox_half_width - visible_half_width;
        assert!(
            phantom <= PHANTOM_WIDTH_CEILING,
            "{kind:?}: the hitbox reaches {:.3} m but nothing is drawn past {:.3} m — {:.3} m of \
             invisible width to ram into, past the recorded ceiling {PHANTOM_WIDTH_CEILING}",
            blueprint.hull.hitbox_half_width,
            visible_half_width,
            phantom
        );
        if phantom > 0.0 {
            println!(
                "HITBOX DEBT {kind:?}: {phantom:.3} m of collision width past the visible \
                 vehicle (target 0 — Model Idealny M14/PR-14)"
            );
        }
    }
}
