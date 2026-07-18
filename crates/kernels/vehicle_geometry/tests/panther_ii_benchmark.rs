//! The Panther II shape cage: locks the wedge anatomy the blueprint migration bought — the
//! steepest German glacis standing ON the armor plane, the 29° leaned sides, the deliberately
//! narrow Schmalturm converging to a slim roof, and the seven overlapped steel wheels. Each
//! lock names a defect that would silently un-Panther the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::PantherII).expect("Panther II has a blueprint")
}

/// The 55° ramp is the steepest GERMAN glacis in the fleet, and the visible plate lies ON the
/// armor volume's plane.
#[test]
fn the_ramp_is_the_steepest_german_glacis_on_the_armor_plane() {
    let bp = blueprint();
    assert!((bp.armor.hull_front.0 - 55.0).abs() < 1.0e-6);
    for kind in [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::Jagdtiger] {
        let other = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        assert!(other.armor.hull_front.0 < 55.0, "{kind:?} must not out-slope the Panther II");
    }

    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::PantherII).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;
    let glacis = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::UpperGlacis)
        .expect("glacis plane");
    let on_plane = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| {
            point.y > bp.hull.sponson_y - cy - 1.0e-3
                && (glacis.normal.dot(*point) - glacis.offset).abs() < 1.0e-3
        })
        .count();
    assert!(on_plane >= 4, "the visible ramp must lie on the armor plane: {on_plane}");
}

/// The upper sides lean their 29° on the armor plane — the Panther family's sponson rake.
#[test]
fn the_upper_sides_lean_their_29_degrees() {
    let bp = blueprint();
    assert!((bp.armor.hull_side.0 - 29.0).abs() < 1.0e-6);
    let volumes = vehicle_armor_volumes(VehicleKind::PantherII).expect("armor volumes");
    let side = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::HullSide && plane.normal.x > 0.5)
        .expect("right side plane");
    assert!((side.normal.y.asin().to_degrees() - 29.0).abs() < 1.0e-3);

    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let cy = bp.hull.hitbox_center_y;
    let on_plane = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| point.x > 0.5 && (side.normal.dot(*point) - side.offset).abs() < 1.0e-3)
        .count();
    assert!(on_plane >= 4, "the visible side wall must lie on the armor plane: {on_plane}");
}

/// The Schmalturm is genuinely NARROW: its beam is the smallest turret beam in the German
/// line, its front plate is barely wider than the Saukopf, and its walls stand on the armor
/// The Panther G turret (dossier PII.1/PII.2, Fort Benning specimen): a NARROW front plate
/// whose cheeks SPLAY to the full 2.04 m beam at the rear third, closed by the wide bustled
/// rear wall — the opposite plan of the old Schmalturm wedge — wearing the curved cast
/// G-blende band across the front. Walls stand on the armor prism planes at the seat.
#[test]
fn the_g_turret_splays_to_its_full_beam_with_the_blende() {
    let bp = blueprint();
    let volumes = vehicle_armor_volumes(VehicleKind::PantherII).expect("armor volumes");
    assert_eq!(volumes.turret.planes.len(), 6, "welded box: a prism");
    let side = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretSide && plane.normal.x > 0.5)
        .expect("right cheek");
    assert!((side.normal.y.asin().to_degrees() - 25.0).abs() < 1.0e-3, "25-degree cheeks");

    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;

    // The plan SPLAYS: at the ring, the shell ahead of the seat mid is narrower than the
    // rear third (the Schmalturm converged the other way — this is the un-wedge lock).
    let ring_width_at = |z_min: f32, z_max: f32| -> f32 {
        turret_mesh
            .vertices()
            .iter()
            .filter(|v| {
                (v.position.y - bp.turret.ring_y).abs() < 1.0e-3
                    && v.position.z > z_min
                    && v.position.z < z_max
            })
            .map(|v| v.position.x)
            .fold(0.0_f32, f32::max)
    };
    let front_third = ring_width_at(bp.turret.ring_z + 0.6, bp.turret.ring_z + 1.2);
    let rear_third = ring_width_at(bp.turret.ring_z - 1.2, bp.turret.ring_z - 0.3);
    assert!(
        rear_third > front_third + 0.25,
        "the G plan splays rearward (front {front_third:.2} vs rear {rear_third:.2});          a converging plan is the old Schmalturm"
    );
    assert!(
        (rear_third - bp.turret.plan_half_width).abs() < 0.02,
        "the cheek stands on the armor plane at the full beam: {rear_third}"
    );

    // The G-blende: the cast band (mantlet smoothing group) spans well over a metre.
    let mantlet_sg = vehicle_geometry::SmoothingGroup(6);
    let blende_x: Vec<f32> = turret_mesh
        .vertices()
        .iter()
        .filter(|v| v.smoothing == mantlet_sg)
        .map(|v| v.position.x)
        .collect();
    assert!(!blende_x.is_empty(), "the front carries the G-blende band");
    let blende_w = blende_x.iter().copied().fold(f32::MIN, f32::max)
        - blende_x.iter().copied().fold(f32::MAX, f32::min);
    assert!(blende_w > 1.0, "G-blende spans {blende_w:.2} m across the front plate");
}

/// Seven overlapped steel wheels per side in two rows — the Tiger II school, not the Panther's
/// rubber dish — with no return rollers.
#[test]
fn seven_overlapped_steel_wheels_without_rollers() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 7, "seven axles per side");
    assert_eq!(bp.track.return_rollers, 0);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::PantherII).expect("blueprint gear");
    let wheels: Vec<f32> = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
        .map(|p| p.transform.w_axis.x)
        .collect();
    // The Schachtellaufwerk sandwich (W1 PR-T1.2): every outer-row axle carries a second
    // deep-plane disc, so 7 axles render 11 discs per side in three planes.
    assert_eq!(wheels.len(), 11);
    let mut rows = wheels.clone();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
    assert_eq!(rows.len(), 3, "three interleaved wheel planes, got {rows:?}");
    assert!(bp.track.overlap_inner_dx >= 2.0 * kin.wheel_half_width, "discs must not merge");
}

/// The KwK 42 L/70 wears a small brake and no evacuator, reaching the documented ~8.9 m
/// overall — a MEDIUM gun, visibly slighter than the Tiger guns beside it.
#[test]
fn the_kwk42_is_a_slighter_braked_gun() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_some());
    assert!(bp.gun.evacuator.is_none());
    assert!(bp.gun.barrel_radius < 0.09, "a 7.5 cm barrel, not an 8.8");
    assert!(
        ((bp.hull.half_len + bp.gun.muzzle_z) - 8.865).abs() < 0.05,
        "the documented 8.86 m overall"
    );
}

/// The migrated body is the RESEARCHED Panther II: 6.87 m hull in a 6.98 m box, 3.42 m over
/// the tracks, 2.99 m tall — not the old 7.4 m box with vertical sides.
#[test]
fn the_hitbox_is_the_researched_body_not_the_legacy_stretch() {
    let bp = blueprint();
    let hitbox = game_core::HitboxProfile::for_vehicle(VehicleKind::PantherII);
    assert!((hitbox.half_length_m - 3.49).abs() < 1.0e-6);
    assert!((hitbox.half_width_m - 1.73).abs() < 1.0e-6);
    assert!(((hitbox.center_y_m + hitbox.half_height_m) - 2.99).abs() < 1.0e-6);
    assert!((bp.hull.half_len - 3.435).abs() < 1.0e-6, "the documented 6.87 m hull");
    assert!((bp.track.outer_x - 1.70).abs() < 1.0e-6, "3.4 m over the tracks");
}

/// The PII.3 hull dressing (dossier): the Kugelblende in the glacis right, the driver's twin
/// periscope hoods at the roof edge left, ONE Bosch light standing on the glacis LEFT (not
/// the family's central deck pod), and the curved three-segment fender sweeps over the front
/// sprockets. Each cites the dossier; losing any of them un-Panthers the bow.
#[test]
fn the_pii3_bow_dressing_holds_ball_periscopes_light_and_sweeps() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;

    // Kugelblende: cast ball right of centre on the 55-degree plate.
    let ball = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::CastArmor
                && (v.position.x - 0.60).abs() < 0.20
                && (v.position.y - 1.42).abs() < 0.20
                && v.position.z > 2.0
        })
        .count();
    assert!(ball > 0, "the MG Kugelblende sits in the glacis right");

    // Driver periscope hoods: LEFT of centre at the roof front edge.
    let hoods = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.position.x < -0.35
                && v.position.x > -0.85
                && v.position.y > bp.hull.deck_y + 0.01
                && v.position.z > 2.0
        })
        .count();
    assert!(hoods > 0, "the driver's periscope hoods sit at the roof edge left");

    // ONE headlight, and it stands LEFT on the glacis (BarrelSteel drum, x < -0.7).
    let light_left = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::BarrelSteel
                && v.position.x < -0.7
                && v.position.y > 1.5
                && v.position.z > 2.0
        })
        .count();
    assert!(light_left > 0, "the Bosch light stands on the glacis left (F4)");

    // Fender sweeps: hull geometry DROPPING ahead of the sprocket axle over each track band —
    // the third segment reaches well below the sponson line (a flat guard would not).
    let sweep_low = hull
        .vertices()
        .iter()
        .filter(|v| {
            (v.position.x.abs() - bp.track.center_x).abs() < 0.45
                && v.position.z > bp.track.end_z + 0.5
                && v.position.y < bp.hull.sponson_y - 0.25
                && v.position.y > bp.hull.sponson_y - 0.55
        })
        .count();
    assert!(sweep_low > 0, "the curved fender sweep drops over the sprocket (F3)");
}
