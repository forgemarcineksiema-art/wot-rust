//! The fleet's construction invariants, one gate for every playable vehicle (Forge 2.0
//! acceleration, step 2c). Until 2026-09-05 each vehicle carried its own test file restating
//! its blueprint's numbers in Rust asserts (roof 2.60, sponson 1.22, 3.548 over the tracks …) —
//! a second copy of the RON that every data change had to edit too. A number lives once, in
//! RON, and is watched by the dimension anchors (`reference/<slug>.reference.ron`) and the
//! K0 outlines; what a TEST owns is the construction: that the metal a player sees lies on the
//! planes a shell meets, that the running gear is the blueprint's, that the gun and the cupola
//! are what the blueprint says. Every check runs on the SHIPPED composition (`describe`), the
//! bake the game draws, and reads only the blueprint — a new vehicle is covered the moment it
//! has a RON file.

use game_core::{ArmorZone, TurretForm, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, RunningGearKinematics, SubmeshKind,
    running_gear_placements,
};

fn shipped(kind: VehicleKind) -> BakedVehicle {
    vehicle_recipes::describe(kind).unwrap_or_else(|| panic!("{kind:?} describes")).build()
}

fn blueprint(kind: VehicleKind) -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(kind).unwrap_or_else(|| panic!("{kind:?} has a blueprint"))
}

fn submesh(vehicle: &BakedVehicle, kind: SubmeshKind) -> &GeometryMesh {
    &vehicle.submesh(kind).unwrap_or_else(|| panic!("{:?} has a {kind:?}", vehicle.kind())).mesh
}

/// Vertices of `mesh` (hull frame, `cy` removed so they compare with the armour planes) that
/// lie ON `plane` within a millimetre, after `keep` filters the candidates.
fn on_plane(
    mesh: &GeometryMesh,
    cy: f32,
    plane: &game_core::TaggedPlane,
    keep: impl Fn(Vec3) -> bool,
) -> usize {
    mesh.vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| keep(*point))
        .filter(|point| (plane.normal.dot(*point) - plane.offset).abs() < 1.0e-3)
        .count()
}

/// What you see leaning is what the penetration model resolves: every upper front plate the
/// armour bakes (one, or the pike's two) carries visible hull metal ON it, and so does an
/// authored bow shelf (`hull_bow_shelf`), which is the glacis-zoned plane facing mostly up.
#[test]
fn the_visible_front_plates_lie_on_the_armor_glacis_planes() {
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let cy = bp.hull.hitbox_center_y;
        let vehicle = shipped(kind);
        let hull = submesh(&vehicle, SubmeshKind::Hull);
        let volumes = vehicle_armor_volumes(kind).expect("armor volumes");
        let plates: Vec<_> = volumes
            .hull
            .iter()
            .flat_map(|volume| volume.planes.iter())
            .filter(|plane| plane.zone == ArmorZone::UpperGlacis)
            .collect();
        let expected = if bp.hull.pike_sweep_deg > 0.0 { 2 } else { 1 }
            + usize::from(bp.armor.hull_bow_shelf.is_some());
        assert!(
            plates.len() >= expected,
            "{kind:?}: {expected} glacis-zoned planes expected, found {}",
            plates.len()
        );
        for plate in plates {
            let count =
                on_plane(hull, cy, plate, |point| point.y > bp.hull.sponson_y - cy - 1.0e-3);
            assert!(
                count >= 4,
                "{kind:?}: the visible front plate must lie on its armor plane (normal {:?}): {count}",
                plate.normal
            );
        }
    }
}

/// The upper hull sides — vertical or leaned (`hull_side`'s degrees) — carry visible metal on
/// the side planes the armour bakes, both sides.
#[test]
fn the_visible_hull_sides_lie_on_the_armor_side_planes() {
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let cy = bp.hull.hitbox_center_y;
        let vehicle = shipped(kind);
        let hull = submesh(&vehicle, SubmeshKind::Hull);
        let volumes = vehicle_armor_volumes(kind).expect("armor volumes");
        for sign in [1.0_f32, -1.0] {
            let side = volumes.hull[0]
                .planes
                .iter()
                .find(|plane| plane.zone == ArmorZone::HullSide && plane.normal.x * sign > 0.5)
                .unwrap_or_else(|| panic!("{kind:?}: an upper side plane on x {sign}"));
            let lean = side.normal.y.asin().to_degrees();
            assert!(
                (lean - bp.armor.hull_side.0).abs() < 1.0e-3,
                "{kind:?}: the side plane leans the armour table's degrees: {lean}"
            );
            let count = on_plane(hull, cy, side, |point| point.x * sign > 0.3);
            assert!(count >= 4, "{kind:?}: the visible side wall must lie on its plane: {count}");
        }
    }
}

/// A welded box turret is a plate prism to the armour model, and its visible walls stand on
/// it: the side planes and the rear plane each carry turret metal ON them — the rear plane is
/// whatever closes the bustle (the Tiger's Rommelkiste back face, the Henschel's leaned rear
/// wall). Cast domes and casemates have their own constructions (the dome's sectors, the
/// casemate as the hull's own planes continued).
#[test]
fn the_welded_turret_walls_stand_on_the_armor_prism() {
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        if bp.turret.form != TurretForm::WeldedBox {
            continue;
        }
        checked += 1;
        let cy = bp.hull.hitbox_center_y;
        let volumes = vehicle_armor_volumes(kind).expect("armor volumes");
        assert_eq!(volumes.turret.planes.len(), 6, "{kind:?}: a welded box is a six-plane prism");
        let vehicle = shipped(kind);
        let turret = submesh(&vehicle, SubmeshKind::Turret);
        for plane in &volumes.turret.planes {
            let wall = match plane.zone {
                ArmorZone::TurretSide => "side wall",
                ArmorZone::TurretRear => "rear wall",
                _ => continue,
            };
            let count = on_plane(turret, cy, plane, |_| true);
            assert!(
                count >= 4,
                "{kind:?}: the visible {wall} must lie on its armor plane (normal {:?}): {count}",
                plane.normal
            );
        }
    }
    assert!(checked >= 3, "the German line's welded turrets are covered: {checked}");
}

/// The running gear is the blueprint's, both sides: one road wheel per authored axle, the
/// authored return rollers, the belt's authored link count, the sprocket at the drive end and
/// the idler at the other, overlapped wheels in more than one plane when the blueprint says so.
#[test]
fn the_running_gear_is_the_blueprint_s() {
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let kin = RunningGearKinematics::for_vehicle(kind).expect("blueprint gear");
        let placements = running_gear_placements(&kin, 0.0, 0.0);
        let count = |part: GearPart| placements.iter().filter(|p| p.part == part).count();
        assert_eq!(count(GearPart::RoadWheel), 2 * bp.track.wheel_count, "{kind:?}: road wheels");
        assert_eq!(count(GearPart::ReturnRoller), 2 * bp.track.return_rollers, "{kind:?}: rollers");
        if let Some(links) = bp.track.link_count {
            assert_eq!(count(GearPart::Link), 2 * links, "{kind:?}: links per side");
            assert_eq!(kin.link_count(), links);
        }
        assert_eq!(count(GearPart::Sprocket), 2, "{kind:?}: one sprocket a side");
        assert_eq!(count(GearPart::Idler), 2, "{kind:?}: one idler a side");
        let z_of = |part: GearPart| {
            placements.iter().find(|p| p.part == part).map(|p| p.transform.w_axis.z).unwrap()
        };
        let (sprocket_z, idler_z) = (z_of(GearPart::Sprocket), z_of(GearPart::Idler));
        assert!(
            (sprocket_z > 0.0) == bp.track.drive_front && (idler_z > 0.0) != bp.track.drive_front,
            "{kind:?}: the sprocket sits at the drive end (front: {}), the idler opposite",
            bp.track.drive_front
        );
        let mut planes: Vec<f32> = placements
            .iter()
            .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
            .map(|p| p.transform.w_axis.x)
            .collect();
        planes.sort_by(f32::total_cmp);
        planes.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
        let overlapped = bp.track.overlap_inner_dx > 0.0;
        assert_eq!(
            planes.len() > 1,
            overlapped,
            "{kind:?}: overlapped wheels run in more than one plane ({planes:?})"
        );
        if overlapped {
            assert!(
                bp.track.overlap_inner_dx >= 2.0 * kin.wheel_half_width,
                "{kind:?}: overlapped discs must not merge"
            );
        }
    }
}

/// The gun is the blueprint's: the barrel reaches its muzzle, and it wears a brake exactly when
/// the blueprint authors one (the brake is the widest metal at the muzzle).
#[test]
fn the_gun_reaches_its_muzzle_and_wears_only_the_authored_brake() {
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let vehicle = shipped(kind);
        let gun = submesh(&vehicle, SubmeshKind::Gun);
        let muzzle = gun.vertices().iter().map(|v| v.position.z).fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (muzzle - bp.gun.muzzle_z).abs() < 0.05,
            "{kind:?}: the barrel ends at the blueprint's muzzle: {muzzle} vs {}",
            bp.gun.muzzle_z
        );
        let at_the_muzzle = gun
            .vertices()
            .iter()
            .map(|v| v.position)
            .filter(|p| p.z > bp.gun.muzzle_z - 0.45)
            .map(|p| (p.x.powi(2) + (p.y - bp.gun.trunnion_y).powi(2)).sqrt())
            .fold(0.0_f32, f32::max);
        let braked = at_the_muzzle > bp.gun.barrel_radius * 1.25;
        assert_eq!(
            braked,
            bp.gun.muzzle_brake.is_some(),
            "{kind:?}: a brake is the widest metal at the muzzle ({at_the_muzzle:.3} vs barrel {:.3})",
            bp.gun.barrel_radius
        );
    }
}

/// The commander's cupola tops the turret roof by its authored height (never by a deficit the
/// roof left behind), and a hatch a crewman fits through is at least half a metre across.
#[test]
fn the_cupola_tops_the_roof_by_its_authored_height() {
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let Some(proud) = bp.turret.cupola_height else { continue };
        assert!(bp.turret.cupola_radius >= 0.25, "{kind:?}: a crewman fits through the cupola");
        let vehicle = shipped(kind);
        let turret = submesh(&vehicle, SubmeshKind::Turret);
        let drum_top = turret
            .vertices()
            .iter()
            .map(|v| v.position)
            .filter(|p| {
                ((p.x - bp.turret.cupola_x).powi(2) + (p.z - bp.turret.cupola_z).powi(2)).sqrt()
                    <= bp.turret.cupola_radius + 0.02
            })
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        // The drum's rim, or a lid riding it by a few centimetres.
        assert!(
            drum_top >= bp.turret.roof_y + proud - 0.01
                && drum_top <= bp.turret.roof_y + proud + 0.10,
            "{kind:?}: the cupola tops the roof by its authored {proud}: {drum_top} over {}",
            bp.turret.roof_y
        );
    }
}

/// Side skirts, where the blueprint authors them, are real plates on the spaced-armour plane
/// over the whole upper wheel run — the bazooka plates and Schuerzen a shell meets first.
#[test]
fn authored_skirts_are_plates_on_the_spaced_armor_plane() {
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let Some(skirt) = bp.hull.skirt else { continue };
        checked += 1;
        let vehicle = shipped(kind);
        let hull = submesh(&vehicle, SubmeshKind::Hull);
        let skirt_x = bp.track.outer_x + skirt.standoff_m;
        let span: Vec<f32> = hull
            .vertices()
            .iter()
            .filter(|v| (v.position.x.abs() - skirt_x).abs() < skirt.thickness_m + 0.005)
            .map(|v| v.position.z)
            .collect();
        assert!(!span.is_empty(), "{kind:?}: no skirt plate on the spaced plane x = {skirt_x:.3}");
        let run = span.iter().copied().fold(f32::MIN, f32::max)
            - span.iter().copied().fold(f32::MAX, f32::min);
        assert!(run > 3.0, "{kind:?}: the skirt covers the upper wheel run: {run:.2} m");
    }
    assert!(checked >= 2, "the Tiger II and the Centurion author skirts: {checked}");
}

/// Track guards authored in the visual file (`fender`) are the widest hull metal, and nothing
/// past the belt's outer face but the guard and its lip.
#[test]
fn authored_track_guards_are_the_widest_hull_metal() {
    for kind in VehicleKind::PLAYABLE {
        let bp = blueprint(kind);
        let Some(fender) = bp.visual_detail().and_then(|v| v.fender) else { continue };
        let vehicle = shipped(kind);
        let hull = submesh(&vehicle, SubmeshKind::Hull);
        let widest = hull.vertices().iter().map(|v| v.position.x.abs()).fold(0.0_f32, f32::max);
        let edge = fender.side_x + fender.half.x;
        assert!(
            (widest - edge).abs() < 0.01,
            "{kind:?}: the guards' outer edge is the widest metal: {widest} vs {edge}"
        );
    }
}
