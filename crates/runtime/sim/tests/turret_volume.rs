//! Locks for the two-volume tank hit model: hits above the armor split only connect inside the
//! turret's own (traversing) box, the hull deck is thin roof plate rather than phantom turret
//! side, and everything below the split behaves exactly as the single-box model did.

use std::f32::consts::FRAC_PI_2;

use game_core::math::HullPose;
use game_core::{ArmorFacing, ArmorZone, HitboxProfile, TankId, VehicleKind};
use glam::Vec3;
use sim::{SegmentImpact, ShellTraceWorld, TraceTank, segment_impact};

fn t54_at_origin(turret_yaw_rad: f32) -> TraceTank {
    TraceTank::for_kind(
        TankId(1),
        Vec3::ZERO,
        HullPose::level(0.0),
        turret_yaw_rad,
        VehicleKind::T54_1951,
    )
}

fn world(tank: &TraceTank) -> ShellTraceWorld<'_> {
    ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: std::slice::from_ref(tank),
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
    }
}

/// The old single-box model resolved this as a `TurretSide` hit on empty air: the segment crosses
/// the hull plan at turret height, but visually passes *beside* the turret. It must fly on.
#[test]
fn shot_beside_the_turret_at_turret_height_passes_over_the_hull() {
    let tank = t54_at_origin(0.0);
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    // Between the turret plan (0.95) and the hull plan (1.75), above the split (world y 1.80).
    let x = (hitbox.turret_half_width_m + hitbox.half_width_m) * 0.5;
    let outcome = segment_impact(
        Vec3::new(x, 2.0, -10.0),
        Vec3::new(x, 2.0, 10.0),
        Vec3::new(0.0, 0.0, 300.0),
        &world(&tank),
    );

    assert_eq!(outcome, None, "shot beside the turret must pass over the hull deck");
}

/// The same lane below the split is still a hull-side hit — the slab did not shrink.
#[test]
fn shot_below_the_split_still_lands_on_the_hull_side() {
    let tank = t54_at_origin(0.0);
    // 1.3 m: above the T-54's sponson step (1.0 m) and its 810 mm wheels' track band, below
    // the armor split — the bare upper hull side.
    let outcome = segment_impact(
        Vec3::new(-10.0, 1.3, 0.0),
        Vec3::new(10.0, 1.3, 0.0),
        Vec3::new(300.0, 0.0, 0.0),
        &world(&tank),
    );

    match outcome {
        Some(SegmentImpact::Tank { zone, facing, .. }) => {
            assert_eq!(zone, ArmorZone::HullSide);
            assert_eq!(facing, ArmorFacing::HullSide);
        }
        other => panic!("expected a hull-side hit, got {other:?}"),
    }
}

/// Head-on turret shots: a shot down the gun line lands on the mantlet ball (a real circular
/// patch on the casting, not a width band), a cheek shot on plain turret front.
#[test]
fn turret_front_keeps_its_mantlet_and_front_bands() {
    let tank = t54_at_origin(0.0);
    // The T-54 gun line: trunnion height 1.80 — the mantlet patch is centered on it.
    let center = segment_impact(
        Vec3::new(0.0, 1.78, 10.0),
        Vec3::new(0.0, 1.78, -10.0),
        Vec3::new(0.0, 0.0, -300.0),
        &world(&tank),
    );
    match center {
        Some(SegmentImpact::Tank { zone, facing, .. }) => {
            assert_eq!(zone, ArmorZone::Mantlet);
            assert_eq!(facing, ArmorFacing::TurretFront);
        }
        other => panic!("expected a mantlet hit, got {other:?}"),
    }

    let off_center = segment_impact(
        Vec3::new(0.7, 1.78, 10.0),
        Vec3::new(0.7, 1.78, -10.0),
        Vec3::new(0.0, 0.0, -300.0),
        &world(&tank),
    );
    match off_center {
        Some(SegmentImpact::Tank { zone, .. }) => assert_eq!(zone, ArmorZone::TurretFront),
        other => panic!("expected a turret-front hit, got {other:?}"),
    }
}

/// A plunging hit on the deck beside the turret lands on thin roof plate — previously it read as
/// a full-thickness turret side.
#[test]
fn plunging_hit_on_the_deck_beside_the_turret_is_a_hull_deck_hit() {
    let tank = t54_at_origin(0.0);
    // The T-54's ⌀2.25 dome overhangs its narrow 1.05 m hull laterally, so "beside the
    // turret" lives along Z: drop the shot on the rear deck behind the dome's plan.
    let outcome = segment_impact(
        Vec3::new(0.5, 5.0, -2.0),
        Vec3::new(0.5, 1.0, -2.0),
        Vec3::new(0.0, -300.0, 0.0),
        &world(&tank),
    );

    match outcome {
        Some(SegmentImpact::Tank { zone, hit_position, .. }) => {
            // `HullDeck`, not `Roof` — the deck and the turret roof are different plates
            // on different structures, and since v40 they are different zones.
            assert_eq!(zone, ArmorZone::HullDeck);
            // The deck is the REAL blueprint hull roof, not the hitbox split plane the old
            // box model used.
            let deck_y = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
                .expect("blueprint")
                .hull
                .deck_y;
            assert!(
                (hit_position.y - deck_y).abs() < 1.0e-3,
                "deck hit should land on the hull roof plane, got y {:.3}",
                hit_position.y
            );
        }
        other => panic!("expected a deck roof hit, got {other:?}"),
    }
}

/// The turret volume traverses with the turret: a lane that clips the long Tiger II turret at
/// zero traverse is open deck once the turret swings 90 degrees away.
#[test]
fn turret_traverse_swings_the_hittable_volume() {
    let lane = |tank: &TraceTank| {
        segment_impact(
            Vec3::new(-10.0, 2.5, 1.3),
            Vec3::new(10.0, 2.5, 1.3),
            Vec3::new(300.0, 0.0, 0.0),
            &world(tank),
        )
    };

    let forward =
        TraceTank::for_kind(TankId(1), Vec3::ZERO, HullPose::level(0.0), 0.0, VehicleKind::TigerII);
    match lane(&forward) {
        Some(SegmentImpact::Tank { zone, .. }) => assert_eq!(zone, ArmorZone::TurretSide),
        other => panic!("expected a turret-side hit at zero traverse, got {other:?}"),
    }

    let traversed = TraceTank::for_kind(
        TankId(1),
        Vec3::ZERO,
        HullPose::level(0.0),
        FRAC_PI_2,
        VehicleKind::TigerII,
    );
    assert_eq!(
        lane(&traversed),
        None,
        "with the turret swung away the same lane crosses open deck"
    );
}
