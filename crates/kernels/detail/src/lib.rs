//! Deterministic semantic detail (renderer-free): bolts, weld beads, handle rails, casting seams and
//! louvre arrays, placed by a **deterministic** scatter keyed on a stable hash of
//! `(vehicle, part, feature, ordinal)` — never process-random. Scatter respects exclusion zones
//! (armour/hitbox boundaries, gun socket, hatches, already-placed details) and is deterministic in
//! placement order as well as shape, so a bake is reproducible.

mod bolt;
mod scatter;
mod weld;

pub use bolt::bolt_head;
pub use scatter::{
    ExclusionZone, ScatterPlacement, ScatterRequest, ScatterSlot, louvre_slats, scatter,
};
pub use weld::{casting_seam, casting_seam_loop, handle_rail, weld_bead};

/// The kinds of semantic detail, used in the stable placement hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureKind {
    Bolt,
    WeldBead,
    Handle,
    CastingSeam,
    Louvre,
}

/// A stable 64-bit hash of `(vehicle, part, feature, ordinal)` — the per-feature seed. Stable across
/// processes and runs (FNV-1a over the identifying strings and ordinal), so scatter is reproducible.
pub fn stable_hash(vehicle: &str, part: &str, feature: FeatureKind, ordinal: u32) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    mix(vehicle.as_bytes());
    mix(&[0]);
    mix(part.as_bytes());
    mix(&[0]);
    mix(&[feature as u8]);
    mix(&ordinal.to_le_bytes());
    h
}
