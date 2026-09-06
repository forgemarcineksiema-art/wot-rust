//! The Tiger II shape cage: locks the sloped-school anatomy the blueprint migration bought —
//! the fleet's longest glacis and leaned upper sides standing ON the armor planes, the faceted
//! Henschel prism with its bustle closing the rear armor plane, the overlapped nine-wheel
//! rows, and the five-metre braked KwK 43. Each lock names a defect that would silently
//! un-King-Tiger the tank.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::TigerII).expect("Tiger II has a blueprint")
}

/// The W1 dressing locks (dossier PR-T2.1/T2.2): Schuerzen hanging on the spaced-armor plane,
/// the wide Turmblende band on the gun, hinged bow flaps drooping over the FRONT drive
/// sprockets, and twin exhaust stacks with OPEN dark mouths. Each cites a dossier number;
/// losing any of them silently un-King-Tigers the flank, bow, or tail read.
#[test]
fn the_w1_dressing_holds_skirts_turmblende_flaps_and_exhausts() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::TigerII).expect("Tiger II bakes");
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun submesh").mesh;

    // Schuerzen: a skirt plate stands at outer_x + standoff (the spaced-armor plane), and the
    // dressed beam is the dossier's 3.88 m over fitted skirts.
    let skirt = bp.hull.skirt.expect("the Tiger II carries Schuerzen (PR-T2.2)");
    let skirt_x = bp.track.outer_x + skirt.standoff_m;
    let skirt_span_z: Vec<f32> = hull
        .vertices()
        .iter()
        .filter(|v| (v.position.x.abs() - skirt_x).abs() < skirt.thickness_m + 0.005)
        .map(|v| v.position.z)
        .collect();
    assert!(
        !skirt_span_z.is_empty(),
        "no skirt plate found on the spaced-armor plane x = {skirt_x:.3}"
    );
    let skirt_run = skirt_span_z.iter().copied().fold(f32::MIN, f32::max)
        - skirt_span_z.iter().copied().fold(f32::MAX, f32::min);
    assert!(skirt_run > 4.0, "the Schuerzen run covers the upper wheel run (got {skirt_run:.2} m)");
    let beam = hull.bounds().expect("hull bounds").max.x * 2.0;
    assert!(
        (beam - 3.88).abs() < 0.05,
        "dressed beam {beam:.3} should be the dossier's 3.88 m over fitted skirts"
    );

    // Turmblende: the mantlet band spans well over a metre — a round collar reads Panther,
    // not Serienturm (real band ~1.4 m over the 0.6 m generic collar).
    let gun_bounds = gun.bounds().expect("gun bounds");
    let mantlet_width = gun_bounds.max.x - gun_bounds.min.x;
    assert!(
        mantlet_width > 1.25,
        "Turmblende band {mantlet_width:.2} m — the wide mask is the Serienturm's face"
    );

    // Bow flaps: hinged plates droop ahead of the sprocket axle over each track band.
    let flap_vertices = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.position.z > bp.track.end_z
                && (v.position.x.abs() - bp.track.center_x).abs() < 0.45
                && v.position.y < bp.hull.sponson_y + 0.03
                && v.position.y > bp.hull.sponson_y - 0.30
        })
        .count();
    assert!(
        flap_vertices > 0,
        "no bow fender flap ahead of the front sprocket (F3 — the German bow signature)"
    );

    // Exhausts: the stacks end in OPEN dark mouths (TrackMetal) above the deck at the tail.
    let open_mouths = hull
        .vertices()
        .iter()
        .filter(|v| {
            v.material == vehicle_geometry::MaterialRole::TrackMetal
                && v.position.y > bp.hull.deck_y + 0.10
                && v.position.z < -bp.hull.half_len + 0.35
        })
        .count();
    assert!(
        open_mouths > 0,
        "exhaust stacks lost their open mouths — a capped rod is not a pipe (audit #5)"
    );
}
