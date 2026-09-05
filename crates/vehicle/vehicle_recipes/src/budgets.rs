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

/// The PROCEDURAL bake envelope — every vehicle whose mesh comes from the recipes in this crate,
/// which is the whole fleet except the hybrid benchmark.
///
/// Not lineup-wide, and the distinction is load-bearing: the T-54 the game ships bakes through
/// `vehicle_build` and is held to that crate's per-class LOD0 budgets instead (26,000 triangles,
/// 18,000 vertices — several times these numbers, because it is a several-times denser mesh).
/// Which envelope governs a given vehicle is answered in one place,
/// `vehicle_forge::shipped_cost_ceiling`, and gated on the shipped mesh by
/// `vehicle_forge/tests/shipped_cost.rs`. Reading this table as the fleet's only ceiling is how
/// the hybrid's vertex count went unmeasured until 2026-08-09.
///
/// Runtime running gear is excluded: wheels, suspension, end wheels, and links are animated
/// instances. The continuous backing skin belongs to each moving link rather than the static hull,
/// so a thrown track leaves no frozen ghost belt.
pub const VEHICLE_BUDGETS: VehicleBudgets = VehicleBudgets {
    hull_tri: (120, 2750),
    // Raised 900 -> 1150 (measured 2026-08-08, the roundness law): a cupola is the same radius as
    // an idler and was getting 12 segments where the idler's floor gives it 20, purely because one
    // went through `segments_for` and the other was typed at its call site. With segments derived
    // from radius the fleet's worst turret is the Centurion at 1052 (its Mk 3 cupola is the widest
    // drum on any turret here); 1150 gives that measured shape 9% headroom.
    //
    // Affordable, and the frame says so rather than taste: the whole fleet gains roughly 250-400
    // triangles per vehicle, ~4k across a 7v7 against the 616k the scene submits, inside the
    // 2.99 ms of p95 headroom measured on the MX330 min spec.
    turret_tri: (24, 1150),
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
/// Re-measured 2026-08-08 (the shoe-pitch fix). Five vehicles had no authored shoe count and were
/// rendering shoes 1.65-2.05x too long; authoring the documented counts roughly doubles their
/// links, which is the whole point and it has to be paid for:
///
/// | vehicle | near before -> after | far before -> after |
/// |---|---|---|
/// | Tiger II | 28,712 -> 39,336 | 14,012 -> 17,276 |
/// | Jagdtiger | 28,936 -> 39,336 | 14,076 -> 17,276 |
/// | Centurion | 24,816 -> 38,032 | 11,464 -> 15,560 |
/// | Panther II | 24,120 -> 34,808 | 11,492 -> 14,820 |
/// | T-34-85 | 23,088 -> 28,912 | 10,368 -> 12,480 |
///
/// The NEAR ceiling holds untouched at 40,000 — the worst vehicle lands at 39,336, which is 1.7%
/// of headroom and deliberately tight. `far_tri_max` moves 17,000 -> 18,500 for the 276 triangles
/// Tiger II and Jagdtiger now cross it by, with 7% of room over the measured worst.
///
/// Affordable, and specifically so: forcing every tank in a 7v7 to FAR gear measures 12.17 ms p50
/// against 12.19 for a normal battle. The distant tier is not where the frame goes, which is
/// exactly why it is the tier that could absorb this.
///
/// What the frame can afford is now measured rather than assumed
/// (measured 2026-08-08 on the MX330 min spec, at the shipped 1x MSAA): a 7v7 costs
/// 12.19 ms p50 / 13.68 ms p95 of GPU work against a 16.67 ms line, and forcing every tank to
/// NEAR gear costs +1.55 ms p50 and leaves **1.46 ms of p95 headroom**. That is the budget the
/// five vehicles with 1.65–2.05x oversized shoes have to fit their fix into.
/// RAISED 40_000 -> 40_500 (2026-08-12, the suspension-read wave, measured): the T-54's four
/// lever shock absorbers land at 40,144 near triangles (+512 over the previous 39,632 — 128 per
/// damper across 4 instances, 208 total instances). Priced at the measured 9.76 ns/triangle x3
/// passes that is 15 us for the one or two vehicles ever close enough to draw NEAR gear, against
/// the 1.46 ms of documented p95 headroom. Far tier keeps its pin-less damper and stays at
/// 15,968 (60% saved, floor 40%). Per-item raise for a named part — not fleet headroom.
pub const GEAR_BUDGETS: GearBudgets =
    GearBudgets { near_tri_max: 40_500, far_tri_max: 18_500, instances_max: 260 };

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
/// prism face (`turret_fittings::vision_prism`). The test-only prototype (since deleted in wire
/// v48) was the one row that did NOT move, which is the check that this went in through the
/// shared fittings: it routed through none of them. Locked by `vehicle_forge/tests/material_law.rs`.
///
/// Previous, before that pass: Tiger I 8_638_016_921_081_242_465; Tiger II
/// 7_566_020_042_162_252_338; Jagdtiger 5_983_034_482_053_846_612; Panther II
/// 7_506_679_536_634_783_988; IS-3 764_441_410_926_956_128; Centurion
/// 15_818_076_589_286_630_709; T-34-85 10_310_688_321_347_204_439 (the T-54's own chain is
/// kept at its row).
/// The 2026-08-08 roundness re-record moves EIGHT rows (the since-deleted prototype routed
/// through none of the affected fittings and stayed put, which is again the check that this went
/// in through shared construction). Segment counts on hand-written revolves — cupolas, the IS-3 fuel drums, deck
/// fittings, headlights — now come from `game_core::roundness::round_segments` instead of a number
/// typed at the call site. Positions are unchanged; only ring resolution moves.
/// Re-recorded 2026-08-12 (the mirror): the world's +X is the vehicle's PORT side, and every
/// hand-authored asymmetric fitting in the fleet — cupolas, driver hatches, bow MG balls,
/// headlights, periscope hoods — was authored under the opposite belief and rendered mirrored
/// (judged against Studio tiles whose camera basis carried the same inversion). EIGHT rows move
/// with the sign flip; the since-deleted prototype was byte-identical, which is the check that
/// the flip went in through the authored data and not through shared construction. Locked from now on by
/// `game_core/tests/handedness.rs` (blueprint-to-screen chain).
/// The bake goldens live in `goldens/bake_hashes.txt` (one row per vehicle: the recipe's LOD0
/// hash and, when the shipped bake is not the recipe, the shipped LOD0 hash). A data file, so
/// `cargo run -p tools -- bless --vehicle <slug>` re-records ONE vehicle without touching Rust;
/// the rows' history is the file's git log (until 2026-09-05 it was this array's comments).
const BAKE_GOLDENS: &str = include_str!("../goldens/bake_hashes.txt");

/// One parsed row of the goldens file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BakeGolden {
    pub kind: VehicleKind,
    /// The recipe bake's LOD0 hash (`bake_vehicle`).
    pub recipe: u64,
    /// The authoritative description's LOD0 hash when it is NOT the recipe bake.
    pub shipped: Option<u64>,
}

/// Every row of the goldens file, parsed once. A malformed row is a broken instrument and
/// panics here, where the gate that reads it will name the line.
pub fn bake_goldens() -> &'static [BakeGolden] {
    static ROWS: std::sync::OnceLock<Vec<BakeGolden>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| {
        BAKE_GOLDENS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let mut fields = line.split_whitespace();
                let name = fields.next().expect("a kind");
                let kind = VehicleKind::PLAYABLE
                    .iter()
                    .copied()
                    .find(|kind| format!("{kind:?}") == name)
                    .unwrap_or_else(|| panic!("bake_hashes.txt: unknown vehicle `{name}`"));
                let recipe = fields
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| panic!("bake_hashes.txt: {name} needs a recipe hash"));
                let shipped = match fields.next() {
                    Some("-") | None => None,
                    Some(s) => Some(s.parse().unwrap_or_else(|_| {
                        panic!("bake_hashes.txt: {name} has a malformed shipped hash")
                    })),
                };
                BakeGolden { kind, recipe, shipped }
            })
            .collect()
    })
}

/// The recipe LOD0 golden for `kind`.
pub fn golden_bake_hash(kind: VehicleKind) -> Option<u64> {
    bake_goldens().iter().find(|row| row.kind == kind).map(|row| row.recipe)
}

/// The SHIPPED LOD0 golden for `kind`: the recorded shipped hash when the authoritative bake is
/// not the recipe, the recipe golden otherwise.
pub fn shipped_bake_hash(kind: VehicleKind) -> Option<u64> {
    bake_goldens()
        .iter()
        .find(|row| row.kind == kind)
        .and_then(|row| row.shipped.or(Some(row.recipe)))
}
