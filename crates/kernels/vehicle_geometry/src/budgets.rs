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
    (VehicleKind::T54_1951, 1_352_245_318_290_454_355_u64),
    (VehicleKind::TigerI, 2_528_636_824_404_053_672_u64),
    (VehicleKind::TigerII, 7_566_020_042_162_252_338_u64),
    (VehicleKind::Jagdtiger, 12_913_696_618_745_343_377_u64),
    (VehicleKind::PantherII, 2_837_722_706_443_665_062_u64),
    (VehicleKind::IS3, 764_441_410_926_956_128_u64),
    (VehicleKind::Centurion, 15_818_076_589_286_630_709_u64),
    (VehicleKind::T34_85, 10_310_688_321_347_204_439_u64),
];

pub fn golden_bake_hash(kind: VehicleKind) -> Option<u64> {
    GOLDEN_BAKE_HASHES.iter().find(|(k, _)| *k == kind).map(|(_, hash)| *hash)
}
