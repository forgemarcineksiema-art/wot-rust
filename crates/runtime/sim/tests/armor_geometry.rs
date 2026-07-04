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
    let world = ShellTraceWorld { tanks: &tanks, blockers: &[], heightmap: None, cover: &[] };
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
    let world = ShellTraceWorld { tanks: &tanks, blockers: &[], heightmap: None, cover: &[] };
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

#[test]
fn a_shot_down_the_gun_line_lands_on_the_mantlet_ball() {
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let y = blueprint.gun.trunnion_y;
    let (zone, _) =
        hit(Vec3::new(0.0, y, 10.0), Vec3::new(0.0, y, -2.0), t54_at_origin(HullPose::level(0.0)));
    assert_eq!(zone, ArmorZone::Mantlet, "the gun line meets the mantlet ball, a real circle");
}

#[test]
fn the_dome_cheek_presents_a_steeper_angle_than_the_dome_center() {
    // The cast dome's normal sweeps around the casting: the same flat shot meets the front
    // center near its base slope and the far cheek sector at a much harsher angle.
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let y = blueprint.gun.trunnion_y;
    let (_, center_angle) = hit(
        Vec3::new(0.15, y, 10.0),
        Vec3::new(0.15, y, -2.0),
        t54_at_origin(HullPose::level(0.0)),
    );
    let (_, cheek_angle) = hit(
        Vec3::new(0.95, y, 10.0),
        Vec3::new(0.95, y, -2.0),
        t54_at_origin(HullPose::level(0.0)),
    );
    assert!(
        cheek_angle > center_angle + 15.0,
        "the cheek glances: {cheek_angle}° vs center {center_angle}°"
    );
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
fn a_shell_dropping_on_the_deck_lands_on_the_roof_measured_against_up() {
    // Straight down onto the rear deck, behind the turret: the deck is the hull slab's top face.
    let (zone, angle) = hit(
        Vec3::new(0.6, 12.0, -2.6),
        Vec3::new(0.6, 0.5, -2.6),
        t54_at_origin(HullPose::level(0.0)),
    );
    assert_eq!(zone, ArmorZone::Roof, "the deck is roof plate, not a phantom side");
    assert!(angle < 1.0, "a vertical drop is square-on to the roof (angle vs UP), got {angle}");
}
