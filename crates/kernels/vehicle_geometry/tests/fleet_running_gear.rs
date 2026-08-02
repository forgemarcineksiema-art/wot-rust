//! THE RUNNING-GEAR SOLIDITY CONTRACT, RUN OVER THE FLEET.
//!
//! Three benchmarks already say "discs must not merge" — `tiger_ii`, `panther_ii` and `jagdtiger`
//! each assert their Schachtellaufwerk stagger is at least a wheel's width, because their wheels
//! deliberately overlap in Z and only the inboard offset keeps them apart. The rule they state is
//! general (two solid discs cannot occupy the same metal) but it was only ever checked where the
//! author already knew it was in play, and only between road wheels. Nothing checked a wheel
//! against a return roller, a wheel against the idler it sits next to, or a layout whose wheels
//! overlap without any stagger to save them.
//!
//! So the Centurion shipped with its Horstmann pairs interpenetrating by 0.20 m — a quarter of a
//! wheel diameter, visible below the skirt line — plus its middle return roller buried 0.065 m
//! into both centre wheels, while every ratio, budget, silhouette and determinism gate stayed
//! green. This file is the general gate: every disc of every vehicle, against every other disc
//! that shares its plane.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_geometry::RunningGearKinematics;

/// Discs are allowed to TOUCH: a road wheel tangent to its neighbour is how a tight bogie really
/// looks, and floating-point ends of a hand-authored layout land a millimetre either side of
/// contact. Past this, metal is inside metal.
const TOUCH_TOLERANCE_M: f32 = 0.01;

/// Interpenetration that exists TODAY and is recorded rather than asserted away, in the
/// FLOOR/TARGET form the art-direction programme uses: the depth is a CEILING, so the defect can
/// only ever shrink, and it is printed on every run so it cannot be forgotten.
///
/// The T-34-85's first and last road wheels sit 0.09 m inside the idler and drive sprocket they
/// flank. Pulling them apart means either moving the end wheels (which the belt path, the wrap
/// and the documented 4.15 m ground run all read) or shrinking the ⌀0.83 discs its own benchmark
/// locks as BIG — a shape decision on the T-34, not a mechanical repair, so it is booked here
/// instead of being guessed at.
const RECORDED_MERGE_CEILING: &[(VehicleKind, f32)] = &[(VehicleKind::T34_85, 0.09)];

fn ceiling(kind: VehicleKind) -> f32 {
    RECORDED_MERGE_CEILING.iter().find(|(k, _)| *k == kind).map_or(0.0, |(_, allowed)| *allowed)
}

/// One solid of revolution in a track's side plane.
struct Disc {
    what: String,
    z: f32,
    y: f32,
    radius: f32,
    /// How far inboard this disc's plane sits (the Schachtellaufwerk stagger).
    inboard_dx: f32,
    half_width: f32,
}

/// Every disc one side of the running gear turns: road wheels on the axle line, return rollers on
/// theirs, and the two end wheels.
fn discs(kin: &RunningGearKinematics) -> Vec<Disc> {
    let mut discs = Vec::new();
    for (index, &z) in kin.wheel_zs.iter().enumerate() {
        discs.push(Disc {
            what: format!("road wheel {index}"),
            z,
            y: kin.cy,
            radius: kin.wheel_radius,
            // The interleaved layouts push every odd wheel inboard onto its own row.
            inboard_dx: if index % 2 == 1 { kin.wheel_overlap_dx } else { 0.0 },
            half_width: kin.wheel_half_width,
        });
    }
    for (index, &z) in kin.roller_zs.iter().enumerate() {
        discs.push(Disc {
            what: format!("return roller {index}"),
            z,
            y: kin.roller_y,
            radius: kin.roller_radius,
            inboard_dx: 0.0,
            half_width: kin.wheel_half_width,
        });
    }
    let (front, rear) = if kin.drive_front { ("sprocket", "idler") } else { ("idler", "sprocket") };
    for (what, z) in [(front, kin.end_cz), (rear, -kin.end_cz)] {
        discs.push(Disc {
            what: what.to_string(),
            z,
            y: kin.end_cy,
            radius: kin.end_radius,
            inboard_dx: 0.0,
            half_width: kin.wheel_half_width,
        });
    }
    discs
}

/// The deepest interpenetration between two discs that share a plane, and what they are.
fn worst_merge(kin: &RunningGearKinematics) -> (f32, String) {
    let discs = discs(kin);
    let mut worst = (0.0_f32, "nothing merges".to_string());
    for (index, a) in discs.iter().enumerate() {
        for b in discs.iter().skip(index + 1) {
            // Discs on genuinely separate rows cannot merge, whatever they do in Z: that is the
            // whole point of the German line's inboard stagger.
            if (a.inboard_dx - b.inboard_dx).abs() >= a.half_width + b.half_width {
                continue;
            }
            let (dz, dy) = (a.z - b.z, a.y - b.y);
            let merge = a.radius + b.radius - (dz * dz + dy * dy).sqrt();
            if merge > worst.0 {
                worst = (merge, format!("{} x {}", a.what, b.what));
            }
        }
    }
    worst
}

/// No two discs sharing a plane may occupy the same metal. A merged pair is not a shading
/// subtlety: it is a wheel visibly sunk into its neighbour, and it means the layout is not one a
/// real suspension could be built to.
#[test]
fn no_running_gear_disc_merges_into_another() {
    let mut gears_checked = 0;
    for kind in VehicleKind::ALL {
        let Some(kin) = RunningGearKinematics::for_vehicle(kind) else {
            continue; // The test-only prototype keeps fused static gear.
        };
        gears_checked += 1;
        let (merge, what) = worst_merge(&kin);
        let allowed = ceiling(kind) + TOUCH_TOLERANCE_M;
        assert!(
            merge <= allowed,
            "{kind:?}: {what} interpenetrate by {merge:.3} m, past the allowance {allowed:.3} — \
             two solid discs cannot share metal"
        );
        if merge > TOUCH_TOLERANCE_M {
            println!(
                "GEAR DEBT {kind:?}: {what} merge {merge:.3} m (ceiling {:.3}, target 0)",
                ceiling(kind)
            );
        }
    }
    assert_eq!(
        gears_checked,
        VehicleKind::PLAYABLE.len(),
        "every blueprint-born vehicle has running gear; skipping one hides it from this rule"
    );
}

/// A recorded ceiling that no longer bites is a lie about the state of the fleet: it reads as
/// "this is still broken" long after someone fixed it. Every entry must still be reached.
#[test]
fn every_recorded_merge_ceiling_is_still_earned() {
    for (kind, allowed) in RECORDED_MERGE_CEILING {
        let kin = RunningGearKinematics::for_vehicle(*kind).expect("recorded vehicle has gear");
        let (merge, what) = worst_merge(&kin);
        assert!(
            (merge - allowed).abs() <= 0.005,
            "{kind:?} now merges {merge:.3} m ({what}), not the recorded {allowed:.3} — if this \
             is a FIX, drop the entry; if it is a regression, the other gate already told you"
        );
    }
}

/// The ground run is where the tracks touch the world, and the blueprint says where that is:
/// `bottom_y`. The belt path puts it at `axle - wheel_radius - seat`, so a wheel diameter that
/// does not reach the authored belt bottom silently lifts the whole vehicle off its own tracks.
/// Booked, not asserted, for the fleet: the IS-3 draws its ground run at 0.11, a full 0.08 above
/// the 0.03 it authors, and closing that is a shape decision on that vehicle.
#[test]
fn the_authored_belt_bottom_is_reported_against_the_drawn_one() {
    let mut belts_reported = 0;
    for kind in VehicleKind::ALL {
        let (Some(kin), Some(bp)) =
            (RunningGearKinematics::for_vehicle(kind), VehicleBlueprint::for_vehicle(kind))
        else {
            continue;
        };
        belts_reported += 1;
        // `LINK_SEAT` (0.02) is the belt module's private constant; the drawn ground run sits
        // that far under the wheel rim.
        let drawn = kin.cy - kin.wheel_radius - 0.02;
        let lift = drawn - bp.track.bottom_y;
        if lift.abs() > 0.02 {
            println!(
                "GEAR DEBT {kind:?}: ground run drawn {lift:+.3} m off the authored belt bottom"
            );
        }
    }
    assert_eq!(
        belts_reported,
        VehicleKind::PLAYABLE.len(),
        "every blueprint-born vehicle has running gear; skipping one hides it from this rule"
    );
}
