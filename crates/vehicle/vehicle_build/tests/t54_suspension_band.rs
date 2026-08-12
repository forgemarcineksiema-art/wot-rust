//! The suspension band works as a SYSTEM: the shelf sits at the sheet line with the crest
//! links nearly brushing it (daylight lives in the scallop valleys, nowhere else), the
//! mudguards ARCH over the end wheels instead of a full-length shelf clearing the wraps, and
//! the tub wall between the wheels carries its machinery (the pivot-boss row, the bump stops).
//!
//! This is the band the 2026-08-12 review called out: the sag alone was fixed while the stack
//! stayed exploded — a uniform slot above the whole run, a shelf a hand too high, slabs over
//! the ends, bare plate between the wheels. Locks measure the OUTCOME, on placed links and
//! finished parts.

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

/// The crest links KISS the shelf: over the road-wheel stations the top run rises to within a
/// few centimetres of the fender lip — the compressed stack every reference shows. Too small is
/// z-fighting; too large is the old floating band.
#[test]
fn the_crest_links_kiss_the_shelf() {
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
        (0.005..=0.050).contains(&gap),
        "the crest links kiss the shelf: gap {gap:.3} (crest top {crest_top:.3}, lip \
         {lip_bottom:.3}) — under 5 mm z-fights, over 50 mm is the floating band again"
    );
}

/// The mudguards arch OVER the end wheels: four swept guards, each clearing its wrap's link
/// line, each peaking in the three-view's 1.15-1.24 band — the kicked-up bow guard every
/// reference shows, not a slab angled at the dirt.
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
                (1.15..=1.24).contains(&peak),
                "{name}: the arch peaks in the three-view's band, got {peak:.3}"
            );
        }
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
