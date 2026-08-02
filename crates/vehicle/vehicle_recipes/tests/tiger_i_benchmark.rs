//! The Tiger I shape cage: locks the slab anatomy the blueprint migration bought — vertical
//! plates that ARE the armor planes, the horseshoe turret prism, the interleaved wheel rows,
//! the braked KwK 36, and the drum cupola topping the documented 3.00 m silhouette. Each lock
//! names a defect that would silently un-Tiger the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{GearPart, RunningGearKinematics, SubmeshKind, running_gear_placements};
use vehicle_recipes::bake_vehicle;

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
    assert_eq!(wheels.len(), 8, "one wheel unit per authored axle");
    let rows: Vec<f32> = {
        let mut xs = wheels.clone();
        xs.sort_by(f32::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
        xs
    };
    assert_eq!(rows.len(), 2, "two interleaved wheel planes, got {rows:?}");
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
    assert!((bp.track.outer_x - 1.8525).abs() < 1.0e-6, "3.705 m over the combat tracks");
}

/// The Tiger has TWO documented widths and each is carried by the part that owns it: 3.56 m
/// over the sponsons, 3.705 m over the combat tracks. Authoring the beam on the hull instead
/// (the migration's shortcut) satisfies the composed-bounds width gate just as well, while
/// hiding the belts inside the sponson line — and with them the entire fender line.
#[test]
fn the_tracks_carry_the_width_anchor_and_stand_proud_of_the_sponsons() {
    let bp = blueprint();
    assert!((2.0 * bp.hull.half_width - 3.56).abs() < 1.0e-3, "3.56 m over the sponsons");
    assert!((2.0 * bp.track.outer_x - 3.705).abs() < 1.0e-3, "3.705 m over the tracks");
    let proud = bp.track.outer_x - bp.hull.half_width;
    assert!(proud > 0.06, "the belt must stand proud of the hull wall: {proud} m");
    let band = bp.track.outer_x - bp.track.inner_x;
    assert!((band - 0.725).abs() < 1.0e-3, "the 725 mm combat track, edge to edge: {band}");
    assert!(bp.track.outer_x <= bp.hull.hitbox_half_width, "the box still holds the widest metal");
}

/// The exposed run wears its guards: there is hull metal outboard of the sponson wall over the
/// track, and NOTHING out-reaches the belt's outer face — the width anchor stays the track's.
#[test]
fn the_exposed_track_run_wears_its_fender_line() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let over_the_run: Vec<Vec3> = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position)
        .filter(|point| {
            point.x > bp.hull.half_width + 0.01
                && point.y > bp.track.top_y
                && point.y < bp.hull.deck_y
        })
        .collect();
    assert!(over_the_run.len() >= 8, "the exposed belt must be covered: {}", over_the_run.len());
    let front = over_the_run.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    let rear = over_the_run.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    assert!(
        front > bp.track.end_z && rear < -bp.track.end_z,
        "the guard must run the whole belt, wrap to wrap: {rear}..{front}"
    );
    let widest =
        hull_mesh.vertices().iter().map(|v| v.position.x).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        widest <= bp.track.outer_x + 1.0e-3,
        "no fitting may reach past the belt: {widest} vs {}",
        bp.track.outer_x
    );
}

/// The roof and the drum are locked SEPARATELY. The bare turret roof stands at the documented
/// 2.885 m and the cupola adds only the last 0.115 m. The model used to derive the drum's
/// height as "whatever fills the gap up to the hitbox apex", so a 16.5 cm roof deficit simply
/// grew a 2.5x-too-tall drum and the 3.00 m silhouette lock still passed.
#[test]
fn the_roof_and_the_cupola_are_locked_independently() {
    let bp = blueprint();
    assert!((bp.turret.roof_y - 2.885).abs() < 1.0e-6, "the documented bare-roof height");
    assert!(bp.turret.cupola_height.is_some(), "the drum height must be AUTHORED, not derived");
    let proud = bp.turret.cupola_proud_m(&bp.hull);
    assert!((proud - 0.115).abs() < 1.0e-6, "3.00 m silhouette over a 2.885 m roof: {proud}");

    let baked = bake_vehicle(VehicleKind::TigerI).expect("Tiger I bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let wall_top = turret_mesh
        .vertices()
        .iter()
        .filter(|v| (v.position.x.abs() - bp.turret.plan_half_width).abs() < 0.02)
        .map(|v| v.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (wall_top - bp.turret.roof_y).abs() < 0.02,
        "the horseshoe wall tops out at the bare roof: {wall_top} vs {}",
        bp.turret.roof_y
    );
    let apex =
        turret_mesh.vertices().iter().map(|v| v.position.y).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        apex - bp.turret.roof_y < 0.15,
        "the drum tops the roof by a hatch, not by a deficit: {}",
        apex - bp.turret.roof_y
    );
}

/// Audit #3 held to its own number: the cast cupola is a drum a crewman climbs through (the
/// helper's own reference is ⌀0.78 m), not the ⌀0.66 token the migration carried.
#[test]
fn the_cupola_is_a_hatch_a_crewman_fits_through() {
    let bp = blueprint();
    let diameter = 2.0 * bp.turret.cupola_radius;
    assert!(diameter >= 0.76, "the drum must pass a man: ⌀{diameter}");
    assert!(
        bp.turret.cupola_x.abs() + bp.turret.cupola_radius <= bp.turret.plan_half_width,
        "the honest drum must still land inside the roof plan"
    );
}

/// Audit #16: the German line drives from the FRONT — the transmission sits at the bow and
/// the toothed sprocket is the forward end wheel (photo-confirmed on all four Germans).
#[test]
fn the_drive_sprocket_sits_at_the_bow() {
    let kin = vehicle_geometry::RunningGearKinematics::for_vehicle(VehicleKind::TigerI)
        .expect("Tiger gear");
    assert!(kin.drive_front, "the Tiger I is front-drive");
    let placements = vehicle_geometry::running_gear_placements(&kin, 0.0, 0.0);
    for p in placements {
        match p.part {
            vehicle_geometry::GearPart::Sprocket => {
                assert!(p.transform.w_axis.z > 0.0, "sprocket at the bow")
            }
            vehicle_geometry::GearPart::Idler => {
                assert!(p.transform.w_axis.z < 0.0, "idler at the stern")
            }
            _ => {}
        }
    }
}

/// W1 dossier anchors beyond the box (docs/vehicles/panzerkampfwagen-vi-tiger.md): the
/// documented 0.47 m ground clearance and the 2.17 m fire line (German records give the
/// firing height as 2.195 m; the 2.5 cm difference is a recorded deviation, not drift).
#[test]
fn the_dossier_clearance_and_fire_line_hold() {
    let bp = blueprint();
    assert!((bp.hull.belly_y - 0.47).abs() < 1.0e-6, "0.47 m ground clearance (Wikipedia)");
    assert!((bp.gun.trunnion_y - 2.17).abs() < 1.0e-6, "fire line 2.17 m (documented 2.195 m)");
    let mounts = game_core::MountFrames::for_vehicle(VehicleKind::TigerI);
    assert!(
        (mounts.gun_trunnion.translation.y - bp.gun.trunnion_y).abs() < 1.0e-6,
        "the mount chain carries the same fire line the blueprint authors"
    );
}
