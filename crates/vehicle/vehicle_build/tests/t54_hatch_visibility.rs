//! The hatch hardware is VISIBLE: collars and hinges stand proud of the metal they seat on,
//! measured against the built meshes — not against the lid they belong to.
//!
//! The 2026-08-10 audit found 412 triangles of hatch hardware rendering zero pixels: the cupola's
//! coaming lay entirely inside the cupola drum, and the loader's coaming AND hinge lay under the
//! dome roof. `every_hatch_carries_a_coaming_a_hinge_and_a_handle` was green the whole time,
//! because it measured the collar against the LID — and a collar chained to a lid that is
//! deliberately rooted deep into the casting follows it underground. The relation that matters is
//! collar-to-METAL, and the metal is what this file reads.

use vehicle_build::t54_description;
use vehicle_geometry::MeshBounds;

fn bounds_of(name: &str, instance: Option<u16>) -> MeshBounds {
    let description = t54_description();
    let mut acc: Option<MeshBounds> = None;
    for part in description.parts.iter().filter(|p| {
        p.key.name == name && instance.map(|wanted| p.key.instance == wanted).unwrap_or(true)
    }) {
        if let Some(b) = part.mesh().bounds() {
            acc = Some(match acc {
                None => b,
                Some(mut a) => {
                    a.include(b.min);
                    a.include(b.max);
                    a
                }
            });
        }
    }
    acc.unwrap_or_else(|| panic!("{name} (instance {instance:?}) exists and has geometry"))
}

/// Collar tops clear the surface each hatch is cut into. The surfaces are read off the BUILT
/// parts — the cupola drum's top, the dome's roof, the hull's roof — so a reshaped casting moves
/// the bar with it.
#[test]
fn every_hatch_collar_stands_proud_of_the_metal_it_seats_on() {
    let cupola_top = bounds_of("cupola", None).max.y;
    let dome_roof = bounds_of("turret_shell", None).max.y;
    let hull_roof = bounds_of("upper_hull", None).max.y;

    let cases = [
        ("cupola_hatch", cupola_top, "the cupola drum's top rim"),
        ("loader_hatch", dome_roof, "the dome's roof plate"),
        ("driver_hatch", hull_roof, "the hull roof"),
    ];
    for (hatch, surface, what) in cases {
        let collar = bounds_of(hatch, Some(100));
        assert!(
            collar.max.y > surface + 0.010,
            "{hatch}: the collar tops out at {:.3} against {what} at {surface:.3} — buried \
             hardware is triangles rendering the inside of a casting",
            collar.max.y
        );
        // And it is WELDED there, not floating over it: the collar's base bites the surface.
        assert!(
            collar.min.y < surface + 0.005,
            "{hatch}: the collar's base hangs at {:.3}, clear off {what} at {surface:.3}",
            collar.min.y
        );
    }
}

/// The loader's hinge swings a lid that opens ABOVE the roof, so its barrels sit above the roof.
/// It used to lie at 2.367..2.395 under a 2.40 plate — a hinge nobody could see or use.
#[test]
fn the_loader_hinge_rides_above_the_dome_roof() {
    let dome_roof = bounds_of("turret_shell", None).max.y;
    let hinge = bounds_of("loader_hatch", Some(101));
    assert!(
        hinge.max.y > dome_roof + 0.005,
        "the loader's hinge tops out at {:.3} under the {dome_roof:.3} roof",
        hinge.max.y
    );
}

/// The cupola carries the ring of vision blocks it exists for: five devices under the top rim,
/// each rooted into the drum with its glass standing proud of the hood's face.
#[test]
fn the_cupola_carries_vision_blocks_around_its_drum() {
    let description = t54_description();
    let cupola = bounds_of("cupola", None);
    let (cx, cz) = ((cupola.min.x + cupola.max.x) * 0.5, (cupola.min.z + cupola.max.z) * 0.5);
    let drum_radius = (cupola.max.x - cupola.min.x) * 0.5;

    let mut hoods = 0usize;
    let mut panes = 0usize;
    for part in description.parts.iter().filter(|p| p.key.name == "cupola_vision_block") {
        let b = part.mesh().bounds().expect("block bounds");
        let centre_r = {
            let (bx, bz) = ((b.min.x + b.max.x) * 0.5, (b.min.z + b.max.z) * 0.5);
            ((bx - cx).powi(2) + (bz - cz).powi(2)).sqrt()
        };
        // On the drum's wall: near its radius, inside its height band.
        assert!(
            (drum_radius * 0.75..drum_radius * 1.30).contains(&centre_r),
            "a vision block sits at radius {centre_r:.3} on a {drum_radius:.3} drum — off the wall"
        );
        assert!(
            b.max.y < cupola.max.y && b.min.y > cupola.min.y,
            "a vision block leaves the drum's height band: {:.3}..{:.3} vs {:.3}..{:.3}",
            b.min.y,
            b.max.y,
            cupola.min.y,
            cupola.max.y
        );
        if part.key.instance < 8 {
            hoods += 1;
            // Rooted: the hood reaches INTO the drum, so it is a device set into armour rather
            // than a crouton glued onto it — the defect class the periscopes already burned.
            let nearest_r = nearest_plan_radius(&b, cx, cz);
            assert!(
                nearest_r < drum_radius,
                "a hood floats {nearest_r:.3} from the axis of a {drum_radius:.3} drum"
            );
        } else {
            panes += 1;
        }
    }
    assert!(hoods >= 4, "a commander's cupola carries a ring of devices; found {hoods}");
    assert_eq!(hoods, panes, "every hood carries its glass");
}

/// The plan-space distance from the drum axis to the nearest corner of a block's bounds.
fn nearest_plan_radius(b: &MeshBounds, cx: f32, cz: f32) -> f32 {
    let mut nearest = f32::MAX;
    for x in [b.min.x, b.max.x] {
        for z in [b.min.z, b.max.z] {
            nearest = nearest.min(((x - cx).powi(2) + (z - cz).powi(2)).sqrt());
        }
    }
    nearest
}
