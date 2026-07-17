//! The fleet's cost contract as DATA: per-submesh/per-vehicle triangle and vertex budgets, and
//! the golden deterministic bake hash per vehicle. One source read by both the enforcement
//! tests (`tests/vehicle_budgets.rs`) and the Forge Studio report — the tool an author reads
//! must quote the same numbers the gate enforces, or the loop teaches the wrong limits.

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

/// The lineup-wide budget envelope (min, max) every baked vehicle must sit inside.
pub const VEHICLE_BUDGETS: VehicleBudgets = VehicleBudgets {
    hull_tri: (120, 2400),
    turret_tri: (24, 900),
    gun_tri: (24, 500),
    vehicle_tri: (250, 3600),
    vehicle_vert_max: 10_000,
};

/// The recorded golden bake hash per vehicle. Re-recorded for the fleet fittings pass — driver hatches, headlights, tow hooks and antenna
/// pots in the shared deck details, with turret clearance measured to the FITTING EDGE (a T-55A
/// hatch corner grazing the turret seat failed the client embed gate) — on top of Program C: the
/// generic blueprint contact-cavity bake (`recipes::blueprint_cavity`) writes surface_shade
/// into every blueprint vehicle's vertices — an intentional, fleet-wide shading change; the
/// prototype (no blueprint) is byte-identical. A geometry change that is INTENTIONAL re-records
/// its entry (the enforcement test prints the replacement array); anything else failing here is
/// a regression. Kept beside the budgets so the studio report can say whether the current bake
/// still matches its golden.
// Fleet re-record (model-logic audit, defect #7): the static belt band now follows the SAME
// belt path the links ride — ground run pressed under the wheels, true wrap positions, and a
// top run that drapes onto its carriers with the v27 tension read — replacing the taut boxes.
pub const GOLDEN_BAKE_HASHES: [(VehicleKind, u64); 9] = [
    (VehicleKind::PrototypeMedium, 12_366_106_953_789_565_521_u64),
    // Re-recorded for the T-54 1:1 blueprint reset (documented 6.04 × 3.27 × 2.40 m body,
    // 810 mm wheels, raised idler/sprocket, 2.25 m turret on the 1.75 m roof). The T-55A moves
    // only through the shared Soviet cupola cap; the German vehicles are untouched.
    (VehicleKind::T54_1951, 2_673_530_798_804_038_998_u64),
    // Re-recorded for the W1 late-E detail pass: exhaust shields shroud the twin stacks and
    // four spare track links rack on the lower bow (the Feifel cleaners stay deliberately
    // absent — the modelled variant is a late Ausf. E, which dropped them).
    (VehicleKind::TigerI, 3_021_918_926_565_464_375_u64),
    // Re-recorded for the Tiger II blueprint migration: the researched 1:1 sloped body
    // (7.38 m hull, fleet's longest glacis at 50°, 25° leaned upper sides, Henschel prism
    // with bustle, 9 overlapped wheels, braked KwK 43) replaces the legacy stretch.
    (VehicleKind::TigerII, 2_275_320_215_993_179_203_u64),
    // Re-recorded for the Jagdtiger blueprint migration: the researched 1:1 casemate
    // (7.80 m hull, superstructure flank continuing the hull's 25° plane, 15° 250 mm face,
    // periscope roof, 9 overlapped wheels, braked PaK 44) replaces the legacy box.
    (VehicleKind::Jagdtiger, 9_214_004_648_881_314_087_u64),
    // Re-recorded for the Panther II blueprint migration: the researched 1:1 wedge (6.87 m
    // hull, steepest German glacis at 55°, 29° leaned sides, narrow Schmalturm, 7 overlapped
    // steel wheels, braked KwK 42) replaces the last legacy German body.
    (VehicleKind::PantherII, 16_355_403_364_216_964_623_u64),
    // Re-recorded for the IS-3 finish pass: the signature external fuel drums join the
    // rear fender shelves; the rest of the fleet is untouched.
    (VehicleKind::IS3, 14_691_152_626_051_102_928_u64),
    // Recorded at birth: the Centurion Mk 3 (skirted hull over Horstmann bogie pairs,
    // 57° glacis, cast Mk 3 dome with the bustle bin, clean 20-pounder).
    (VehicleKind::Centurion, 5_837_580_918_550_652_731_u64),
    // Recorded at birth: the T-34-85 — the first vehicle authored through the Forge Studio
    // loop (60° glacis, raked sides, five bare Christie wheels with the open gap, low wide
    // -85 cast dome seated forward, clean ZiS-S-53).
    (VehicleKind::T34_85, 14_378_401_308_748_658_762_u64),
];

/// The golden hash recorded for `kind`, if any.
pub fn golden_bake_hash(kind: VehicleKind) -> Option<u64> {
    GOLDEN_BAKE_HASHES.iter().find(|(k, _)| *k == kind).map(|(_, hash)| *hash)
}
