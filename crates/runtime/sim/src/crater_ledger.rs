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

/// Record one high-explosive ground burst in the ledger: merge with a crater it re-excavates,
/// otherwise append, evicting the oldest past the cap. Deterministic — same bursts in the same
/// order produce the same ledger on every machine.
pub fn record_high_explosive_burst(
    ledger: &mut Vec<CraterRecord>,
    position: Vec3,
    caliber_mm: f32,
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
        ledger.remove(0);
    }
    ledger.push(record);
}
