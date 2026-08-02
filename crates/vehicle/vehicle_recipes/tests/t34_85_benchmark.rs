//! The T-34-85's shape cage: each test names a defect that would un-T-34 the tank. The first
//! vehicle authored through the Forge Studio loop — its shape lives in
//! `blueprints/t34_85.blueprint.ron`, and these are the cage bars around that file.

use game_core::{TurretForm, VehicleBlueprint, VehicleKind};
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::T34_85).expect("T-34-85 has a blueprint")
}

/// Slope IS the armour: the 60-degree glacis over a single flat bow (no pike), with the raked
/// sides the armour model reads at the same angle the eye sees.
#[test]
fn the_glacis_is_the_sixty_degree_bet() {
    let bp = blueprint();
    assert_eq!(bp.hull.glacis_slope_deg, 60.0, "the T-34 bet everything on the 60-degree slope");
    assert_eq!(bp.hull.pike_sweep_deg, 0.0, "a flat bow, not a pike");
    assert!(
        bp.armor.hull_side.0 >= 35.0,
        "the upper sides are RAKED ({}°) — a vertical-sided T-34 is not a T-34",
        bp.armor.hull_side.0
    );
    // ("what you see is what you shoot" — the shape/armour slope agreement this used to
    // assert for the T-34 alone is now a FLEET lint in game_core, enforced at load time:
    // `lint_refuses_a_plate_that_looks_one_way_and_resolves_another`.)
}

/// Five big bare Christie wheels with the signature open gap behind the first station, no
/// return rollers, and the sag of a top run resting on the wheels.
#[test]
fn the_christie_gear_reads_five_wheels_a_gap_and_no_rollers() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 5, "five 830 mm discs per side");
    assert_eq!(bp.track.return_rollers, 0, "Christie gear carries its own top run");
    assert!(bp.track.wheel_radius >= 0.40, "the discs are BIG ({} m)", bp.track.wheel_radius);

    // The signature gap sits at the FRONT (+Z): between the first and second wheels counting
    // from the bow — the Christie spring wells live in the hull side there.
    let stations = bp.track.wheel_stations();
    let gaps: Vec<f32> = stations.windows(2).map(|w| w[1] - w[0]).collect();
    let front_gap = *gaps.last().expect("five wheels have four gaps");
    let rest_max = gaps[..gaps.len() - 1].iter().fold(0.0_f32, |a, &b| a.max(b));
    assert!(
        front_gap > rest_max + 0.05,
        "the open gap belongs at the FRONT pair: front {front_gap:.2} vs rest max {rest_max:.2}"
    );
}

/// The -85 refit's turret: a low wide cast dome, seated FORWARD on the low hull, overhanging
/// its widened ring.
#[test]
fn the_dome_is_low_wide_and_forward() {
    let bp = blueprint();
    assert_eq!(bp.turret.form, TurretForm::CastDome);
    assert!(bp.turret.ring_z >= 0.3, "the 85 turret sits forward, got ring_z {}", bp.turret.ring_z);
    assert!(bp.turret.ring_radius < bp.turret.base_radius, "the casting overhangs its ring race");
    // The proportions: a LOW hull (roof ~1.56) under a dome carrying the rest of the ~2.7 m
    // silhouette — the dome is a pot, not a bowl.
    assert!(bp.hull.deck_y <= 1.60, "the hull stays low, got deck_y {}", bp.hull.deck_y);
    let dome_height = bp.turret.roof_y - bp.turret.ring_y;
    assert!(
        (0.70..=1.00).contains(&dome_height),
        "the dome carries ~0.9 m of the silhouette, got {dome_height:.2}"
    );
}

/// The clean ZiS-S-53: no muzzle brake, no fume extractor — a plain tube to z = 5.05.
#[test]
fn the_zis_s53_is_a_clean_tube() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_none(), "no brake on the ZiS-S-53");
    assert!(bp.gun.evacuator.is_none(), "no fume extractor in 1944");
    assert!((bp.gun.muzzle_z - 5.05).abs() < 0.01, "8.10 m overall length gun forward");
}

/// The baked mesh honours the blueprint: hull inside the 3.00 m beam, the dome forward of the
/// hull centre, the whole tank under the documented height.
#[test]
fn the_baked_mesh_reads_the_blueprint() {
    let baked = bake_vehicle(VehicleKind::T34_85).expect("bakes");
    let hull = baked.submesh(SubmeshKind::Hull).unwrap().mesh.bounds().unwrap();
    let turret = baked.submesh(SubmeshKind::Turret).unwrap().mesh.bounds().unwrap();

    assert!(hull.max.x - hull.min.x <= 3.02, "hull spans the documented 3.00 m beam");
    let turret_centre_z = (turret.min.z + turret.max.z) * 0.5;
    assert!(turret_centre_z > 0.1, "the dome sits forward, got centre z {turret_centre_z:.2}");
    assert!(turret.max.y <= 2.75, "the silhouette stays under 2.75 m, got {}", turret.max.y);
}
