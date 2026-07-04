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
    // The nose-up pose moves the bow, so resolve where the upper glacis actually sits (a fixed
    // hull-local point on the front face carried through the pose) and fire flat through it.
    let nose_up = HullPose { yaw_rad: 0.0, pitch_rad: 0.3, roll_rad: 0.0 };
    let spec = TankSpec::t54_1951();
    let glacis_local = Vec3::new(0.0, 0.3, spec.hitbox.half_length_m - 0.001);
    let glacis_world =
        nose_up.basis() * (Vec3::Y * spec.hitbox.center_y_m) + nose_up.basis() * glacis_local;
    let (zone, angle) = hit(
        Vec3::new(0.0, glacis_world.y, glacis_world.z + 8.0),
        Vec3::new(0.0, glacis_world.y, glacis_world.z - 2.0),
        t54_at_origin(nose_up),
    );
    assert_eq!(zone, ArmorZone::UpperGlacis);
    assert!(
        angle > glacis_slope() + 12.0,
        "a ~17° nose-up pose adds its pitch to the effective slope, got {angle}"
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
