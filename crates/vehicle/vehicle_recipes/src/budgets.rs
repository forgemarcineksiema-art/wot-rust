//! Fleet cost and deterministic-output contracts shared by tests and Forge Studio.

use game_core::VehicleKind;

/// Triangle budgets (per submesh and per vehicle). Upper bounds keep runtime cost stable as the
/// tank count grows; lower bounds guard against silhouettes regressing back into plain boxes.
#[derive(Debug, Clone, Copy)]
pub struct VehicleBudgets {
    pub hull_tri: (usize, usize),
    pub turret_tri: (usize, usize),
    pub gun_tri: (usize, usize),
    pub vehicle_tri: (usize, usize),
    pub vehicle_vert_max: usize,
}

/// The lineup-wide procedural bake envelope. Runtime running gear is excluded: wheels,
/// suspension, end wheels, and links are animated instances. The continuous backing skin belongs
/// to each moving link rather than the static hull, so a thrown track leaves no frozen ghost belt.
pub const VEHICLE_BUDGETS: VehicleBudgets = VehicleBudgets {
    hull_tri: (120, 2750),
    turret_tri: (24, 900),
    // Raised 500 -> 650 (measured 2026-08-03, W4 F5.ii): the authored gun group - the
    // bore-honest barrel (28 segments x 11 profile stations, the muzzle reads as a HOLE)
    // plus the mantlet body - measures 612 on the T-54. The old cap was sized to the legacy
    // flat-dimple barrel this group replaces; 650 gives the measured shape 6% headroom and
    // still bounds a 14-tank lineup to under 10k gun triangles.
    gun_tri: (24, 650),
    vehicle_tri: (250, 3950),
    vehicle_vert_max: 11_000,
};

/// What the instanced RUNNING GEAR is allowed to cost, per vehicle, per detail tier.
///
/// `VEHICLE_BUDGETS` above bounds the static bake and says plainly that the gear is excluded —
/// correct for what it guards, and it meant the largest single body of geometry on a vehicle had
/// no ceiling at all. A T-54 draws 38.6k gear triangles across 204 instances: more than twice its
/// whole static bake, and until now nothing could tell you if that number grew.
///
/// Two ceilings, because a distance tier that saves nothing is a rename. `far_tri_max` is not
/// just "smaller"; the ratio between the two is what makes the tier worth its complexity, and
/// `FAR_MUST_SAVE_FRACTION` states the minimum it has to earn.
#[derive(Debug, Clone, Copy)]
pub struct GearBudgets {
    /// Triangles the gear may draw at [`crate::GearDetail::Near`], per vehicle.
    pub near_tri_max: usize,
    /// Triangles at [`crate::GearDetail::Far`], per vehicle.
    pub far_tri_max: usize,
    /// Draw instances per vehicle, either tier (the tier changes meshes, never placements).
    pub instances_max: usize,
}

/// Re-measured 2026-08-08 (`cargo test -p vehicle_forge --test fleet_draw_cost -- --nocapture`).
/// Every row had grown since it was written and nothing said so, because the ceiling still held:
///
/// | vehicle | near | far | saved | was (2026-07-29) |
/// |---|---:|---:|---:|---|
/// | T-54 | 39,632 | 15,776 | 60% | 38,568 / 15,092 |
/// | IS-3 | 39,632 | 16,528 | 58% | 38,448 / 15,448 |
/// | Tiger I | 38,736 | 16,592 | 57% | — |
/// | Jagdtiger | 28,936 | 14,076 | 51% | 26,392 / 13,788 |
/// | Tiger II | 28,712 | 14,012 | 51% | — |
/// | Centurion | 24,816 | 11,464 | 54% | — |
/// | Panther II | 24,120 | 11,492 | 52% | — |
/// | T-34-85 | 23,088 | 10,368 | 55% | 19,800 / 9,420 |
///
/// **A withdrawn claim.** This block used to say PR-22 would remove "11,520 triangles per tank
/// currently buried inside the link's backing slab and rendering nothing". Checked
/// (`running_gear_geom.rs::track_link_unit_mesh`): the slab is one 12-triangle `box_prism` per
/// link — about 2,160 triangles per tank, not 11,520 — and it has a documented job, closing the
/// triangular gaps rectangular shoe faces expose around an idler wrap. **There is no free
/// reserve there.** Anything that raises the link count has to be paid for out of the frame.
///
/// What the frame can afford is now measured rather than assumed
/// (`docs/battle-first/measurements.md`, 2026-08-08, MX330 at the shipped 1x MSAA): a 7v7 costs
/// 12.19 ms p50 / 13.68 ms p95 of GPU work against a 16.67 ms line, and forcing every tank to
/// NEAR gear costs +1.55 ms p50 and leaves **1.46 ms of p95 headroom**. That is the budget the
/// five vehicles with 1.65–2.05x oversized shoes have to fit their fix into.
pub const GEAR_BUDGETS: GearBudgets =
    GearBudgets { near_tri_max: 40_000, far_tri_max: 17_000, instances_max: 260 };

/// The distance tier must remove at least this share of the gear's triangles, or it is not
/// earning the second mesh set it costs to keep.
pub const FAR_MUST_SAVE_FRACTION: f32 = 0.40;

/// Golden procedural bake hashes. Re-record only after an intentional geometry change and visual
/// review. The 2026-07-18 fleet re-record removes the fused track band from every blueprint hull;
/// its overlap skin now rides in the animated link mesh and therefore does not affect these hashes.
///
/// The 2026-07-26 re-record fixes the revolve caps on a profile that runs BACKWARDS along its
/// axis (`builder/revolve.rs`). Every gun's bore funnel recedes from the muzzle face into the
/// tube, so both its end caps were wound INWARD — back-facing, and therefore dropped by the
/// vehicle pipeline's `cull_mode: Back` — and their welded rims left 24–28 inconsistently wound
/// edges per gun. Vertex POSITIONS are untouched: only the cap index order and the welded rim
/// normals move, so silhouette, ratios and budgets are unchanged.
///
/// The 2026-07-26 IS-3-only re-record walks the sponson-underside boundary through the tub step
/// corner instead of fanning across it (`recipes/is3_hull.rs`), clearing the fleet's last 2
/// inconsistently wound edges. One triangle's corner moves along a line it was already on, so the
/// silhouette is identical; no other vehicle's hash moves.
///
/// The 2026-07-26 Centurion re-record moves ONLY that vehicle: its bazooka plates drop 0.07 m,
/// from 0.40 to 0.33, to keep standing at the axle line after the Horstmann wheels shrank to
/// their real ⌀0.61 (the pairs used to interpenetrate by 0.20 m). Nothing else in the hull moves,
/// and the running gear itself is instanced, so no other row changes.
///
/// The 2026-08-08 re-record is the MATERIAL LAW's first pass, and it moves eight of the nine
/// rows because it changes shared CONSTRUCTION rather than any one vehicle's layout. Measured
/// beforehand: seven of eight vehicles tagged four material roles (RolledArmor, CastArmor,
/// BarrelSteel, TrackMetal) and no other — every lens and every prism in the fleet outside the
/// T-54 was being drawn as one of four kinds of steel. `deck_details::headlight` now builds a
/// painted housing whose bezel turns in over a recessed `Glass` lens instead of a solid
/// `BarrelSteel` cylinder, and the driver's, commander's and British sight hoods carry a glass
/// prism face (`turret_fittings::vision_prism`). `PrototypeMedium` is the one row that does NOT
/// move, which is the check that this went in through the shared fittings: the prototype routes
/// through none of them. Locked by `vehicle_forge/tests/material_law.rs`.
///
/// Previous, before that pass: Tiger I 8_638_016_921_081_242_465; Tiger II
/// 7_566_020_042_162_252_338; Jagdtiger 5_983_034_482_053_846_612; Panther II
/// 7_506_679_536_634_783_988; IS-3 764_441_410_926_956_128; Centurion
/// 15_818_076_589_286_630_709; T-34-85 10_310_688_321_347_204_439 (the T-54's own chain is
/// kept at its row).
pub const GOLDEN_BAKE_HASHES: [(VehicleKind, u64); 9] = [
    (VehicleKind::PrototypeMedium, 17_689_896_064_511_691_746_u64),
    // Re-recorded 2026-07-29 (PR-14, the hull at its documented length): the T-54's hull grows
    // from 6.00 m to 6.235 and its belly from 0.440 to the documented 0.425 clearance. The
    // legacy recipe reads the same `HullShape` the shipping hybrid does, so it moves with it —
    // again in PR-15 for the 2.40 m turret roof and its 2.363 m plan, and in PR-16 for the
    // documented ⌀624 cupola, and in PR-17 for the narrow gun aperture that replaces the
    // external ball mantlet, and in PR-18 for the documented 580 mm belt on the 2.640 m
    // gauge. T-54 only.
    // Re-recorded 2026-08-03 (W4 F5.ii, the authored gun group): the recipe's gun submesh
    // now dispatches on the DATA - a blueprint carrying a GunVisual part bakes the generic
    // revolved group (bore-honest barrel + mantlet body) instead of the legacy plan, and the
    // T-54 is the first vehicle whose blueprint carries one. T-54 only: no other vehicle
    // authors a gun part yet, so no other row moves - which is itself the proof the dispatch
    // reads data, not identity.
    // Previous: 10_764_434_940_917_887_702 (PR-18, the 580 mm belt);
    //           4_449_583_882_369_858_906 (PR-17, the aperture);
    //           12_248_531_318_198_965_275 (PR-16, the cupola);
    //           4_620_056_473_903_640_451 (PR-15, the dome);
    //           1_895_447_275_523_063_518 (PR-14, the hull at 6.235);
    //           3_638_672_634_192_500_695 (PR-06, one-slope-one-truth).
    // Re-recorded 2026-08-08 (the material law): the legacy recipe reads the shared Soviet deck,
    // so its headlight gains the bezel and the Glass lens with the rest of the fleet. The
    // SHIPPED hybrid is untouched — it builds its own lens in `t54_details.rs` and has done
    // since the Model Idealny pass, which is why this row moves and the hybrid's own golden in
    // `vehicle_build/tests/t54_hybrid.rs` does not.
    // Previous: 7_427_199_630_274_926_331 (W4 F5.ii, the authored gun group).
    (VehicleKind::T54_1951, 8_542_046_868_445_520_267_u64),
    // Re-recorded 2026-07-26 for the Tiger I model-logic review: the 3.705 m beam moves onto the
    // 725 mm combat tracks (the sponsons were carrying it, with the belts hiding inside them), the
    // turret roof returns to its documented 2.885 m with an authored drum, the cupola opens to
    // 0.78 m, and the exposed run gets its fender line. Tiger I only.
    // Re-recorded 2026-08-03 (W4 F5.iii): Tiger I is the first vehicle to AUTHOR a visual part
    // in a file — tiger_i_ausf_e.visual.ron carries its gun group, so the recipe bakes the
    // generic revolved group: the bore-honest KwK 36 (88 mm hole in the muzzle face), the
    // double-baffle brake as chambers with a waist, and the Walzenblende body spanning exactly
    // the armour's mantlet patch band (-0.23..+0.07 of the trunnion, radius 0.34). Tiger I only.
    // Previous: 11_582_503_112_659_279_264 (the model-logic review).
    (VehicleKind::TigerI, 2_984_524_824_510_671_745_u64),
    (VehicleKind::TigerII, 11_398_927_513_557_119_832_u64),
    // Re-recorded 2026-07-26 for dossier JT.3: proud cast collar, full-width casemate face,
    // crewed roof, six-shoe racks and hull-flank stowage. Jagdtiger only — the rest of the fleet
    // is byte-identical, which is the check that `plan_front_pad` defaults to no-op.
    (VehicleKind::Jagdtiger, 3_598_140_900_466_594_842_u64),
    // Re-recorded 2026-07-29 (PR-06): the Panther II turret face and rear carried two angles
    // each (11 vs 20, 25 vs 20). The dossier states 20 deg for both, three times over, so the
    // SHAPE moves onto the armour's numbers — a real silhouette change (roof plan narrows) and
    // a real gameplay change (9 deg more slope on the face, 5 less at the rear). Panther II only.
    (VehicleKind::PantherII, 17_885_335_997_969_312_434_u64),
    (VehicleKind::IS3, 18_065_472_775_842_885_288_u64),
    (VehicleKind::Centurion, 8_811_073_067_794_389_069_u64),
    (VehicleKind::T34_85, 13_432_093_406_429_825_707_u64),
];

pub fn golden_bake_hash(kind: VehicleKind) -> Option<u64> {
    GOLDEN_BAKE_HASHES.iter().find(|(k, _)| *k == kind).map(|(_, hash)| *hash)
}
