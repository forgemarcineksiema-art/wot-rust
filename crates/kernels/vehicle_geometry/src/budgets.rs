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
    gun_tri: (24, 500),
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

/// Recorded 2026-07-29 (PR-21), with the fleet's worst vehicle at each line and a hand of
/// headroom for the construction work W4 is about to do:
///
/// | vehicle | near | far | saved |
/// |---|---:|---:|---:|
/// | T-54 | 38,568 | 15,092 | 61% |
/// | IS-3 | 38,448 | 15,448 | 60% |
/// | Jagdtiger | 26,392 | 13,788 | 48% |
/// | T-34-85 | 19,800 | 9,420 | 52% |
///
/// The near ceiling is deliberately close to today's worst. W4 adds real construction to these
/// parts (OMSh hinge eyes, twin tyres, sprocket engagement) and it must PAY for it — PR-22 alone
/// removes 11,520 triangles per tank that are currently buried inside the link's backing slab and
/// render nothing.
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
pub const GOLDEN_BAKE_HASHES: [(VehicleKind, u64); 9] = [
    (VehicleKind::PrototypeMedium, 17_689_896_064_511_691_746_u64),
    // Re-recorded 2026-07-29 (PR-06, one-slope-one-truth): the T-54's `hull.rear_slope_deg`
    // was 8 deg while the armour resolved 5 deg, and the SHIPPING hybrid builds its rear plate
    // from the armour value — so this legacy-recipe row is the only place the ghost angle was
    // still visible. T-54 only.
    (VehicleKind::T54_1951, 3_638_672_634_192_500_695_u64),
    // Re-recorded 2026-07-26 for the Tiger I model-logic review: the 3.705 m beam moves onto the
    // 725 mm combat tracks (the sponsons were carrying it, with the belts hiding inside them), the
    // turret roof returns to its documented 2.885 m with an authored drum, the cupola opens to
    // 0.78 m, and the exposed run gets its fender line. Tiger I only.
    (VehicleKind::TigerI, 11_582_503_112_659_279_264_u64),
    (VehicleKind::TigerII, 7_566_020_042_162_252_338_u64),
    // Re-recorded 2026-07-26 for dossier JT.3: proud cast collar, full-width casemate face,
    // crewed roof, six-shoe racks and hull-flank stowage. Jagdtiger only — the rest of the fleet
    // is byte-identical, which is the check that `plan_front_pad` defaults to no-op.
    (VehicleKind::Jagdtiger, 5_983_034_482_053_846_612_u64),
    // Re-recorded 2026-07-29 (PR-06): the Panther II turret face and rear carried two angles
    // each (11 vs 20, 25 vs 20). The dossier states 20 deg for both, three times over, so the
    // SHAPE moves onto the armour's numbers — a real silhouette change (roof plan narrows) and
    // a real gameplay change (9 deg more slope on the face, 5 less at the rear). Panther II only.
    (VehicleKind::PantherII, 7_506_679_536_634_783_988_u64),
    (VehicleKind::IS3, 764_441_410_926_956_128_u64),
    (VehicleKind::Centurion, 15_818_076_589_286_630_709_u64),
    (VehicleKind::T34_85, 10_310_688_321_347_204_439_u64),
];

pub fn golden_bake_hash(kind: VehicleKind) -> Option<u64> {
    GOLDEN_BAKE_HASHES.iter().find(|(k, _)| *k == kind).map(|(_, hash)| *hash)
}
