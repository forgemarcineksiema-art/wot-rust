//! The Tiger I shape cage: locks the slab anatomy the blueprint migration bought — vertical
//! plates that ARE the armor planes, the horseshoe turret prism, the interleaved wheel rows,
//! the braked KwK 36, and the drum cupola topping the documented 3.00 m silhouette. Each lock
//! names a defect that would silently un-Tiger the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::TigerI).expect("Tiger I has a blueprint")
}

/// The slab, honestly: the visible driver's plate stands within a degree of vertical and lies
/// ON the armor volume's glacis plane — what you see leaning back 9° is what the penetration
/// model resolves at 9°.
#[test]
fn the_visible_drivers_plate_is_the_armor_glacis_plane() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::TigerI).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;

    assert!(bp.armor.hull_front.0 <= 12.0, "a Tiger's front is nearly vertical");
    let glacis = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::UpperGlacis)
        .expect("glacis plane");
    let slope = glacis.normal.y.asin().to_degrees();
    assert!((slope - bp.armor.hull_front.0).abs() < 1.0e-3, "armor plane carries the 9°: {slope}");

    // Real mesh vertices sit ON that plane (the upper front plate between sponson and deck).
    let on_plane = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| {
            point.y > bp.hull.sponson_y - cy - 1.0e-3
                && point.x.abs() <= bp.hull.half_width + 1.0e-3
                && (glacis.normal.dot(*point) - glacis.offset).abs() < 1.0e-3
        })
        .count();
    assert!(on_plane >= 4, "the visible driver's plate must lie on the armor plane: {on_plane}");
}

/// The hull sides are genuinely vertical walls at the full sponson beam — the armor facet says
/// 0° and the visible wall is a plane at `half_width`, not a raked or narrowed panel.
#[test]
fn the_hull_sides_are_vertical_slabs_at_the_full_beam() {
    let bp = blueprint();
    assert_eq!(bp.armor.hull_side.0, 0.0, "the 80 mm sides are vertical");
    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;

    // Above the sponson step the widest hull metal is the side wall itself, and plenty of
    // vertices stand exactly on it over a real vertical run.
    let wall: Vec<f32> = hull_mesh
        .vertices()
        .iter()
        .filter(|vertex| {
            vertex.position.y > bp.hull.sponson_y - 1.0e-3
                && (vertex.position.x - bp.hull.half_width).abs() < 1.0e-3
        })
        .map(|vertex| vertex.position.y)
        .collect();
    let lo = wall.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = wall.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        hi - lo > 0.9,
        "the side wall must be a tall vertical plate at x = {}, spans {lo}..{hi}",
        bp.hull.half_width
    );
}

/// The horseshoe turret is a plate prism to the armor model — six planes, vertical side walls
/// the visible wall stands on, and the Rommelkiste's back face closing the rear armor plane.
#[test]
fn the_horseshoe_turret_walls_are_the_armor_prism() {
    let bp = blueprint();
    let volumes = vehicle_armor_volumes(VehicleKind::TigerI).expect("armor volumes");
    assert_eq!(volumes.turret.planes.len(), 6, "welded box: a prism, not a swept dome");
    let side = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretSide && plane.normal.x > 0.9)
        .expect("right wall");
    assert_eq!(side.normal.y, 0.0, "the horseshoe wall is vertical");

    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let cy = bp.hull.hitbox_center_y;
    let widest =
        turret_mesh.vertices().iter().map(|v| v.position.x).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (widest - bp.turret.plan_half_width).abs() < 0.02,
        "the visible wall stands on the armor plane: {widest} vs {}",
        bp.turret.plan_half_width
    );
    // The bustle's visible rear (the stowage bin's back face) sits on the rear armor plane.
    let rear_plane_z = bp.turret.ring_z - bp.turret.plan_half_length;
    let rearmost =
        turret_mesh.vertices().iter().map(|v| v.position.z).fold(f32::INFINITY, f32::min);
    assert!(
        (rearmost - rear_plane_z).abs() < 0.02,
        "the Rommelkiste closes the rear armor plane: {rearmost} vs {rear_plane_z}"
    );
    // And nothing pokes out of the prism.
    for vertex in turret_mesh.vertices() {
        let point = vertex.position - Vec3::Y * cy;
        let signed = side.normal.dot(point) - side.offset;
        assert!(signed <= 1.0e-3, "turret metal outside the armor wall by {signed} m");
    }
}

/// The Schachtellaufwerk: eight wheels per side in two genuinely separate rows — odd wheels a
/// full `overlap_inner_dx` inboard — with no return rollers (the top run rests on the wheels).
#[test]
fn eight_interleaved_wheels_and_no_return_rollers() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 8, "eight axles per side");
    assert!(bp.track.overlap_inner_dx >= 0.20, "a real interleave, not a token stagger");
    assert_eq!(bp.track.return_rollers, 0, "the Tiger carries its top run on the wheels");

    let kin = RunningGearKinematics::for_vehicle(VehicleKind::TigerI).expect("blueprint gear");
    let wheels: Vec<f32> = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
        .map(|p| p.transform.w_axis.x)
        .collect();
    // The Schachtellaufwerk sandwich (W1 PR-T1.2): every outer-row axle carries a second
    // deep-plane disc, so 8 axles render 12 discs per side in three planes.
    assert_eq!(wheels.len(), 12);
    let rows: Vec<f32> = {
        let mut xs = wheels.clone();
        xs.sort_by(f32::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
        xs
    };
    assert_eq!(rows.len(), 3, "three interleaved wheel planes, got {rows:?}");
    assert!((rows[1] - rows[0] - bp.track.overlap_inner_dx).abs() < 1.0e-4);
    // The rows clear each other: the disc faces do not interpenetrate.
    assert!(bp.track.overlap_inner_dx >= 2.0 * kin.wheel_half_width, "discs must not merge");
}

/// The KwK 36 wears its double-baffle muzzle brake and no bore evacuator — the barrel tip
/// visibly swells past the plain barrel radius near the muzzle.
#[test]
fn the_kwk36_carries_a_muzzle_brake_and_no_evacuator() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_some(), "the 8.8 cm wears its brake");
    assert!(bp.gun.evacuator.is_none(), "no bore evacuator on a KwK 36");

    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let gun_mesh = &baked.submesh(SubmeshKind::Gun).expect("gun submesh").mesh;
    let brake_swell = gun_mesh
        .vertices()
        .iter()
        .filter(|v| v.position.z > bp.gun.muzzle_z - 0.45)
        .map(|v| Vec3::new(v.position.x, v.position.y - bp.gun.trunnion_y, 0.0).length())
        .fold(0.0_f32, f32::max);
    assert!(
        brake_swell > bp.gun.barrel_radius * 1.25,
        "the muzzle must swell into a brake: {brake_swell} vs barrel {}",
        bp.gun.barrel_radius
    );
}

/// The drum cupola stands on the LEFT rear roof and tops out the documented 3.00 m silhouette.
#[test]
fn the_drum_cupola_tops_the_three_meter_silhouette_on_the_left() {
    let bp = blueprint();
    assert!(bp.turret.cupola_x < -0.3, "the commander sits on the left");
    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let apex = turret_mesh
        .vertices()
        .iter()
        .max_by(|a, b| a.position.y.total_cmp(&b.position.y))
        .expect("turret has vertices");
    assert!((apex.position.y - 3.01).abs() < 0.05, "cupola top ~3.0 m: {}", apex.position.y);
    assert!(apex.position.x < 0.0, "the apex is the left-side cupola");
}

/// The migrated body is the RESEARCHED Tiger: 6.32 m hull in a 6.44 m box, 3.705 m over the
/// tracks, 3.0 m tall — not the old 7.2 m stretched box.
#[test]
fn the_hitbox_is_the_researched_body_not_the_legacy_stretch() {
    let bp = blueprint();
    let hitbox = game_core::HitboxProfile::for_vehicle(VehicleKind::TigerI);
    assert!((hitbox.half_length_m - 3.22).abs() < 1.0e-6);
    assert!((hitbox.half_width_m - 1.87).abs() < 1.0e-6);
    assert!(((hitbox.center_y_m + hitbox.half_height_m) - 3.01).abs() < 1.0e-6, "3 m tall");
    assert!((bp.hull.half_len - 3.16).abs() < 1.0e-6, "the documented 6.32 m hull");
    assert!((bp.track.outer_x - 1.84).abs() < 1.0e-6, "3.7 m over the combat tracks");
}
