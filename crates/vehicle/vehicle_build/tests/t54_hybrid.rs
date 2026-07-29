//! Locking tests for the hybrid T-54 description (`vehicle_build::t54`). They live here as an
//! integration tree so the production module stays within the reviewability budget; every one drives
//! only the crate's public API (`t54_description`, `t54_from_modules`, `MEDIUM_LOD0_TRI_BUDGET`).
//! This file owns the structural contract — hull dimensions, armour facets, budget, gun/mantlet
//! submesh, silhouette, determinism and the LOD ladder; the exterior detailing (hatches, periscopes,
//! fenders, transmission covers, mantlet bedding) lives in the sibling `t54_hybrid_details.rs`.

use game_core::{MountFrames, VehicleKind};
use glam::Vec3;
use vehicle_build::{MEDIUM_LOD0_TRI_BUDGET, t54_description, t54_from_modules};
use vehicle_geometry::{MaterialRole, RunningGearKinematics, SmoothingGroup, SubmeshKind};

#[test]
fn the_blueprint_is_the_sole_source_of_hull_dimensions() {
    // The generated hull is a pure function of the blueprint's HullVisual — no parallel constant
    // lives in the generator. Its extents track the blueprint block, and perturbing the
    // blueprint copy moves the geometry with it.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let v = bp.hybrid().unwrap();
    let to_bounds = |hull: &game_core::HullVisual| {
        solid::t54_hull_solid(
            hull,
            bp.armor.hull_front.0,
            bp.armor.hull_side.0,
            bp.armor.hull_rear.0,
        )
        .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
        .expect("hull solid is valid")
        .bounds()
        .expect("non-empty hull")
    };

    let plate = to_bounds(&v.hull);
    assert!((plate.max.x - v.hull.half_width).abs() < 0.05, "hull width tracks the blueprint");
    assert!((plate.min.y - v.hull.belly_y).abs() < 0.05, "hull belly tracks the blueprint");
    assert!((plate.max.y - v.hull.roof_y).abs() < 0.05, "hull roof tracks the blueprint");

    let mut wide = v.hull;
    wide.half_width *= 2.0;
    let wide_plate = to_bounds(&wide);
    assert!(
        wide_plate.max.x > plate.max.x * 1.8,
        "doubling the blueprint half-width widens the generated hull (no parallel constant)"
    );
}

#[test]
fn glacis_geometry_slope_matches_the_armour_blueprint() {
    // The CAD glacis plate is built from the blueprint armour facet — the single source — so the
    // *built geometry* carries that angle in the armour convention: a glacis face normal sits
    // `slope` degrees above horizontal (atan2(n.y, n.z)) — what you see is what you shoot.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has a blueprint");
    let baked = t54_description().build();
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let on_slope = hull.vertices().iter().any(|v| {
        let n = v.normal;
        n.y > 0.2 && n.z > 0.2 && (n.y.atan2(n.z).to_degrees() - bp.armor.hull_front.0).abs() < 2.0
    });
    assert!(on_slope, "a glacis face normal carries the armour slope angle");
}

#[test]
fn every_hull_facet_carries_its_blueprint_armour_angle() {
    // Coherence extended past the glacis: the sloped sides and rear plates carry hull_side and
    // hull_rear in the same convention — what you see is what you shoot, on every facet.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap();
    let baked = t54_description().build();
    let ns: Vec<Vec3> = baked
        .submesh(SubmeshKind::Hull)
        .unwrap()
        .mesh
        .vertices()
        .iter()
        .map(|v| v.normal)
        .collect();
    let near = |found: f32, target: f32| (found - target).abs() < 2.0;
    let glacis =
        ns.iter().any(|n| n.z > 0.2 && near(n.y.atan2(n.z).to_degrees(), bp.armor.hull_front.0));
    let side =
        ns.iter().any(|n| n.x > 0.5 && near(n.y.atan2(n.x).to_degrees(), bp.armor.hull_side.0));
    let rear =
        ns.iter().any(|n| n.z < -0.5 && near(n.y.atan2(-n.z).to_degrees(), bp.armor.hull_rear.0));
    assert!(
        glacis && side && rear,
        "glacis={glacis} side={side} rear={rear} must each match blueprint"
    );
}

#[test]
fn the_description_builds_hull_and_turret_within_budget() {
    let baked = t54_description().build();
    assert!(baked.submesh(SubmeshKind::Hull).is_some(), "hull submesh present");
    assert!(baked.submesh(SubmeshKind::Turret).is_some(), "turret submesh present");
    let tris: usize = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();
    assert!(
        (250..MEDIUM_LOD0_TRI_BUDGET).contains(&tris),
        "hybrid T-54 LOD0 {tris} tris outside [250, {MEDIUM_LOD0_TRI_BUDGET})"
    );
}

#[test]
fn the_barrel_is_keyed_to_the_installed_gun_module() {
    // The muzzle z tracks the GunModule's barrel length — geometry from the module, not a scale.
    let gun_z = t54_description()
        .build()
        .submesh(SubmeshKind::Gun)
        .expect("gun submesh")
        .mesh
        .bounds()
        .expect("non-empty")
        .max
        .z;
    assert!(
        (gun_z - MountFrames::for_vehicle(VehicleKind::T54_1951).muzzle.translation.z).abs()
            < 1.0e-4,
        "muzzle {gun_z:.2} matches its authoritative mount"
    );
}

#[test]
fn gun_submesh_uses_the_authoritative_mount_frames() {
    let baked = t54_description().build();
    let bounds = baked.submesh(SubmeshKind::Gun).unwrap().mesh.bounds().unwrap();
    let mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
    assert!(bounds.min.y < mounts.gun_trunnion.translation.y);
    assert!(bounds.max.y > mounts.gun_trunnion.translation.y);
    assert!((bounds.max.z - mounts.muzzle.translation.z).abs() < 1.0e-4);
}

#[test]
fn moving_mantlet_is_part_of_the_gun_submesh() {
    let gun = t54_description().build().submesh(SubmeshKind::Gun).unwrap().mesh.clone();
    let cast: Vec<_> =
        gun.vertices().iter().filter(|v| v.material == MaterialRole::CastArmor).collect();
    assert!(!cast.is_empty(), "gun submesh needs a moving cast mantlet");
    let min_x = cast.iter().map(|v| v.position.x).fold(f32::INFINITY, f32::min);
    let max_x = cast.iter().map(|v| v.position.x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = cast.iter().map(|v| v.position.y).fold(f32::INFINITY, f32::min);
    let max_y = cast.iter().map(|v| v.position.y).fold(f32::NEG_INFINITY, f32::max);
    assert!(max_x - min_x > max_y - min_y, "mantlet is a wide oval");
}

#[test]
fn the_hybrid_t54_silhouette_reads_right() {
    // Locks the hybrid's visual reads against regression (cf. vehicle_geometry silhouette gates).
    let baked = t54_description().build();
    let turret = baked.submesh(SubmeshKind::Turret).unwrap().mesh.bounds().unwrap();
    assert!(
        (turret.max.x - turret.min.x) > (turret.max.y - turret.min.y),
        "turret reads as a flat cast dome (wider than tall)"
    );
    let hull = &baked.submesh(SubmeshKind::Hull).unwrap().mesh;
    let has = |m: MaterialRole| hull.vertices().iter().any(|v| v.material == m);
    assert!(
        !has(MaterialRole::Rubber),
        "road wheels and the complete track belt are runtime-animated gear, not static hull geometry"
    );
    assert!(
        RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).is_some(),
        "runtime running gear supplies the animated rubber wheels"
    );
    let b = hull.bounds().unwrap();
    assert!((b.max.z - b.min.z) > (b.max.x - b.min.x), "hull reads longer than wide");
}

#[test]
fn the_hybrid_bake_is_deterministic() {
    // Same description, same bytes — every generator (CAD, SDF, revolve) is deterministic.
    assert_eq!(
        t54_description().build().deterministic_hash(),
        t54_description().build().deterministic_hash()
    );
}

/// The SHIPPED T-54's cross-commit byte lock.
///
/// `GOLDEN_BAKE_HASHES` locks the procedural fleet — including a T-54 entry for the legacy
/// recipe **nobody ships**. The hybrid mesh the game actually draws had no recorded hash at
/// all: its only guard was ~90 property tests, so a silent geometry drift that happened to
/// satisfy every ratio and bound could ride in unnoticed.
///
/// Re-record deliberately, in the PR that intends the change, together with the studio tiles
/// (`WOT_UPDATE_GOLDENS=1 cargo test -p tools --test studio_goldens`) — a hash bless with no
/// picture to look at is a rubber stamp.
#[test]
fn the_shipped_hybrid_matches_its_recorded_golden() {
    // Recorded 2026-07-29 (PR-16, the documented cupola: ⌀624 not ⌀480, authored once
    // instead of written down three times).
    // Recorded 2026-07-29 (PR-15, the dome at its documented roof). The casting rises from a
    // 2.27 m roof to the documented 2.40, and its PLAN changes with it: Blender session S1 found
    // the widest cut 43% from the front, so the turret reaches 1.016 m forward of the ring and
    // 2.363 m long where the model had 2.10 — a T-54 with no bustle. (Where that length goes
    // stays front-heavy: turning S1's "43% from the front" into a ring-relative split needs the
    // widest cut to BE the ring plane, and S1 flags that registration as an assumption.) The cupola's 131 mm
    // of exposure is authored now instead of being whatever was left under the hitbox apex, and
    // the whole roof band (cupola, hatches, periscopes, DShK mount) rides the roof as a depth
    // into the casting rather than as absolute heights.
    //
    // Re-recorded again in PR-17, and this time the mantlet moves the other way: INSIDE. The
    // external ball is gone, the aperture is a 0.42 x 0.38 m plateau cut through the casting, a
    // canvas boot closes it to the tube, and `cast_loft` refines its azimuth grid over the
    // aperture instead of the whole dome carrying the resolution — so the casting is CHEAPER
    // (1,296 tris against 2,304 at the ring count that failed to sharpen it) and sharper.
    // Previous: 0x8f18_a4aa_f291_5175 (PR-16, the cupola at 624);
    //           0xf490_f9f1_fe07_3049 (PR-14, the hull at 6.235);
    //           0x903d_b868_b9bf_e617 (PR-25, a muzzle is a hole).
    const GOLDEN_HYBRID_LOD0_HASH: u64 = 0xef40_8214_00ae_a9e4;
    let baked = t54_description().build();
    assert_eq!(
        baked.deterministic_hash(),
        GOLDEN_HYBRID_LOD0_HASH,
        "the shipped T-54 hybrid bake drifted from its recorded golden (got 0x{:016x}) — if the \
         change is intended, re-record this constant in the same commit that makes it",
        baked.deterministic_hash()
    );
}

#[test]
fn the_hybrid_reduces_through_lod_tiers_within_tiered_budgets() {
    // Per-LOD budgets (the plan's tiered budgets): each tier first drops the parts its policy
    // excludes (build_lod) then decimates (reduce_vehicle), landing under its cap monotonically.
    use vehicle_geometry::{BakedVehicle, LodLevel, reduce_vehicle};
    const LOD1_BUDGET: usize = 4_000;
    const LOD2_BUDGET: usize = 1_200;
    let d = t54_description();
    let total =
        |v: &BakedVehicle| v.submeshes().iter().map(|s| s.mesh.triangle_count()).sum::<usize>();
    let lod0 = total(&d.build_lod(LodLevel::Lod0));
    let lod1 = total(&reduce_vehicle(&d.build_lod(LodLevel::Lod1), LodLevel::Lod1));
    let lod2 = total(&reduce_vehicle(&d.build_lod(LodLevel::Lod2), LodLevel::Lod2));
    assert!(lod0 > lod1 && lod1 > lod2, "tiers decimate monotonically: {lod0} {lod1} {lod2}");
    assert!(lod1 < LOD1_BUDGET, "LOD1 {lod1} within tier budget {LOD1_BUDGET}");
    assert!(lod2 < LOD2_BUDGET, "LOD2 {lod2} within tier budget {LOD2_BUDGET}");
}

#[test]
fn higher_lods_drop_detail_parts_but_keep_silhouette_and_mounts() {
    // Per-part LOD policy: LOD1 drops the detail fittings and track links (so it has fewer raw
    // triangles than LOD0 before any decimation), while the turret and gun mount parts survive.
    use vehicle_geometry::{BakedVehicle, LodLevel};
    let d = t54_description();
    let total =
        |v: &BakedVehicle| v.submeshes().iter().map(|s| s.mesh.triangle_count()).sum::<usize>();
    let lod0 = d.build_lod(LodLevel::Lod0);
    let lod1 = d.build_lod(LodLevel::Lod1);
    assert!(total(&lod1) < total(&lod0), "LOD1 drops detail parts before decimation");
    assert!(
        lod1.submesh(SubmeshKind::Turret).is_some() && lod1.submesh(SubmeshKind::Gun).is_some(),
        "mount-bearing turret and gun survive into LOD1"
    );
}

#[test]
fn both_d10_barrels_are_the_same_tube_and_the_mechanism_still_rebuilds_them() {
    // Honesty, not modularity theatre: the D-10T and D-10T2S are ONE physical tube (5350 mm
    // monobloc, L/53.5). They differ in the breech, the stabilizer and the fume extractor —
    // things this silhouette does not carry — so a T-54 must NOT grow a longer gun when the
    // upgrade is installed. This test used to assert the opposite (5.0 vs 5.9 m).
    let kind = VehicleKind::T54_1951;
    let muzzle_z = |gun: game_core::GunModule| {
        let mut loadout = kind.default_loadout();
        loadout.gun = gun;
        t54_from_modules(&loadout)
            .build()
            .submesh(SubmeshKind::Gun)
            .unwrap()
            .mesh
            .bounds()
            .unwrap()
            .max
            .z
    };
    let opts = kind.gun_options();
    assert!(opts.len() >= 2, "the T-54 fields both D-10 variants");
    let lengths: Vec<f32> = opts.iter().map(|gun| gun.barrel_length_m()).collect();
    for length in &lengths {
        assert!(
            (length - 5.35).abs() < 0.01,
            "every D-10 variant carries the documented 5.35 m tube, got {length}"
        );
    }
    // The barrel is still REBUILT from the installed module (not a post-bake scale): same
    // length in, same muzzle out, to the millimetre.
    let reach: Vec<f32> = opts.iter().cloned().map(muzzle_z).collect();
    for pair in reach.windows(2) {
        assert!(
            (pair[0] - pair[1]).abs() < 0.001,
            "equal tubes must reach the same muzzle: {:?}",
            reach
        );
    }
}
