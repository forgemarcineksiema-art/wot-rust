//! The T-54's anchor test: the dossier, the blueprint, the mount chain, the armour volumes and the
//! finished mesh, checked against each other in ONE place.
//!
//! Every other test in this programme measures one thing well. This one exists because a vehicle
//! can pass all of them and still be wrong at the joins — a blueprint that says 1.79 and a mount
//! chain that carries 1.78 are each internally consistent and disagree with each other. So this
//! walks the chain: what the dossier documents, what the blueprint authors, what the mounts hand
//! the gameplay code, what the armour resolves against, and what the mesh actually is.
//!
//! Modelled on `tiger_i_benchmark::the_dossier_clearance_and_fire_line_hold`, widened to the whole
//! reference set because the T-54 is the benchmark this workshop was built around.

use game_core::{MountFrames, VehicleBlueprint, VehicleKind};
use vehicle_build::t54_description;
use vehicle_geometry::SubmeshKind;

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("the T-54 has a blueprint")
}

/// Values from `docs/vehicles/t-54.md` — the Reference anatomy table, with its sources and its
/// confidence grades. Written out here so a reader can see the numbers this vehicle answers to
/// without leaving the test.
mod dossier {
    pub const HULL_LENGTH: f32 = 6.235;
    pub const WIDTH_OVER_FENDERS: f32 = 3.27;
    pub const TURRET_ROOF: f32 = 2.40;
    pub const GROUND_CLEARANCE: f32 = 0.425;
    pub const FIRE_LINE: f32 = 1.790;
    pub const TURRET_RING: f32 = 1.825;
    pub const ROAD_WHEEL_DIAMETER: f32 = 0.810;
    pub const ROAD_WHEELS_PER_SIDE: usize = 5;
    pub const TRACK_WIDTH: f32 = 0.580;
    pub const TRACK_GAUGE: f32 = 2.640;
    pub const TRACK_LINKS_PER_SIDE: usize = 90;
    pub const TRACK_PITCH: f32 = 0.137;
    pub const GROUND_CONTACT_BASE: f32 = 3.840;
    pub const CUPOLA_DIAMETER: f32 = 0.624;
    pub const CUPOLA_EXPOSED: f32 = 0.131;
}

/// Link 1: the BLUEPRINT authors what the dossier documents.
#[test]
fn the_blueprint_authors_the_dossier() {
    let bp = blueprint();
    let near = |what: &str, got: f32, want: f32, tol: f32| {
        assert!((got - want).abs() <= tol, "{what}: blueprint {got} vs dossier {want} (tol {tol})");
    };

    near("hull length", bp.hull.half_len * 2.0, dossier::HULL_LENGTH, 1.0e-3);
    near("ground clearance", bp.hull.belly_y, dossier::GROUND_CLEARANCE, 1.0e-6);
    near("fire line", bp.gun.trunnion_y, dossier::FIRE_LINE, 0.011);
    // The ring is quoted "in the clear" — the opening a crew climbs through — so the blueprint's
    // radius is the ring's own and 5 mm of machining is inside the tolerance the gate carries.
    near("turret ring", bp.turret.ring_radius * 2.0, dossier::TURRET_RING, 0.015);
    near("turret roof", bp.turret.roof_y, dossier::TURRET_ROOF, 1.0e-6);
    near("cupola diameter", bp.turret.cupola_radius * 2.0, dossier::CUPOLA_DIAMETER, 1.0e-3);
    near(
        "cupola exposure",
        bp.turret.cupola_height.expect("the cupola's exposure is authored"),
        dossier::CUPOLA_EXPOSED,
        1.0e-6,
    );
    near("road wheel", bp.track.wheel_radius * 2.0, dossier::ROAD_WHEEL_DIAMETER, 1.0e-6);
    near("track width", bp.track.outer_x - bp.track.inner_x, dossier::TRACK_WIDTH, 1.0e-3);
    near("track gauge", bp.track.center_x * 2.0, dossier::TRACK_GAUGE, 1.0e-3);
    assert_eq!(bp.track.wheel_count as usize, dossier::ROAD_WHEELS_PER_SIDE);
    assert_eq!(bp.track.link_count.expect("authored link count"), dossier::TRACK_LINKS_PER_SIDE);

    let stations = bp.track.wheel_stations.expect("the T-54 authors its wheel stations");
    near(
        "ground-contact base",
        stations[stations.len() - 1] - stations[0],
        dossier::GROUND_CONTACT_BASE,
        1.0e-3,
    );
}

/// Link 2: the MOUNT CHAIN carries the same numbers the blueprint authors. A gun that fires from
/// a different height than the one the vehicle is built at is two vehicles.
#[test]
fn the_mount_chain_carries_the_blueprints_numbers() {
    let bp = blueprint();
    let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
    assert!(
        (mounts.gun_trunnion.translation.y - bp.gun.trunnion_y).abs() < 1.0e-6,
        "the trunnion the gameplay code aims from IS the blueprint's fire line"
    );
    assert!(
        (mounts.gun_trunnion.translation.z - bp.gun.trunnion_z).abs() < 1.0e-6,
        "and its station along the hull"
    );
    assert!(
        (mounts.muzzle.translation.z - bp.gun.muzzle_z).abs() < 1.0e-6,
        "the muzzle frame is where the blueprint puts the muzzle"
    );
    assert!(
        (mounts.turret_ring.translation.y - bp.turret.ring_y).abs() < 1.0e-6,
        "and the ring the turret turns on is the ring the blueprint seats it on"
    );
}

/// Link 3: the MESH is the vehicle all of the above describes. Measured, not asserted from the
/// numbers that built it — that is the difference between a check and a tautology.
#[test]
fn the_finished_mesh_is_the_vehicle_the_dossier_describes() {
    let baked = t54_description().build();
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull").mesh.bounds().expect("bounds");
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret").mesh.bounds().expect("bounds");

    assert!(
        (hull.max.x - hull.min.x - dossier::WIDTH_OVER_FENDERS).abs() <= 0.05,
        "over the fenders: {:.3} against a documented {:.3}",
        hull.max.x - hull.min.x,
        dossier::WIDTH_OVER_FENDERS
    );
    assert!(
        turret.max.y >= dossier::TURRET_ROOF && turret.max.y <= dossier::TURRET_ROOF + 0.20,
        "the casting tops out at its documented roof plus its cupola and lid: {:.3}",
        turret.max.y
    );

    // The belt is the chain the dossier specifies, and the pitch falls out of it.
    let kin = vehicle_geometry::RunningGearKinematics::for_vehicle(VehicleKind::T54_1951)
        .expect("running gear");
    let pitch = kin.belt_length() / kin.link_count() as f32;
    assert!(
        (pitch - dossier::TRACK_PITCH).abs() <= 0.001,
        "90 links of a documented 137 mm: got {pitch:.4}"
    );
}

/// The other half of the finish line: the FORM rules the dossier states, which no dimension can
/// express.
///
/// `dimension_gate` proves every numeric anchor is Locked. These are the ones a tape measure
/// cannot see — the fittings this vehicle has, and the ones it does not.
#[test]
fn the_form_rules_the_dossier_states_all_hold() {
    let bp = blueprint();
    let description = t54_description();
    let has = |name: &str| description.parts.iter().any(|part| part.key.name == name);

    // "Plain tapering D-10T barrel — any muzzle furniture marks a later variant."
    assert!(bp.gun.evacuator.is_none(), "obr. 1951 has no bore evacuator");
    assert!(bp.gun.muzzle_brake.is_none(), "and no muzzle brake");
    // "No return rollers — the top run rests on the wheels."
    assert_eq!(bp.track.return_rollers, 0, "no return rollers");
    // "No external gun travel lock."
    assert!(
        !description.parts.iter().any(|part| part.key.name.contains("travel_lock")),
        "no travel lock"
    );
    // The fittings the dossier says ARE there — each one landed in a named PR of this programme.
    for fitting in [
        "course_mg_port",
        "smoke_canister",
        "headlight",
        "headlight_lens",
        "unditching_beam",
        "unditching_beam_bands",
        "dshk_ring",
        "gun_mantlet_cover",
        "turret_casting_seam",
        "engine_deck_bolts",
    ] {
        assert!(has(fitting), "the dossier's {fitting} is on the vehicle");
    }
}
