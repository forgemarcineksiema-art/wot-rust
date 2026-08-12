//! Locks the TRUE-normal armor geometry: plate slope lives in the 3D plate normal, so the
//! impact angle is a real angle of incidence. Flat shots meet a glacis at its slope, plunging
//! fire meets it squarer (plunging DEFEATS sloped armor — the case the old angle-addition model
//! inverted), a nose-up hull-down pose steepens it, and a shell dropping on the deck lands on
//! the roof plate measured against UP, not against a phantom front plate.

use game_core::math::HullPose;
use game_core::{ArmorZone, TankId, TankSpec};
use glam::Vec3;
use sim::{SegmentImpact, ShellTraceWorld, TraceTank, segment_impact};

fn t54_at_origin(hull: HullPose) -> TraceTank {
    TraceTank::from_spec(TankId(9), Vec3::ZERO, hull, 0.0, &TankSpec::t54_1951())
}

/// Fire one straight segment and expect a tank impact.
fn hit(from: Vec3, to: Vec3, tank: TraceTank) -> (ArmorZone, f32) {
    let tanks = [tank];
    let world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &tanks,
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
    };
    match segment_impact(from, to, to - from, &world) {
        Some(SegmentImpact::Tank { zone, impact_angle_degrees, .. }) => {
            (zone, impact_angle_degrees)
        }
        other => panic!("expected a tank impact, got {other:?}"),
    }
}

fn glacis_slope() -> f32 {
    TankSpec::t54_1951().hull.plate(ArmorZone::UpperGlacis).slope_degrees
}

#[test]
fn a_flat_shot_meets_the_glacis_at_its_slope() {
    let (zone, angle) = hit(
        Vec3::new(0.0, 1.5, 10.0),
        Vec3::new(0.0, 1.5, 0.0),
        t54_at_origin(HullPose::level(0.0)),
    );
    assert_eq!(zone, ArmorZone::UpperGlacis);
    assert!(
        (angle - glacis_slope()).abs() < 1.0,
        "flat shot angle {angle} should equal the {}° glacis slope",
        glacis_slope()
    );
}

#[test]
fn plunging_fire_meets_the_reclined_glacis_far_squarer() {
    // 45° dive through the same glacis point a flat shot would strike: the plate leans back,
    // so the diving shell arrives nearly perpendicular. Angle-addition gave ~89° (auto-bounce);
    // the true normal gives slope - 45.
    let through = Vec3::new(0.0, 1.5, 3.2);
    let dive = Vec3::new(0.0, 1.0, 1.0).normalize();
    let (zone, angle) =
        hit(through + dive * 8.0, through - dive * 4.0, t54_at_origin(HullPose::level(0.0)));
    assert_eq!(zone, ArmorZone::UpperGlacis);
    let flat = glacis_slope();
    assert!(
        angle < flat - 30.0,
        "a 45° dive must beat the slope by tens of degrees: {angle} vs flat {flat}"
    );
}

#[test]
fn hull_down_nose_up_steepens_the_glacis() {
    // The nose-up pose moves the bow, so resolve where the upper glacis actually sits — a point
    // on the REAL blueprint glacis plane (fold at the sponson step, reclined by the slope),
    // carried through the pose — and fire flat through it.
    let nose_up = HullPose { yaw_rad: 0.0, pitch_rad: 0.3, roll_rad: 0.0 };
    let blueprint = game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
        .expect("blueprint");
    let cy = blueprint.hull.hitbox_center_y;
    let up_the_plate = 0.29;
    let glacis_local = Vec3::new(
        0.0,
        (blueprint.hull.sponson_y - cy) + up_the_plate,
        blueprint.hull.half_len - up_the_plate * blueprint.armor.hull_front.0.to_radians().tan(),
    );
    let glacis_world = nose_up.basis() * (Vec3::Y * cy) + nose_up.basis() * glacis_local;
    let (zone, angle) = hit(
        Vec3::new(0.0, glacis_world.y, glacis_world.z + 8.0),
        Vec3::new(0.0, glacis_world.y, glacis_world.z - 4.0),
        t54_at_origin(nose_up),
    );
    assert_eq!(zone, ArmorZone::UpperGlacis);
    assert!(
        angle > glacis_slope() + 12.0,
        "a ~17° nose-up pose adds its pitch to the effective slope, got {angle}"
    );
}

/// Fire one straight segment and expect NO tank impact.
fn misses(from: Vec3, to: Vec3, tank: TraceTank) {
    let tanks = [tank];
    let world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &tanks,
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
    };
    let outcome = sim::segment_impact(from, to, to - from, &world);
    assert!(outcome.is_none(), "expected a clean miss, got {outcome:?}");
}

#[test]
fn the_gap_over_the_tracks_is_honest_air() {
    // The T-54 hull wall sits at ±1.05 m and the track band tops out around 1.04 m of height;
    // the old hitbox pretended armor filled the full ±1.75 m width. A shot threading over the
    // track, outside the narrow hull, must fly on.
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let x = (blueprint.hull.half_width + blueprint.hull.hitbox_half_width) * 0.5;
    let y = blueprint.track.top_y + blueprint.track.belt_half_thickness + 0.15;
    misses(Vec3::new(x, y, 10.0), Vec3::new(x, y, -10.0), t54_at_origin(HullPose::level(0.0)));
}

#[test]
fn the_track_band_is_the_real_belt_box() {
    // Into the belt from the side, at wheel height: the impact is the track zone at the belt's
    // real outer face — the spaced-armor screen in front of the hull wall.
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let y = blueprint.track.wheel_radius; // mid-belt height
    let (zone, _) =
        hit(Vec3::new(10.0, y, 0.5), Vec3::new(-10.0, y, 0.5), t54_at_origin(HullPose::level(0.0)));
    assert_eq!(zone, ArmorZone::RightTrack, "the belt box answers before the hull wall");
}

/// Renamed with PR-17: there is no ball. The zone a shot down the gun line meets is the
/// APERTURE — the patch is sized to the hole (0.20 half-extent) rather than to a mask half again
/// wider than it, and it is no longer inflated by the external-ball rule that exists to catch a
/// socket lip this vehicle does not have.
#[test]
fn a_shot_down_the_gun_line_lands_on_the_gun_aperture() {
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let y = blueprint.gun.trunnion_y;
    let (zone, _) =
        hit(Vec3::new(0.0, y, 10.0), Vec3::new(0.0, y, -2.0), t54_at_origin(HullPose::level(0.0)));
    assert_eq!(zone, ArmorZone::Mantlet, "the gun line meets the mount behind the aperture");
}

/// The cast dome's normal SWEEPS around the casting, so where a flat shot lands across the face
/// decides the angle it meets — the whole reason a T-54 is hard to kill from the front.
///
/// Re-blessed 2026-07-29 (PR-13, the armour volume became the casting). The sweep used to be
/// measured against a swept CIRCLE of 1.12 m, which the cheeks bulge a third of a metre past.
/// With the volume drawn on the real casting the numbers move, and they move toward the vehicle:
/// the cheek's fuller front face meets a flat shot more squarely at its inboard edge (0.95 m out:
/// 41 degrees, where the circle gave a steeper sector by accident) and then rises hard across the
/// shoulder — 60 degrees at 1.05 m, 70 at the flank. That is a casting, not a cylinder.
#[test]
fn the_dome_cheek_presents_a_steeper_angle_than_the_dome_center() {
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let y = blueprint.gun.trunnion_y;
    let flat_shot_at = |x: f32| {
        hit(Vec3::new(x, y, 10.0), Vec3::new(x, y, -2.0), t54_at_origin(HullPose::level(0.0)))
    };

    let (_, center_angle) = flat_shot_at(0.15);
    // The cheek SHOULDER, where the swell turns away from the shot.
    let (_, shoulder_angle) = flat_shot_at(1.05);
    assert!(
        shoulder_angle > center_angle + 15.0,
        "the cheek shoulder glances: {shoulder_angle}° vs center {center_angle}°"
    );

    // And it is a sweep, not a step: the angle rises monotonically outward across the casting.
    let mut previous = 0.0_f32;
    for x in [0.15_f32, 0.45, 0.75, 0.95, 1.05, 1.12] {
        let (_, angle) = flat_shot_at(x);
        assert!(
            angle >= previous - 0.01,
            "the casting must turn away steadily, but x={x:.2} meets {angle:.1}° after              {previous:.1}°"
        );
        previous = angle;
    }
    assert!(previous > 65.0, "by the flank the same shot is glancing hard: {previous:.1}°");
}

#[test]
fn the_is3_pike_rewards_head_on_and_punishes_angling() {
    // The pike nose inverts the angling instinct: head-on, a bow face compounds its vertical
    // slope with the plan sweep; yawing the hull toward the shot FLATTENS the near face until
    // it presents only its vertical slope. Locks that the two baked pike planes are real.
    let spec = game_core::VehicleKind::IS3.spec();
    let head_on_tank =
        TraceTank::from_spec(TankId(3), Vec3::ZERO, HullPose::level(0.0), 0.0, &spec);
    let (zone, head_on) = hit(Vec3::new(0.6, 1.2, 10.0), Vec3::new(0.6, 1.2, 0.0), head_on_tank);
    assert_eq!(zone, ArmorZone::UpperGlacis);

    let sweep = game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::IS3)
        .expect("blueprint")
        .hull
        .pike_sweep_deg;
    // Yawing the hull by the sweep squares the RIGHT pike face to the shot — and swings the
    // bow leftward, so the face's center now sits near world x ≈ -0.8.
    let angled_tank = TraceTank::from_spec(
        TankId(3),
        Vec3::ZERO,
        HullPose::level(-sweep.to_radians()),
        0.0,
        &spec,
    );
    let (zone, angled) = hit(Vec3::new(-0.83, 1.2, 10.0), Vec3::new(-0.83, 1.2, 0.0), angled_tank);
    assert_eq!(zone, ArmorZone::UpperGlacis);
    // Head-on the face compounds slope and sweep (~64° true); squared, it presents only its
    // bare ~56° vertical slope — a ~27% effective-armor loss for the angler.
    assert!(
        angled < head_on - 5.0,
        "turning the pike face square must flatten it: angled {angled}° vs head-on {head_on}°"
    );
}

#[test]
fn a_shell_dropping_on_the_deck_lands_on_the_hull_deck_measured_against_up() {
    // Straight down onto the rear deck, behind the turret: the deck is the hull slab's top face.
    let (zone, angle) = hit(
        Vec3::new(0.6, 12.0, -2.6),
        Vec3::new(0.6, 0.5, -2.6),
        t54_at_origin(HullPose::level(0.0)),
    );
    // `HullDeck`, not `Roof`: they used to share one zone, so a deck hit reported the TURRET
    // front on the wire and both plates resolved against one derived thickness. The deck is
    // 20 mm of engine cover; the turret roof is 30 mm of casting.
    assert_eq!(zone, ArmorZone::HullDeck, "the deck is hull plate, not the turret roof");
    assert!(angle < 1.0, "a vertical drop is square-on to the deck (angle vs UP), got {angle}");
}
// APPEND to crates/runtime/sim/tests/armor_geometry.rs
// (PR-1c z mapy kaskad: pierwszy test sim, ktory w ogole strzela w dolna plyte.)

/// THE LOWER PLATE PLAYS ITS AUTHORED GEOMETRY. No sim test ever fired at it — which is how a
/// 27-degree armour plate lived under a 36.8-degree visual and a 55-degree dossier without one
/// red test. A flat bow shot below the fold must meet the LowerPlate zone at the blueprint's
/// authored angle, and the facet the resolver reads must carry the dossier's 100 mm — LOS
/// 100/cos(55°) ≈ 174 mm, the number the plate was always supposed to be worth.
#[test]
fn a_flat_shot_meets_the_lower_plate_at_its_authored_slope() {
    let blueprint = game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
        .expect("blueprint");
    let (authored_deg, _) =
        blueprint.armor.hull_lower_front.expect("the T-54 authors its lower plate");

    // Below the fold (sponson step at 1.0), above the belly: the middle of the plate.
    let (zone, angle) = hit(
        Vec3::new(0.0, 0.70, 10.0),
        Vec3::new(0.0, 0.70, 0.0),
        t54_at_origin(HullPose::level(0.0)),
    );
    assert_eq!(zone, ArmorZone::LowerPlate);
    assert!(
        (angle - authored_deg).abs() < 1.0,
        "a flat shot below the fold meets the plate at its authored {authored_deg}°, got {angle}"
    );

    // And the facet the resolver reads carries the dossier's steel: full module thickness at
    // the authored angle — not 72% of the glacis at 45% of its slope.
    let plate = TankSpec::t54_1951().hull.plate(ArmorZone::LowerPlate);
    assert!(
        (plate.slope_degrees - authored_deg).abs() < 0.1,
        "facet slope {} vs authored {authored_deg}",
        plate.slope_degrees
    );
    let los = plate.nominal_thickness_mm / plate.slope_degrees.to_radians().cos();
    assert!(
        (170.0..180.0).contains(&los),
        "100 mm at 55 degrees is ~174 mm of line-of-sight steel, got {los:.0}"
    );
}

/// The FIRST shots ever fired at the T-54's stern. The rear was one sourceless 5-degree plate
/// for months and nothing noticed, because nothing looked: the same disease the lower plate had.
/// Now the tail is the authored pair — 45 mm @ 17 above the knuckle, the undercut below it —
/// and a shell meets each plate at the angle the player sees.
#[test]
fn a_flat_shot_meets_the_stern_pair_at_its_authored_angles() {
    let blueprint = game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
        .expect("blueprint");
    let (knuckle_y, lower_deg) =
        blueprint.armor.hull_rear_knuckle.expect("the T-54 authors its stern knuckle");
    let upper_deg = blueprint.armor.hull_rear.0;

    // From BEHIND, above the knuckle: the upper plate at its documented 17 degrees.
    let (zone, angle) = hit(
        Vec3::new(0.0, 1.40, -10.0),
        Vec3::new(0.0, 1.40, 0.0),
        t54_at_origin(HullPose::level(0.0)),
    );
    assert_eq!(zone, ArmorZone::HullRear);
    assert!(
        (angle - upper_deg).abs() < 1.0,
        "a flat shot above the knuckle meets the upper plate at {upper_deg}°, got {angle}"
    );

    // From behind, below the knuckle (and below the sponson, into the tub's tail): the undercut.
    let (zone, angle) = hit(
        Vec3::new(0.0, 0.90, -10.0),
        Vec3::new(0.0, 0.90, 0.0),
        t54_at_origin(HullPose::level(0.0)),
    );
    assert_eq!(zone, ArmorZone::HullRear);
    assert!(
        (angle - lower_deg).abs() < 1.0,
        "a flat shot below the knuckle meets the undercut at {lower_deg}° (authored at \
         knuckle_y {knuckle_y}), got {angle}"
    );

    // The facet the resolver reads: 45 mm at 17 degrees is ~47 mm of line-of-sight steel.
    let plate = TankSpec::t54_1951().hull.plate(ArmorZone::HullRear);
    assert!(
        (plate.slope_degrees - upper_deg).abs() < 0.1,
        "facet slope {} vs authored {upper_deg}",
        plate.slope_degrees
    );
    let los = plate.nominal_thickness_mm / plate.slope_degrees.to_radians().cos();
    assert!(
        (45.0..50.0).contains(&los),
        "45 mm at 17 degrees is ~47 mm of line-of-sight steel, got {los:.0}"
    );
}
