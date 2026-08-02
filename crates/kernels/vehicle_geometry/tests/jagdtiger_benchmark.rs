//! The Jagdtiger shape cage: locks the casemate anatomy the blueprint migration bought — the
//! fighting compartment whose flank IS the hull's own armor plane, the 15° 250 mm face, the
//! no-cupola roof, the lineup's longest gun overhang, and the fixed prism that never
//! traverses. Each lock names a defect that would silently un-Jagdtiger the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger has a blueprint")
}

/// The welded original's signature: the casemate side wall and the hull's leaned upper side
/// are ONE unbroken 25° plane — the armor volumes' hull-side plane and casemate-side plane
/// coincide, and visible metal stands on it from the sponson to the roof.
#[test]
fn the_casemate_flank_is_the_hull_side_plane_continued() {
    let bp = blueprint();
    let volumes = vehicle_armor_volumes(VehicleKind::Jagdtiger).expect("armor volumes");
    let hull_side = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::HullSide && plane.normal.x > 0.5)
        .expect("hull right side plane");
    let casemate_side = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretSide && plane.normal.x > 0.5)
        .expect("casemate right wall");
    assert!(
        (hull_side.normal - casemate_side.normal).length() < 1.0e-3,
        "one slope: {:?} vs {:?}",
        hull_side.normal,
        casemate_side.normal
    );
    assert!(
        (hull_side.offset - casemate_side.offset).abs() < 2.0e-3,
        "one plane: {} vs {}",
        hull_side.offset,
        casemate_side.offset
    );

    // Visible metal stands on that plane in BOTH submeshes.
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let cy = bp.hull.hitbox_center_y;
    for sub in [SubmeshKind::Hull, SubmeshKind::Turret] {
        let mesh = &baked.submesh(sub).expect("submesh").mesh;
        let on_plane = mesh
            .vertices()
            .iter()
            .map(|vertex| vertex.position - Vec3::Y * cy)
            .filter(|point| {
                point.x > 0.5 && (hull_side.normal.dot(*point) - hull_side.offset).abs() < 2.0e-3
            })
            .count();
        assert!(on_plane >= 4, "{sub:?} must carry metal on the shared flank: {on_plane}");
    }
}

/// The 250 mm face leans only 15° — thickness over slope — and the visible face lies ON the
/// armor front plane with the mantlet patch riding it.
#[test]
fn the_250mm_face_leans_fifteen_degrees_on_the_armor_plane() {
    let bp = blueprint();
    assert!((bp.armor.turret_front.0 - 15.0).abs() < 1.0e-6);
    let volumes = vehicle_armor_volumes(VehicleKind::Jagdtiger).expect("armor volumes");
    assert_eq!(volumes.turret.planes.len(), 6, "casemate: a fixed prism, not a dome");
    let front = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretFront)
        .expect("casemate face");
    assert!((front.normal.y.asin().to_degrees() - 15.0).abs() < 1.0e-3);
    assert!(!front.patches.is_empty(), "the face carries the mantlet patch");

    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let cy = bp.hull.hitbox_center_y;
    let on_plane = turret_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| (front.normal.dot(*point) - front.offset).abs() < 1.0e-3)
        .count();
    assert!(on_plane >= 4, "the visible face must lie on the armor plane: {on_plane}");
}

/// A casemate never traverses: the spec keeps its fixed-casemate clamp, and the whole
/// superstructure sits in the turret submesh only because the renderer needs a slot.
#[test]
fn the_casemate_never_traverses() {
    let spec = VehicleKind::Jagdtiger.spec();
    assert!(spec.has_fixed_casemate(), "the sim must clamp casemate yaw at zero");
    assert!(matches!(blueprint().turret.form, game_core::TurretForm::Casemate));
}

/// THE COLLAR MUST STAND PROUD. Dossier JT.3, the defect this test was written for: the old
/// collar was a Z-aligned socket on a face that leans 15°, so it buried its lower half 275 mm
/// INSIDE the plate and cleared it by only 52 mm at the top — a dark cave around the barrel
/// instead of a casting. The previous lock measured the collar's WIDTH, which the buried shape
/// satisfied, so nothing failed. Width is not relief: this measures relief.
#[test]
fn the_cast_collar_stands_proud_of_the_leaning_casemate_face() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let casemate = &baked.submesh(SubmeshKind::Turret).expect("casemate submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::Jagdtiger).expect("armor volumes");
    let face = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretFront)
        .expect("casemate face");
    let cy = bp.hull.hitbox_center_y;

    let mantlet_sg = vehicle_geometry::SmoothingGroup(6);
    let stand_off: Vec<f32> = casemate
        .vertices()
        .iter()
        .filter(|v| v.smoothing == mantlet_sg)
        .map(|v| face.normal.dot(v.position - Vec3::Y * cy) - face.offset)
        .collect();
    assert!(!stand_off.is_empty(), "the casemate face carries a cast collar");

    let deepest = stand_off.iter().copied().fold(f32::MAX, f32::min);
    let proudest = stand_off.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        deepest > -0.010,
        "the collar's root must SIT ON the armor plane, not sink into it: {deepest:+.3} m"
    );
    assert!(
        proudest > 0.18,
        "the casting must stand clear of the plate all round, not peek over its top edge: \
         {proudest:+.3} m"
    );
}

/// No cupola: whatever the roof carries stays low — the silhouette tops out at the flat casemate
/// roof, unlike every turreted German in the line.
#[test]
fn no_cupola_tops_the_flat_casemate_roof() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let apex =
        turret_mesh.vertices().iter().map(|v| v.position.y).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        apex - bp.turret.roof_y < 0.10,
        "only a low periscope housing may rise over the roof: apex {apex} vs roof {}",
        bp.turret.roof_y
    );
}

/// THE FACE IS A PLATE, NOT A WEDGE. Dossier JT.3: the casemate plan used to chamfer its front
/// corners down to 1.80 m on a 2.71 m deck, leaving a triangular shelf half a metre wide along
/// each side of the gun — the front read as a small box on a wide barge. The welded original's
/// superstructure is a BOX: the 250 mm face spans the full deck width and meets each side wall
/// at a sharp vertical corner.
#[test]
fn the_250mm_face_spans_the_full_deck_width() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let casemate = &baked.submesh(SubmeshKind::Turret).expect("casemate submesh").mesh;
    let front_z = bp.turret.ring_z + bp.turret.plan_half_length;

    let face_half_width = casemate
        .vertices()
        .iter()
        .filter(|v| (v.position.y - bp.turret.ring_y).abs() < 0.01 && v.position.z > front_z - 0.01)
        .fold(0.0_f32, |acc, v| acc.max(v.position.x.abs()));
    assert!(
        (face_half_width - bp.turret.plan_half_width).abs() < 1.0e-3,
        "the face must be as wide as the casemate body: {face_half_width:.3} vs {:.3}",
        bp.turret.plan_half_width
    );

    // And the hull deck must not show a shelf beside it: at the ring plane the hull is exactly
    // as wide as the casemate, so the flank runs on without a step.
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let deck_half_width = hull
        .vertices()
        .iter()
        .filter(|v| (v.position.y - bp.hull.deck_y).abs() < 0.01)
        .fold(0.0_f32, |acc, v| acc.max(v.position.x.abs()));
    assert!(
        (deck_half_width - bp.turret.plan_half_width).abs() < 1.0e-3,
        "no shelf beside the casemate: deck {deck_half_width:.3} vs casemate {:.3}",
        bp.turret.plan_half_width
    );
}

/// THE ROOF IS CREWED. Dossier JT.3: the roof used to carry ONE fitting — a 0.28 m periscope
/// pad — on an otherwise blank 2.7 x 3.2 m plate, with no way in or out. It now carries the two
/// flush crew hatches and the ventilator dome as well, and still no cupola: the check above
/// keeps the silhouette flat.
#[test]
fn the_casemate_roof_carries_hatches_and_a_ventilator() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let casemate = &baked.submesh(SubmeshKind::Turret).expect("casemate submesh").mesh;

    // Cast fittings above the roof plane, split by side: a hatch each side plus the ventilator.
    let roof_fittings: Vec<&_> = casemate
        .vertices()
        .iter()
        .filter(|v| {
            v.position.y > bp.turret.roof_y + 0.005
                && v.material == vehicle_geometry::MaterialRole::CastArmor
        })
        .collect();
    assert!(
        roof_fittings.iter().any(|v| v.position.x > 0.2),
        "a crew hatch on the right of the roof"
    );
    assert!(
        roof_fittings.iter().any(|v| v.position.x < -0.2),
        "a crew hatch and the ventilator on the left of the roof"
    );
    let hatch_z = bp.turret.ring_z - 0.95;
    assert!(
        roof_fittings.iter().any(|v| (v.position.z - hatch_z).abs() < 0.35),
        "the crew hatches sit on the rear roof, over the fighting compartment"
    );
    // The ventilator is forward of the hatches; without this the hatches alone would satisfy
    // both side checks above and the dome could be dropped unnoticed.
    assert!(
        roof_fittings.iter().any(|v| v.position.z > bp.turret.ring_z + 0.40),
        "the ventilator dome sits on the forward roof"
    );
}

/// THE FLANK IS A USED VEHICLE. Dossier JT.3: the sponson slab is the largest single surface on
/// any vehicle in the lineup and it carried nothing at all. It now wears the tow cable, jack,
/// timber block and tool box — and every one of them obeys the fittings rule, lying ON the
/// 25-degree armor plane rather than floating in un-hittable air.
#[test]
fn the_hull_flank_carries_stowage_on_the_armor_plane() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let lean = bp.armor.hull_side.0.to_radians().tan();

    let stowage: Vec<&_> = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::TrackMetal
                && v.position.x.abs() > 1.0
                && v.position.y > bp.hull.sponson_y
        })
        .collect();
    assert!(!stowage.is_empty(), "the sponson flank carries stowage");
    for v in &stowage {
        let wall = bp.hull.half_width - (v.position.y - bp.hull.sponson_y) * lean;
        assert!(
            v.position.x.abs() <= wall + 0.090,
            "stowage vertex x {:.3} floats {:.3} proud of the leaning sponson {:.3}",
            v.position.x.abs(),
            v.position.x.abs() - wall,
            wall
        );
    }
    let run = stowage.iter().map(|v| v.position.z).fold(f32::MIN, f32::max)
        - stowage.iter().map(|v| v.position.z).fold(f32::MAX, f32::min);
    assert!(run > 3.0, "the stowage is spread along the flank, not clustered: {run:.2} m");
}

/// The PaK 44's overhang is the lineup's longest: the muzzle reaches almost three metres past
/// the bow, and no other vehicle's reach beats it.
#[test]
fn the_pak44_overhang_is_the_longest_in_the_lineup() {
    let mut rivals_compared = 0;
    let bp = blueprint();
    // Dossier PR-JT.1: the reference specimen (Aberdeen) and service photos run the PaK 44
    // with a PLAIN muzzle — a brake reappearing here means someone re-fitted it on spec alone.
    assert!(bp.gun.muzzle_brake.is_none(), "the 12.8 cm runs a plain muzzle (dossier decision)");
    let overhang = bp.gun.muzzle_z - bp.hull.half_len;
    assert!(overhang > 2.8, "almost three metres past the bow: {overhang}");
    let reach = bp.gun.muzzle_z - bp.gun.trunnion_z;
    for kind in VehicleKind::ALL {
        if kind == VehicleKind::Jagdtiger {
            continue;
        }
        let mounts = game_core::MountFrames::for_vehicle(kind);
        let other = mounts.muzzle.translation.z - mounts.gun_trunnion.translation.z;
        rivals_compared += 1;
        assert!(other < reach, "{kind:?} out-reaches the PaK 44: {other} vs {reach}");
    }
    assert_eq!(
        rivals_compared,
        VehicleKind::ALL.len() - 1,
        "the PaK 44's reach is only a claim if it was measured against every other gun in the fleet"
    );
}

/// Nine overlapped wheels per side in two rows on the stretched chassis, no return rollers.
#[test]
fn nine_overlapped_wheels_on_the_stretched_chassis() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 9);
    assert_eq!(bp.track.return_rollers, 0);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::Jagdtiger).expect("blueprint gear");
    let wheels: Vec<f32> = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
        .map(|p| p.transform.w_axis.x)
        .collect();
    assert_eq!(wheels.len(), 9, "one wheel unit per authored axle");
    let mut rows = wheels.clone();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
    assert_eq!(rows.len(), 2, "two overlapped wheel planes, got {rows:?}");
    assert!(bp.track.overlap_inner_dx >= 2.0 * kin.wheel_half_width, "discs must not merge");
}

/// The migrated body is the RESEARCHED Jagdtiger: 7.80 m hull in a 7.9 m box, 3.64 m wide,
/// 2.95 m tall — not the old 8.2 m box.
#[test]
fn the_hitbox_is_the_researched_body_not_the_legacy_stretch() {
    let bp = blueprint();
    let hitbox = game_core::HitboxProfile::for_vehicle(VehicleKind::Jagdtiger);
    assert!((hitbox.half_length_m - 3.95).abs() < 1.0e-6);
    assert!((hitbox.half_width_m - 1.82).abs() < 1.0e-6);
    assert!(((hitbox.center_y_m + hitbox.half_height_m) - 2.95).abs() < 1.0e-6);
    assert!((bp.hull.half_len - 3.90).abs() < 1.0e-6, "the documented 7.80 m hull");
}

/// The JT.2 dressing locks (dossier PR-JT.1/JT.2): the massive cast collar at the casemate
/// face, spare-shoe rows LYING ON the 25-degree side walls, flat bow guards over the front
/// sprockets, and the glacis Kugelblende. Each cites a dossier number; the plain-muzzle
/// decision is locked in `the_pak44_reaches_past_the_bow` above.
#[test]
fn the_jt2_dressing_holds_collar_racks_guards_and_ball() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger bakes");
    let casemate = &baked.submesh(SubmeshKind::Turret).expect("casemate submesh").mesh;
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;

    // Cast collar: the SG_MANTLET band spans over a metre — the slight shared ring reads
    // as a Panther-class mantlet, not the 12.8 cm casting.
    let mantlet_sg = vehicle_geometry::SmoothingGroup(6);
    let collar_x: Vec<f32> = casemate
        .vertices()
        .iter()
        .filter(|v| v.smoothing == mantlet_sg)
        .map(|v| v.position.x)
        .collect();
    assert!(!collar_x.is_empty(), "the casemate face carries a cast collar");
    let collar_w = collar_x.iter().copied().fold(f32::MIN, f32::max)
        - collar_x.iter().copied().fold(f32::MAX, f32::min);
    assert!(collar_w > 1.0, "cast collar spans {collar_w:.2} m — the photo shows over a metre");
    // Width alone is what let a BURIED collar pass this lock; its relief is measured by
    // `the_cast_collar_stands_proud_of_the_leaning_casemate_face`.

    // Side racks: TrackMetal shoes on the flank, and every shoe vertex stays AT or UNDER the
    // leaning armor plane (plus the shoe's own proud thickness) — the fittings rule that
    // keeps nothing floating in un-hittable air.
    let lean = bp.turret.side_slope_deg.to_radians().tan();
    let shoe_z: Vec<f32> = casemate
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::TrackMetal && v.position.x.abs() > 0.9
        })
        .map(|v| {
            let wall = bp.turret.plan_half_width - (v.position.y - bp.turret.ring_y) * lean;
            assert!(
                v.position.x.abs() <= wall + 0.078,
                "shoe vertex x {:.3} floats {:.3} proud of the leaning wall {:.3}",
                v.position.x.abs(),
                v.position.x.abs() - wall,
                wall
            );
            v.position.z
        })
        .collect();
    assert!(!shoe_z.is_empty(), "the casemate flank carries spare-shoe rows");
    let shoe_run = shoe_z.iter().copied().fold(f32::MIN, f32::max)
        - shoe_z.iter().copied().fold(f32::MAX, f32::min);
    assert!(shoe_run > 1.5, "the shoe row runs {shoe_run:.2} m along the flank");

    // The row must READ as stowage, not as windows cut in the wall (dossier JT.3): individual
    // shoes with gaps between them, on a carrier rail. Four broad plates satisfied the run
    // above and still read as glazing, so count the separate pieces.
    let row_y = bp.turret.ring_y + 0.42;
    let mut shoe_edges: Vec<f32> = casemate
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::TrackMetal
                && v.position.x.abs() > 0.9
                && (v.position.y - row_y).abs() < 0.13
        })
        .map(|v| v.position.z)
        .collect();
    shoe_edges.sort_by(f32::total_cmp);
    shoe_edges.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    assert!(
        shoe_edges.len() >= 12,
        "at least six separate shoes (two Z edges each), got {} edges: {shoe_edges:?}",
        shoe_edges.len()
    );

    // Flat bow guards: hull plate riding just above the sponson ahead of the front sprocket.
    let guard_vertices = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.position.z > bp.track.end_z
                && (v.position.x.abs() - bp.track.center_x).abs() < 0.45
                && (v.position.y - (bp.hull.sponson_y + 0.03)).abs() < 0.10
        })
        .count();
    assert!(guard_vertices > 0, "flat bow guards cover the front sprockets (F3)");

    // Kugelblende: a cast ball seated in the glacis right of centre.
    let ball_vertices = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::CastArmor
                && (v.position.x - 0.58).abs() < 0.20
                && (v.position.y - 1.45).abs() < 0.20
                && v.position.z > 2.0
        })
        .count();
    assert!(ball_vertices > 0, "the bow MG Kugelblende sits in the glacis right");
}
