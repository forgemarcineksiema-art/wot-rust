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
    let v = bp.complete_visual().unwrap();
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

    let plate = to_bounds(v.hull);
    assert!((plate.max.x - v.hull.half_width).abs() < 0.05, "hull width tracks the blueprint");
    assert!((plate.min.y - v.hull.belly_y).abs() < 0.05, "hull belly tracks the blueprint");
    assert!((plate.max.y - v.hull.roof_y).abs() < 0.05, "hull roof tracks the blueprint");

    let mut wide = *v.hull;
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
    //
    // And again in PR-18, for the documented running gear: a 580 mm belt on the 2.640 m gauge,
    // 90 links at the documented 137 mm pitch, a wrap on the sprocket's own 572.4 mm pitch
    // circle (which drops the tooth count to the documented 13 without anyone typing 13), the
    // 3.840 m ground-contact base, and a tub narrowed to the 2.060 m that gauge leaves it.
    //
    // And in PR-19, for the fittings: the asymmetric fender line (two flat fuel tanks on the
    // right, not three), the fixed course MG's boss in the glacis, two smoke canisters lying
    // across the rear plate — and one part DELETED, the travel lock obr. 1951 never had.
    //
    // And in PR-26a, for the exterior mechanics: the headlight turned to face FORWARD with a
    // glass lens, bracket and guard; raked deck louvres; the bolts holding the deck panels down;
    // and the turret's mould line. Three of those come from generators that had existed in the
    // detail kernel since it was written and had never been called once.
    //
    // And in PR-26b, for the mechanics a crew touches: coamings, hinges and handles on all three
    // hatches (the fleet had no hinges at all), the DShK moved onto the loader's hatch RING and
    // given its own 12.7 mm calibre instead of the tank gun's, hooks with a throat and a catch,
    // thimbled and clamped cables, and bands round the log.
    //
    // Re-recorded 2026-07-29 for the gun WINDOW (the player's verdict on the first internal
    // mount stood: "the mantlet stayed the same, shrunk and swallowed"). The face is two-step
    // now — a wide shallow rectangular window (~0.85 x 0.44 m rebate) with the narrow deep
    // aperture dropping through its shelf — and what closes it is a rectangular canvas PANEL
    // with a flat mattress front (not a round boot in a round pocket), clamped by a perimeter
    // steel strip, pierced by a round sleeve whose inradius clears the tube. Judged against
    // Blender renders across six iterations before this bless; fold ridges were tried twice,
    // read as stamps/slashes both times, and are deliberately absent. The occluded internal
    // mantlet pays for the window: 12 segments, since only a breach remesh ever shows it.
    //
    // Re-recorded 2026-07-29 for PILOT MODULE 1 — the turret shell rebuilt from PHOTOGRAPHS
    // (the player's verdict: "wieza nie jest nalesnikiem"). Stations extracted from walkaround
    // REF 46 (ring->roof calibration, depth-plane cross-check), plan exponent 2.35 (egg, not
    // squircle), one central face-plate plateau the window is cut through, skirt overhang at
    // the widest band, no rearward crown drift. The S1 drawing's vertical flank band — the
    // sheet with its own 4-7% disagreement — is overturned; the instruments were rebuilt with
    // the geometry (tight face sampler, local wall steps). Player accepted the in-game render
    // ("jest git") before this bless.
    //
    // Re-recorded 2026-07-29 for PILOT MODULE 2 — the window MEASURED: ~0.50 m wide (dead-front
    // reference, turret width 2.25 in frame; barrel-diameter cross-check), not the first
    // build's eyeballed 0.85 m letterbox. Panel 0.45 x 0.37 pillow, window bump 0.172 rad,
    // aperture unchanged at the documented 0.40. Every proportion lock re-anchored to the
    // measured numbers in the same commit.
    // Re-recorded 2026-08-02 for the ENGINE-BAY FUEL CELLS. The fleet containment rule measured
    // both of them reaching 0.46 in hull-local y where the hull interior ends at 0.39: 7 cm of
    // fuel standing above the engine deck, in air. Their roofs now stop on the deck plane and
    // their floors have not moved. This is the first bless in the file that no eye asked for —
    // the previous two of exactly this fault (the engine block's roof, the bow rack) were both
    // caught by looking at renders, once each, on this one vehicle.
    // Re-recorded 2026-08-02 for the FENDERS AS PRESSINGS. Four cuboids a side plus one more
    // cuboid glued under the outer edge become four `panel` sections a side, each with its own
    // folded return lip (`PanelEdge::Hem`). The blueprint numbers do not move — same footprint,
    // same 40 mm plate, same 50 mm lip drop, so the invariant keeping the lip clear of the track's
    // top run holds unchanged. What changes is that the shelf's deliberate 3 cm seams now run all
    // the way to the fold, where a single strip used to carry straight through them.
    // Re-recorded 2026-08-02 for the COVER PROUD OF THE REBUILT FACE. Pilot module 2 moved the
    // casting's nose out to z 1.2424 and left the canvas panel's stations at their pre-pilot
    // values, so the face swallowed the border station and the front read flat — the
    // swallowed-boot verdict again. `the_visible_gun_mount_is_no_wider_than_its_canvas_cover`
    // measured it (fabric 0.215 against the 0.22 floor) and was RED ON MASTER from that merge
    // until this bless: the stack shipped without the full suite running. The panel's edge
    // stays in the frame's clamp; the border and face stations move ahead of the measured
    // nose (border 1.255, face 1.300), pressed out past the rim as the photographs show.
    // Previous: 0x2768_519d_821e_7b95 (the fenders as pressings);
    //           0x26e2_4b85_1151_a3ad (the engine-bay fuel cells);
    //           0x0a07_5ae1_b4fd_f995 (pilot module 2, the measured window);
    //           0x3eb2_aca8_62f7_7ceb (pilot module 1, the photo-derived egg);
    //           0xa921_f937_ea70_c698 (the gun window rebuild);
    //           0xadf8_2d0f_e2d8_f8be (PR-26b, the crew's mechanics);
    //           0x6af9_ab6d_876a_8099 (PR-26a, the exterior mechanics);
    //           0xe9ce_9c05_6f0b_f9df (PR-19, the fittings);
    //           0x7108_fd58_f651_b2c3 (PR-18, the belt at 90 x 137);
    //           0xef40_8214_00ae_a9e4 (PR-17, the mantlet goes inside);
    //           0x8f18_a4aa_f291_5175 (PR-16, the cupola at 624);
    //           0xf490_f9f1_fe07_3049 (PR-14, the hull at 6.235);
    //           0x903d_b868_b9bf_e617 (PR-25, a muzzle is a hole).
    // Re-recorded 2026-08-08 (the construction floor's first burn): both periscopes stop being a
    // box with one corner cut. Each is now the device — the raked housing, a GLASS prism lying in
    // its rake, and two armoured cheeks that are the reason a crew still has a prism after the
    // first burst. Four parts where there was one, twice on the turret roof and twice on the hull.
    // Previous: 0xea41_7d9a_dd9d_3b4b (PR-26, the roundness law applied).
    // Re-recorded 2026-08-08 (Wave 1's construction pass): the exhaust stops being a toolbox and
    // gets louvres across its outboard face plus a dark outlet at its stern — the same read the
    // eight recipe vehicles already had and the benchmark did not; every stowage bin and fuel tank
    // gains the lid seam, hinge and latch that make a box a bin; and the unditching beam becomes a
    // hewn timber instead of a twelve-sided lathe turning, which is a pipe.
    // Previous: 0x77c8_bbaf_eb61_92e6 (PR-27, the periscopes as devices).
    // Re-recorded 2026-08-08 (Wave 1 closing): every hatch lid stops being a flat puck — it domes
    // toward its crown and steps at the rim where it seats on its coaming, all INSIDE the height
    // it already had, because `hitbox_fit` correctly refused a lid that raised the vehicle. And
    // the roundness law reaches `vehicle_build` at last: thirteen hand-written segment counts —
    // the cupola, the DShK's ring, post, pin and sight, the beam bands, the MG port, the smoke
    // canisters and the tow-cable hardware — now come from their own radius.
    // Previous: 0xb8d6_7fbe_9b1d_8a07 (PR-28, the construction pass).
    // Re-recorded 2026-08-08 (the bevel law): the engine deck's four frame rails stop being
    // perfectly sharp and take the arris a torch leaves. `solid::chamfer` now states the widths a
    // process gives an edge; `chamfered_box` applies them, and always could.
    // Previous: 0xc308_5dcf_814b_3e57 (PR-29, the roundness law in vehicle_build).
    // Re-recorded 2026-08-10 (the gun group rides the trunnion): the D-10T stops floating above
    // the barrel it belongs to. The breech was anchored 230 mm over the bore, the cradle 420 mm,
    // the coax 340 mm and the TSh-2 330 mm — and all of it came out through the casting roof,
    // where the cradle rails stood ~250 mm proud in the garage hero shot. The recoil guard was
    // the one piece anchored to the gun, which is how its own frame gave the error away. Every
    // piece now reads ONE height from the authoritative breech volume, which the layout anchors
    // to the blueprint's trunnion. Also home: the turret ready rack (its top round's case rim
    // stood 71 mm proud of the rear dome — brass on the casting with no breach needed) and the
    // radio set, whose control panel reached 94 mm outside the dome on the diagonal.
    // Previous: 0x0950_1aaa_02c5_23fa (PR-30, the bevel law).
    // Re-recorded 2026-08-10 (the mechanisms do what they are named for): the projectile stops
    // being brass — the case and its rim are cartridge brass, the shell in front of them is
    // painted steel, and the two had it exactly backwards, which is why 32 gold rounds sat in the
    // fighting compartment. The tow hook's C is swept the other way, so its mouth faces the bow
    // instead of the plate it is welded to, with the catch across the gap rather than stranded
    // inside the closed half. The engine-deck bolts move from 0.86 of the half-width to 0.98:
    // fourteen of the eighteen were sealed inside the transmission covers, 1008 triangles
    // rendering the inside of a plate. And the exhaust louvres lie ON their cowl now that
    // `louvre_slats` derives its across-axis from world up rather than from Z — on a vertical
    // face the old convention turned width into height and grew five 675 mm fins out of a 220 mm
    // box, through the fender and into the moving top run of the track.
    // Previous: 0xcaf6_206c_f364_2e35 (PR-31, the gun group on the trunnion).
    // Re-recorded 2026-08-11 (the hardware meets the metal): hatch collars and the loader's hinge
    // seat on the LOCAL surface — the cupola drum's top, the dome's roof plate, the hull roof —
    // instead of hanging off the lid's buried base, which had 412 triangles of coamings and hinge
    // rendering the inside of a casting. And the cupola stops being a smooth drum: five vision
    // blocks under its top rim, each an armoured hood rooted into the wall with a glass pane in
    // its face — the ring of devices the drum exists to carry.
    // Previous: 0x8c8d_d462_f899_9cfb (PR-32, the sprocket's rings and the roof furniture).
    // Re-recorded 2026-08-11 (the slits go INTO the drum): the dimension gate threw the proud
    // vision hoods back — the cupola's Locked ⌀624 tape caught them at ⌀675 — and it was right
    // on the history too: the obr. 1951 cupola carries slits with glass behind them, not pods.
    // Frames flush with the drum's nominal radius (width bounded by chord geometry against the
    // tape), glass recessed 6 mm so the slit reads as a dark opening. The left swing arm also
    // becomes its own mirrored unit mesh (GearPart::SwingArmLeft) — instanced gear, so the
    // static bake moves only through the slit rework.
    // Previous: 0x8ebd_c33e_7f84_bc86 (PR-33, the hatch hardware and the first cupola blocks).
    // Re-recorded 2026-08-11 (the mould line finds the metal, and so does the armour): the seam
    // moves from y 1.95 off the bare curve family to y 1.71 ON the modulated surface — a mould
    // parts at its widest band, and the line now rides the cheeks and dips through the window
    // like the reference casting's. Drawing it there exposed the real find: the armour's bump
    // mirror double-counted the cheek plateau (two gaussians at +-0 vs the mesh's single
    // plateau), leaving ~45 mm of phantom armour across the whole face, hidden at 0.049 under a
    // 0.05 tolerance. Mirror corrected (slack now 1.5 mm), tolerance tightened to 0.010, and
    // the seam path plus rails now read the blueprint's OWN ring/surface family — the private
    // duplicate in kit_lines is gone.
    // Previous: 0x82a8_3c8e_17e9_3af2 (PR-34, the flush cupola slits and the mirrored arm).
    const GOLDEN_HYBRID_LOD0_HASH: u64 = 0xf74e_0e22_b2ab_8199;
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
