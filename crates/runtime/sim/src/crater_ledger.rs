//! The battle's crater ledger (protocol v31): every high-explosive ground burst excavates a
//! real, replicated crater. The sim appends QUANTIZED records (see `terrain::CraterRecord`), the
//! snapshot re-sends the whole ledger (a late joiner converges like cover states), and the
//! battlefield owner folds it into the heightmap overlay — so the server, the predictor and
//! every client stand in bit-identical holes.

use glam::Vec3;
use terrain::{CRATER_KIND_HIGH_EXPLOSIVE, CraterRecord};

/// The ledger's hard cap: past it the OLDEST crater weathers away (soil slumps, ruts fill in).
/// 64 five-byte records keep the wire and the overlay query trivially cheap.
pub const MAX_CRATERS: usize = 64;

/// A burst landing within this fraction of the bigger crater's radius re-excavates it instead
/// of stacking a near-duplicate record.
const MERGE_DISTANCE_FRACTION: f32 = 0.35;

/// Deepening on a re-shelled spot never exceeds the shared depth cap.
const DEPTH_MAX_M: f32 = 1.2;

/// How far past a crater's rim a hull still counts as standing IN it. Roughly a hull half-length,
/// so a tank whose running gear reaches into the hole is protected from having that hole filled
/// in under it.
const HULL_IN_CRATER_REACH_M: f32 = 3.5;

/// Record one high-explosive ground burst in the ledger: merge with a crater it re-excavates,
/// otherwise append, evicting the oldest past the cap. Deterministic — same bursts in the same
/// order produce the same ledger on every machine.
///
/// `hull_positions` is every hull the ground under which must not move. Eviction takes the oldest
/// crater NOBODY is standing in, and if every crater is occupied the burst simply leaves no
/// record. Weathering away a hole with a tank in it un-deforms the heightmap under that tank, and
/// `resolve_vertical`'s rule that rising ground always carries the hull turns that into a snap of
/// up to the full [`DEPTH_MAX_M`] in one tick — a teleport, out of nowhere, on the far side of the
/// map from whatever fired the 65th shell.
pub fn record_high_explosive_burst(
    ledger: &mut Vec<CraterRecord>,
    position: Vec3,
    caliber_mm: f32,
    hull_positions: &[Vec3],
) {
    let radius_m = terrain::he_crater_radius_m(caliber_mm);
    let depth_m = terrain::he_crater_depth_m(radius_m);
    let record = CraterRecord::from_world(
        position.x,
        position.z,
        radius_m,
        depth_m,
        CRATER_KIND_HIGH_EXPLOSIVE,
    );

    if let Some(existing) = ledger.iter_mut().find(|crater| {
        let dx = crater.x_m() - record.x_m();
        let dz = crater.z_m() - record.z_m();
        let merge_reach = crater.radius_m().max(record.radius_m()) * MERGE_DISTANCE_FRACTION;
        crater.kind == record.kind && dx * dx + dz * dz <= merge_reach * merge_reach
    }) {
        // Re-shelling the same spot deepens and widens the hole instead of duplicating it.
        existing.radius_q = existing.radius_q.max(record.radius_q);
        let deepened = existing.depth_m() + record.depth_m() * 0.5;
        existing.depth_q = CraterRecord::from_world(
            existing.x_m(),
            existing.z_m(),
            existing.radius_m(),
            deepened.min(DEPTH_MAX_M),
            existing.kind,
        )
        .depth_q;
        return;
    }

    if ledger.len() >= MAX_CRATERS {
        let Some(evictable) =
            ledger.iter().position(|crater| !hull_stands_in(crater, hull_positions))
        else {
            // Every hole on the field has someone in it. The ground stays as it is: a burst that
            // leaves no crater is a far smaller lie than a hull popped a metre into the air.
            return;
        };
        ledger.remove(evictable);
    }
    ledger.push(record);
}

/// Whether any hull is standing in this crater — i.e. whether filling it back in would move one.
fn hull_stands_in(crater: &CraterRecord, hull_positions: &[Vec3]) -> bool {
    let reach = crater.radius_m() + HULL_IN_CRATER_REACH_M;
    hull_positions.iter().any(|hull| {
        let dx = hull.x - crater.x_m();
        let dz = hull.z - crater.z_m();
        dx * dx + dz * dz <= reach * reach
    })
}
