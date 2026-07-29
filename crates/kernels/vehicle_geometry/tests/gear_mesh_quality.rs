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
    // Winding used to be the one class with recorded debt here: every road wheel in the fleet
    // carried 44 inconsistently wound edges and every return roller 16, because the revolve
    // kernel oriented each band of a lathe on its own (see `builder/revolve.rs`). A ring's inner
    // wall faces the axis rather than away from it, so it came out flipped relative to its
    // neighbours and both its seams lit backwards. The kernel now winds a lathe once and orients
    // it once, and the debt is gone fleet-wide — so this is a hard zero, not a ceiling.
    assert_eq!(
        report.inconsistent_winding_edges, 0,
        "{where_}: {} inconsistently wound edges — those faces light backwards, and a wheel is \
         seen from every angle on every frame",
        report.inconsistent_winding_edges
    );
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

/// THE DISTANCE TIER, AS A RULE RATHER THAN A FEELING.
///
/// The gear is the largest body of geometry a vehicle has and it used to be drawn at full detail
/// on every tank on the field, at every range, for the life of the battle — a 7v7 of T-54s spent
/// 540k triangles on running gear that is four pixels tall across the map. `GearDetail` is the
/// answer, and these are the three things that must stay true about it.
#[test]
fn the_distance_tier_is_the_same_gear_built_coarser() {
    use vehicle_geometry::{GEAR_DETAIL_SWITCH_M, GearDetail, gear_detail_for_distance};

    // 1. The rule is a range, and it is the same range for everyone.
    assert_eq!(gear_detail_for_distance(0.0), GearDetail::Near);
    assert_eq!(gear_detail_for_distance(GEAR_DETAIL_SWITCH_M - 0.1), GearDetail::Near);
    assert_eq!(gear_detail_for_distance(GEAR_DETAIL_SWITCH_M + 0.1), GearDetail::Far);

    for kind in VehicleKind::PLAYABLE {
        let Some(near) = RunningGearKinematics::for_vehicle(kind) else { continue };
        let far = near.at_detail(GearDetail::Far);

        // 2. Every part gets cheaper, and none of them vanishes. A tier that deletes a wheel is
        //    not a tier, it is a different tank.
        for (name, a, b) in [
            ("road_wheel", road_wheel_unit_mesh(&near), road_wheel_unit_mesh(&far)),
            ("idler", idler_unit_mesh(&near), idler_unit_mesh(&far)),
            ("sprocket", sprocket_unit_mesh(&near), sprocket_unit_mesh(&far)),
            ("track_link", track_link_unit_mesh(&near), track_link_unit_mesh(&far)),
        ] {
            assert!(
                b.triangle_count() < a.triangle_count(),
                "{kind:?} {name}: the distant build is not cheaper ({} vs {})",
                b.triangle_count(),
                a.triangle_count()
            );
            assert!(b.triangle_count() > 0, "{kind:?} {name}: the distant build deleted the part");
            // 3. It is the SAME part: the coarse build stays inside the fine one's envelope,
            //    within the chord a halved ring gives up. A silhouette that grows at range is a
            //    popping tank.
            let (Some(fine), Some(coarse)) = (a.bounds(), b.bounds()) else { continue };
            let slack = 0.06;
            for axis in 0..3 {
                assert!(
                    coarse.max[axis] <= fine.max[axis] + slack
                        && coarse.min[axis] >= fine.min[axis] - slack,
                    "{kind:?} {name}: the distant build changes the silhouette on axis {axis} \
                     ({:?} vs {:?})",
                    coarse,
                    fine
                );
            }
        }

        // And the distant build still obeys every mesh-quality rule the near one does — a
        // cheaper mesh is not licence to ship a broken one.
        audit(&road_wheel_unit_mesh(&far), &format!("far gear {kind:?} road_wheel"));
        audit(&track_link_unit_mesh(&far), &format!("far gear {kind:?} track_link"));
        audit(&sprocket_unit_mesh(&far), &format!("far gear {kind:?} sprocket"));
        audit(&idler_unit_mesh(&far), &format!("far gear {kind:?} idler"));
    }
}
