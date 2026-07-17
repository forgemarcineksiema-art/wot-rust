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

/// No cupola: the roof carries only the low periscope housing — the silhouette tops out at the
/// flat casemate roof, unlike every turreted German in the line.
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

/// The PaK 44's overhang is the lineup's longest: the muzzle reaches almost three metres past
/// the bow, and no other vehicle's reach beats it.
#[test]
fn the_pak44_overhang_is_the_longest_in_the_lineup() {
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
        assert!(other < reach, "{kind:?} out-reaches the PaK 44: {other} vs {reach}");
    }
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
    // The Schachtellaufwerk sandwich (W1 PR-T1.2): every outer-row axle carries a second
    // deep-plane disc, so 9 axles render 14 discs per side in three planes.
    assert_eq!(wheels.len(), 14);
    let mut rows = wheels.clone();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
    assert_eq!(rows.len(), 3, "three interleaved wheel planes, got {rows:?}");
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
