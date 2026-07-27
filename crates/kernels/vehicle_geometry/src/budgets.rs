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
pub const GOLDEN_BAKE_HASHES: [(VehicleKind, u64); 10] = [
    (VehicleKind::PrototypeMedium, 5_209_003_704_901_676_167_u64),
    (VehicleKind::T54_1951, 7_946_555_521_149_600_740_u64),
    (VehicleKind::TigerI, 14_348_569_545_994_162_257_u64),
    (VehicleKind::TigerII, 12_544_834_639_457_414_026_u64),
    (VehicleKind::Jagdtiger, 5_179_577_790_178_323_578_u64),
    (VehicleKind::PantherII, 9_168_003_294_476_122_692_u64),
    (VehicleKind::IS3, 5_050_614_938_041_723_061_u64),
    (VehicleKind::Centurion, 15_577_022_592_979_033_199_u64),
    (VehicleKind::T34_85, 8_651_410_882_145_847_605_u64),
    (VehicleKind::KV1_1942, 16_991_067_458_347_792_984_u64),
];

pub fn golden_bake_hash(kind: VehicleKind) -> Option<u64> {
    GOLDEN_BAKE_HASHES.iter().find(|(k, _)| *k == kind).map(|(_, hash)| *hash)
}
