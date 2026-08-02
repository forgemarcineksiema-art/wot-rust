//! The observer cap is a NAMED bound tied to its own mask, not a literal somewhere.
//!
//! History this file exists to not repeat. `compute_observer_masks` returned `Vec<u16>` and looped
//! `.enumerate().take(16)` — a bare literal that was not a design decision but the mask type's
//! width, written out by hand. Hulls past index 16 observed nobody: no panic, no log, just a crew
//! that never saw an enemy. At 7v7 (fourteen hulls) it was invisible; an 8v8 sits exactly on the
//! boundary and anything larger loses vision silently.
//!
//! The same fact lived in a third place — `1 << viewer_index` in `net::snapshot_filter`, an
//! unguarded shift that would have overflowed rather than merely gone quiet.
//!
//! One truth now: the mask type. Everything else is derived from it, and these tests hold the
//! derivation.

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{MAX_OBSERVERS, ObserverMask, SimulationState, compute_observer_masks};

#[test]
fn the_cap_is_the_mask_width_and_nothing_else() {
    assert_eq!(
        MAX_OBSERVERS,
        ObserverMask::BITS as usize,
        "the cap must be the mask's own width — a hand-written number is how they drift apart"
    );
    // A 7v7 fields 14 hulls and an 8v8 exactly 16, so the cap must not sit on a plausible roster.
    const { assert!(MAX_OBSERVERS >= 32) };
}

/// Every hull in a roster is a real observer — including the ones past where the old literal
/// stopped counting. The roster here is deliberately LARGER than the retired cap of sixteen:
/// a sixteen-hull test would have passed under the bug, because index fifteen is the last one
/// `.take(16)` still reaches. A lock that cannot fail is not a lock.
#[test]
fn hulls_past_the_retired_sixteen_still_see() {
    const RETIRED_CAP: usize = 16;
    let mut state = SimulationState::new();
    let roster = RETIRED_CAP + 4;
    for index in 0..roster {
        let team = TeamId(if index % 2 == 0 { 1 } else { 2 });
        state.spawn_tank(team, TankSpec::t54_1951(), Vec3::new(index as f32 * 3.0, 0.0, 0.0));
    }

    let masks = compute_observer_masks(state.tanks(), 0, None, &[]);
    assert_eq!(masks.len(), roster);

    // The highest-index hull is a friend of every other even-index hull, so its bit MUST appear
    // in their masks. Under the old `.take(16)` on a 16-hull roster it sat exactly at the edge.
    let last = roster - 1;
    assert!(last >= RETIRED_CAP, "the probed index must lie beyond the old literal");
    let seen_by_last = masks
        .iter()
        .enumerate()
        .filter(|(index, mask)| *index != last && **mask & (1 << last) != 0)
        .count();
    assert!(
        seen_by_last > 0,
        "the last hull observes nobody — the cap silenced it instead of the roster ending"
    );
}
