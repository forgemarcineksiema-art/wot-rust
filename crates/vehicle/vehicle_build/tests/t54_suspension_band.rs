//! The suspension band works as a SYSTEM: the shelf rides HIGH at its measured 1.35 sheet line
//! with ~0.30 m of daylight over the crest links (the light through under the fenders every
//! side photograph shows), the stowage on it closes the silhouette at the roof line, the
//! guards over the end wheels FALL from that shelf into their flaps, and the tub wall between
//! the wheels carries its machinery (the pivot-boss row, the bump stops).
//!
//! RE-MEASURED 2026-08-12 (second pass over the same sheet, both projections): the first pass
//! scanned the track CREST's line, called it the fender sheet, parked the shelf on the crest
//! and LOCKED the error ("the crest links kiss the shelf"). The kiss was the defect. Locks
//! still measure the OUTCOME, on placed links and finished parts — but now they hold the
//! daylight the reference actually shows.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::t54_description;
use vehicle_geometry::{GearPart, RunningGearKinematics, running_gear_placements};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint")
}

/// Link half-height above its centreline, measured off the unit mesh once — the same body the
/// renderer places.
fn link_top_half(kin: &RunningGearKinematics) -> f32 {
    vehicle_geometry::track_link_unit_mesh(kin).bounds().expect("link bounds").max.y
}

/// The shelf rides DAYLIGHT over the crest: between the crest links and the fender lip there is
/// ~0.30 m of open air — the light through under the fenders that both projections of the
/// reference sheet (and every side photograph) show. The suspension, the sag and the hull side
/// live in that band. Too small is the shelf parked back on the crest (the 2026-08-12 mis-scan
/// this replaces — its "kiss" lock demanded 5-50 mm and was the defect itself); too large means
/// the shelf has drifted past the sheet line toward the roof.
#[test]
fn the_shelf_rides_daylight_over_the_crest() {
    let bp = blueprint();
    let v = bp.complete_visual().expect("visual");
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("gear");
    let link_half = link_top_half(&kin);
    let crest_top = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::Link)
        .map(|p| p.transform.w_axis.truncate())
        .filter(|w| w.x > 0.0 && w.y > kin.cy)
        .filter(|w| kin.wheel_zs.iter().any(|&z| (w.z - z).abs() < 0.10))
        .map(|w| w.y + link_half)
        .fold(f32::NEG_INFINITY, f32::max);
    let lip_bottom = v.fender.center_y - v.fender.half.y - v.detail.fender_lip_drop;
    let gap = lip_bottom - crest_top;
    assert!(
        (0.25..=0.45).contains(&gap),
        "the shelf rides daylight over the crest: gap {gap:.3} (crest top {crest_top:.3}, lip \
         {lip_bottom:.3}) — under 0.25 the shelf is back on the crest, over 0.45 it has left \
         the sheet line"
    );
}

/// The mudguards continue the HIGH shelf over the end wheels: four swept guards, each clearing
/// its wrap's link line, each cresting just over the corrected 1.35 sheet plane (a gentle crown,
/// not the old kicked-up hump — that hump only existed because the shelf itself was parked on
/// the crest) before falling into the hanging flap.
#[test]
fn the_mudguards_arch_over_the_end_wheels() {
    let bp = blueprint();
    let description = t54_description();
    let (idler_z, idler_r) = bp.track.end_front.unwrap_or((bp.track.end_z, bp.track.end_radius));
    let ends = [
        ("mudguard_bow", idler_z, idler_r),
        ("mudguard_tail", bp.track.end_z, bp.track.end_radius),
    ];
    for (name, axle_z, wheel_r) in ends {
        let guards: Vec<_> = description.parts.iter().filter(|p| p.key.name == name).collect();
        assert_eq!(guards.len(), 2, "{name}: one guard per side");
        // The loop's link line over the wrap: axle height + wrap radius + a link body.
        let wrap_links = bp.track.end_y + wheel_r + 0.02 + 0.07;
        for guard in guards {
            let mesh = guard.mesh();
            let over_wrap_min = mesh
                .vertices()
                .iter()
                .filter(|v| (v.position.z.abs() - axle_z).abs() < 0.20)
                .map(|v| v.position.y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                over_wrap_min > wrap_links - 0.005,
                "{name}: the guard clears the wrap's links: sheet at {over_wrap_min:.3} vs \
                 links {wrap_links:.3}"
            );
            let peak =
                mesh.vertices().iter().map(|v| v.position.y).fold(f32::NEG_INFINITY, f32::max);
            assert!(
                (1.34..=1.40).contains(&peak),
                "{name}: the guard crests just over the 1.35 sheet plane, got {peak:.3}"
            );
        }
    }
}

/// The lever shock absorbers ride the DAMPED stations (blueprint `damper_stations` — the
/// T-54's 1st and 5th wheels): one per damped wheel per side, anchored above the arm pivot
/// with the lever reaching for the axle line. They are ANIMATED gear, not hull furniture,
/// because a damper spans hull to MOVING axle — the constraint `suspension_furniture` has
/// documented since it declined to fake them statically. Un-authored vehicles draw none.
#[test]
fn the_dampers_ride_the_damped_stations() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("gear");
    let placements = running_gear_placements(&kin, 0.0, 0.0);
    let dampers: Vec<_> = placements
        .iter()
        .filter(|p| matches!(p.part, GearPart::Damper | GearPart::DamperLeft))
        .collect();
    assert_eq!(dampers.len(), 4, "two damped stations per side");
    for p in &dampers {
        let w = p.transform.w_axis.truncate();
        let near_station = kin
            .damper_stations
            .iter()
            .filter_map(|&s| kin.wheel_zs.get(s))
            .any(|&z| (w.z - (z + kin.arm_reach + 0.15)).abs() < 0.02);
        assert!(near_station, "a damper anchors at a damped station, got z {:.3}", w.z);
        assert!(w.y > kin.cy + kin.arm_rise + 0.05, "anchored above the arm pivot: y {:.3}", w.y);
        assert!(
            (w.x < 0.0) == matches!(p.part, GearPart::DamperLeft),
            "the -X flank instances the mirrored damper, like the arms"
        );
    }
}

/// The pivot-boss row sits ON the arm pivots: ten bosses, each on the same axle the animated
/// arm swings about — one source (`wheel_stations` + `arm_reach`/`arm_rise`), two readers, and
/// this lock holding them to one another. Bare tub wall between the wheels was the tell that
/// nothing anchored the arms to the hull.
#[test]
fn the_pivot_bosses_sit_on_the_arm_pivots() {
    let bp = blueprint();
    let description = t54_description();
    let bosses: Vec<_> =
        description.parts.iter().filter(|p| p.key.name == "suspension_pivot_boss").collect();
    assert_eq!(bosses.len(), 10, "five pivot bosses per side");
    let pivot_y = bp.track.axle_y() + bp.track.arm_rise();
    for boss in bosses {
        let b = boss.mesh().bounds().expect("boss bounds");
        let (cy, cz) = ((b.min.y + b.max.y) * 0.5, (b.min.z + b.max.z) * 0.5);
        assert!(
            bp.track
                .wheel_stations()
                .iter()
                .any(|&s| (cz - (s + bp.track.arm_reach())).abs() < 0.01),
            "a boss rides an arm pivot station, got z {cz:.3}"
        );
        assert!((cy - pivot_y).abs() < 0.01, "and the pivot height, got y {cy:.3} vs {pivot_y:.3}");
    }
    let stops = description.parts.iter().filter(|p| p.key.name == "suspension_bump_stop").count();
    assert_eq!(stops, 4, "bump stops over the first and last arms, both sides");
}
