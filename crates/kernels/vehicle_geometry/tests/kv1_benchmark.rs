//! The KV-1 mod. 1942's shape cage: each test names a defect that would un-KV the tank. The
//! shape lives in `blueprints/kv1_1942.blueprint.ron` and these are the bars around that file.
//! Every number quoted here comes from `docs/vehicles/kv-1.md`.

use game_core::{ShoePattern, SuspensionKind, TurretForm, VehicleBlueprint, VehicleKind};
use vehicle_geometry::{RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::KV1_1942).expect("KV-1 has a blueprint")
}

/// Form rule 5, and the most important bar in this cage: the turret is a long slab-sided LOAF.
/// The shared `cast_turret_shell` hard-codes a station table that tapers to 0.66 of the shoulder
/// beam at the roof — a dome. If anyone ever routes the KV through it to save fifty lines, the
/// roof-band assert below goes red, which is exactly its job.
#[test]
fn the_turret_is_a_long_slab_not_a_dome() {
    let bp = blueprint();
    let t = &bp.turret;
    assert_eq!(t.form, TurretForm::CastSlab, "a casting with FLAT walls, baked as a prism");
    assert!(
        t.plan_half_length > t.plan_half_width * 1.25,
        "the plan is a loaf: {:.2} long vs {:.2} wide",
        t.plan_half_length,
        t.plan_half_width
    );
    assert!(t.side_slope_deg <= 8.0, "near-VERTICAL walls, got {}°", t.side_slope_deg);

    // The mesh proof: measure the casting's half-width at the shoulder and just under the roof.
    let baked = bake_vehicle(VehicleKind::KV1_1942).expect("bakes");
    let turret = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let band_half_width = |lo: f32, hi: f32| {
        turret
            .vertices()
            .iter()
            .filter(|v| v.position.y >= lo && v.position.y <= hi)
            .fold(0.0_f32, |acc, v| acc.max(v.position.x.abs()))
    };
    // A loft only emits vertices at its section heights, so sample the rings themselves: the
    // casting's beam where it meets the ring seat, against its beam at the roof.
    let shoulder = band_half_width(t.ring_y - 0.01, t.ring_y + 0.01);
    let crown = band_half_width(t.roof_y - 0.01, t.roof_y + 0.01);
    assert!(
        crown >= 0.85 * shoulder,
        "the crown must keep the beam ({crown:.2} of {shoulder:.2}) — this is a DOME, not a slab"
    );
}

/// Form rule 6: the rear DT ball in its armoured collar, the one fitting no other vehicle in the
/// fleet wears. It must stand PROUD of the rear wall at its own height — a ball in a wall, not a
/// decal on it.
#[test]
fn the_cast_turret_carries_its_rear_dt_ball() {
    let bp = blueprint();
    let t = &bp.turret;
    let baked = bake_vehicle(VehicleKind::KV1_1942).expect("bakes");
    let turret = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;

    // Where the casting's rear wall sits at the ball's height, from the blueprint's own slope.
    let rise = t.roof_y - t.ring_y;
    let ball_y = t.ring_y + rise * 0.45;
    let wall_z =
        (t.ring_z - t.plan_half_length) + rise * 0.45 * t.rear_slope_deg.to_radians().tan();

    let rearmost = turret
        .vertices()
        .iter()
        .filter(|v| (v.position.y - ball_y).abs() <= 0.18 && v.position.x.abs() <= 0.20)
        .fold(f32::INFINITY, |acc, v| acc.min(v.position.z));
    assert!(
        rearmost < wall_z - 0.03,
        "the DT ball should stand proud of the rear wall: {rearmost:.2} vs wall {wall_z:.2}"
    );
}

/// Form rule 7: flush roof hatches and a periscope, NOT a drum cupola. A cupola on this roof
/// would make it a KV-1S — a different vehicle, and a silhouette clone of the T-34-85's crown.
#[test]
fn no_cupola_crowns_the_roof() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::KV1_1942).expect("bakes");
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh.bounds().unwrap();
    let proud = turret.max.y - bp.turret.roof_y;
    assert!(
        proud < 0.15,
        "only hatch rims and a periscope stand on the roof, got {proud:.2} m of superstructure"
    );
}

/// Form rules 9 and 10: a clean short tube. The two margins that hold the KV's minimal overhang
/// legal are asserted here BY NAME, so a later nudge to the hull or the trunnion says which
/// invariant it broke instead of failing somewhere in the fleet gates.
#[test]
fn the_zis5_is_a_clean_short_tube_that_barely_clears_the_bow() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_none(), "no brake on the ZiS-5");
    assert!(bp.gun.evacuator.is_none(), "no fume extractor in 1942");

    let overhang = bp.gun.muzzle_z - bp.hull.half_len;
    assert!(overhang <= 0.60, "the KV's gun barely clears the bow, got {overhang:.2} m");

    // Invariant 1 (`all_vehicles`): the barrel must protrude past the collision box.
    assert!(
        bp.gun.muzzle_z > bp.hull.hitbox_half_length,
        "muzzle {:.2} must clear hitbox {:.2}",
        bp.gun.muzzle_z,
        bp.hull.hitbox_half_length
    );
    // Invariant 2 (`mount.rs`): 2.5 m of tube ahead of the trunnion.
    assert!(
        bp.gun.muzzle_z > bp.gun.trunnion_z + 2.5,
        "muzzle {:.2} must sit 2.5 m ahead of trunnion {:.2}",
        bp.gun.muzzle_z,
        bp.gun.trunnion_z
    );
}

/// Form rules 2, 3 and 4: six small wheels at an EVEN pitch on torsion bars, a top run carried
/// taut on three return rollers, and the toothed wheel at the tail.
#[test]
fn six_evenly_spaced_wheels_under_three_return_rollers() {
    let bp = blueprint();
    let track = &bp.track;
    assert_eq!(track.wheel_count, 6, "six 600 mm discs per side");
    assert_eq!(track.return_rollers, 3, "three rollers carry the top run");
    assert!(track.roller_radius > 0.0, "a roller needs a radius");
    assert_eq!(track.suspension, SuspensionKind::TorsionBar);
    assert!(!track.drive_front, "Soviet: the drive sprocket is at the TAIL");
    assert!(
        track.wheel_radius <= 0.32,
        "small wheels ({}) — not Christie discs",
        track.wheel_radius
    );
    assert!(track.top_sag_m <= 0.02, "a rollered run stays taut, got {}", track.top_sag_m);

    // Even pitch: no T-34 Christie gap, no T-54 first/second stagger.
    let stations = track.wheel_stations();
    let gaps: Vec<f32> = stations.windows(2).map(|w| w[1] - w[0]).collect();
    let (min, max) =
        gaps.iter().fold((f32::INFINITY, 0.0_f32), |(lo, hi), g| (lo.min(*g), hi.max(*g)));
    assert!(max - min <= 0.02, "the KV's pitch is EVEN: gaps span {min:.2}..{max:.2}");

    // And the placements actually put twelve wheels and six rollers on the vehicle.
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::KV1_1942).expect("KV-1 has gear");
    let placements = running_gear_placements(&kin, 0.0, 0.0);
    let count = |part| placements.iter().filter(|p| p.part == part).count();
    assert_eq!(count(vehicle_geometry::GearPart::RoadWheel), 12, "six wheels a side");
    assert_eq!(count(vehicle_geometry::GearPart::ReturnRoller), 6, "three rollers a side");
}

/// Form rule 1: mass without slope. The sides stand DEAD vertical and the armour facet carries
/// the visible angle — a sloped-sided KV is a T-34 in fancy dress.
#[test]
fn the_hull_is_thick_plate_not_slope() {
    let bp = blueprint();
    assert_eq!(bp.armor.hull_side.0, 0.0, "the KV's sides are VERTICAL");
    assert_eq!(bp.hull.pike_sweep_deg, 0.0, "a stepped bow, not a pike");
    assert!(
        (bp.armor.hull_front.0 - bp.hull.glacis_slope_deg).abs() < 1.0e-6,
        "what you see is what you shoot: the armour facet carries the visible bow angle"
    );
    // The turret barely leans either — this casting stops shells, it does not bounce them.
    assert!(bp.armor.turret_front.0 <= 20.0, "a shallow face, got {}°", bp.armor.turret_front.0);
}

/// The KV wears its OWN track, not the T-54 family's. Sharing a shoe with a vehicle that never
/// shared one is the clone-factory defect the per-family patterns exist to prevent.
#[test]
fn the_kv_wears_its_own_cast_shoe() {
    assert_eq!(blueprint().track.shoe_pattern, ShoePattern::KvCast);
}

/// Form rule 8: full-length squared fenders that stay inside the track band, so nothing visible
/// hangs over air the shell trace cannot hit.
#[test]
fn the_full_length_fenders_stay_inside_the_track_band() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::KV1_1942).expect("bakes");
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh.bounds().unwrap();
    assert!(
        hull.max.x <= bp.track.outer_x + 1.0e-3,
        "the hull and its fenders stay inside the {:.2} m track face, got {:.2}",
        bp.track.outer_x,
        hull.max.x
    );
    // ...and they run the length of the hull rather than stopping short.
    assert!(hull.max.z >= bp.hull.half_len - 1.0e-3, "the guards run full length");
}

/// The researched body: 6.75 x 3.32 m and 2.71 m overall, all of it inside the collision box.
#[test]
fn the_hitbox_is_the_researched_body() {
    let bp = blueprint();
    assert!((bp.hull.half_len * 2.0 - 6.75).abs() < 0.02, "6.75 m hull");
    assert!((bp.track.outer_x * 2.0 - 3.32).abs() < 0.02, "3.32 m over the tracks");
    assert!((bp.hull.belly_y - 0.43).abs() < 0.02, "0.43 m of clearance");

    let baked = bake_vehicle(VehicleKind::KV1_1942).expect("bakes");
    let body = baked.body_bounds().expect("body bounds");
    assert!((body.max.y - 2.71).abs() < 0.10, "2.71 m overall height, got {:.2}", body.max.y);
    let top = bp.hull.hitbox_center_y + bp.hull.hitbox_half_height;
    assert!(body.max.y <= top, "the silhouette never overhangs its collision volume");
}
