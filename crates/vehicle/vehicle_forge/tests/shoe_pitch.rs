//! THE SHOE PITCH LOCK: a rendered track shoe is the length the real one was.
//!
//! Measured 2026-08-08, before this file existed. Three vehicles authored their documented shoe
//! count and landed within a couple of millimetres of the real track; the five that did not
//! rendered shoes **1.65 to 2.05 times too long** — a belt reading as a chunky chain instead of a
//! track, on more than half the fleet.
//!
//! | vehicle | authored | rendered pitch | real |
//! |---|---|---:|---:|
//! | T-54 | yes (90) | 137 mm | 137 (OMSh) |
//! | Tiger I | yes (96) | 130 mm | 130 (Kgs 63/725/130) |
//! | IS-3 | yes (86) | 157 mm | ~162 |
//! | Tiger II | **no** | **301 mm** | ~152 |
//! | Jagdtiger | **no** | **303 mm** | ~152 |
//! | Panther II | **no** | **307 mm** | 150 |
//! | Centurion | **no** | **288 mm** | ~152 |
//! | T-34-85 | **no** | **283 mm** | 172 |
//!
//! One line caused it. `RunningGearKinematics::from_track` derived the count from a STADIUM loop
//! — `4 * half_run + 2 * PI * end_radius` — while `running_gear_place` spread those links over the
//! true ramped `belt_length()`. Every vehicle here carries its idler and sprocket beyond and above
//! the road-wheel line, so the stadium is short by about a quarter and the shoes stretched to
//! cover it. Nothing failed: the belt still closed, the budget still held, and it simply looked
//! coarse.

use game_core::{VehicleKind, VehicleBlueprint};
use vehicle_geometry::RunningGearKinematics;

/// Rendered shoe pitch in metres: the loop the links actually ride, over the links riding it.
fn rendered_pitch(kind: VehicleKind) -> f32 {
    let kinematics = RunningGearKinematics::for_vehicle(kind).expect("every vehicle has gear");
    kinematics.belt_length() / kinematics.link_count().max(1) as f32
}

/// The band a Second World War era tank track lives in. The narrowest here is the Tiger I's Kgs
/// 63/725/130 at 130 mm and the widest the T-34's waffle at 172; nothing in this fleet is outside
/// it, and a number outside it is not a track shoe.
const MIN_PITCH_M: f32 = 0.110;
const MAX_PITCH_M: f32 = 0.200;

#[test]
fn every_vehicle_renders_a_shoe_the_size_a_shoe_is() {
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let pitch = rendered_pitch(kind);
        checked += 1;
        assert!(
            (MIN_PITCH_M..=MAX_PITCH_M).contains(&pitch),
            "{kind:?}: renders a {:.0} mm shoe. Era track runs {:.0}-{:.0} mm; anything outside \
             that is a chain, not a belt.",
            pitch * 1000.0,
            MIN_PITCH_M * 1000.0,
            MAX_PITCH_M * 1000.0
        );
    }
    // The floor the ratchet asks for: a walk that checked nothing would otherwise pass.
    assert_eq!(checked, VehicleKind::PLAYABLE.len(), "every playable vehicle must be walked");
}

/// The whole fleet authors its count now. The fallback is for a vehicle nobody has looked up yet,
/// and it exists to be wrong by rounding rather than by a factor of two — but a shipped vehicle
/// should never be relying on it.
#[test]
fn no_shipped_vehicle_leaves_its_shoe_count_to_the_fallback() {
    let mut unauthored = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let blueprint = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        if blueprint.track.link_count.is_none() {
            unauthored.push(format!("{kind:?}"));
        }
    }
    assert!(
        unauthored.is_empty(),
        "these vehicles have no documented shoe count and are falling back: {unauthored:?} — look \
         the track up; it is the one number that decides whether a belt reads as a belt."
    );
}

/// The fallback must measure the loop the links ACTUALLY ride. This is the defect itself, held as
/// a test: a stadium approximation of a ramped belt is a quarter short, and the pitch it implies
/// is a quarter long.
#[test]
fn the_fallback_measures_the_ramped_loop_not_a_stadium() {
    for kind in VehicleKind::PLAYABLE {
        let blueprint = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        let track = &blueprint.track;
        let half_run = (track.wheel_last_z - track.wheel_first_z) * 0.5;
        let stadium = 4.0 * half_run + 2.0 * std::f32::consts::PI * track.end_radius.max(0.05);
        let real = RunningGearKinematics::for_vehicle(kind).expect("gear").belt_length();
        assert!(
            real > stadium * 1.05,
            "{kind:?}: the ramped loop ({real:.2} m) should exceed the stadium ({stadium:.2} m) — \
             if it ever does not, this vehicle's end wheels sit on the axle line and the two \
             agree, which is worth knowing before trusting either."
        );
    }
}
