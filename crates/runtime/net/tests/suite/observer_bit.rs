//! The viewer's own-eyes bit is bounded by the mask that carries it.
//!
//! `filtered_for_viewer_with_observers` tested `mask & (1 << viewer_index)` with nothing checking
//! that the viewer's index fits the mask. That was the third place the observer cap lived unnamed
//! — the other two being the mask type in `sim::spotting` and a bare `.take(16)` beside it — and
//! the only one of the three that would have overflowed rather than merely gone quiet.

use game_core::TankId;
use net::Snapshot;
use sim::{MAX_OBSERVERS, ObserverMask};

#[test]
fn a_viewer_past_the_cap_is_refused_rather_than_shifted_out_of_range() {
    let snapshot = Snapshot::default();

    // Every index from inside the cap to well past it must be answerable. Before the guard the
    // ones past `MAX_OBSERVERS` were an overflowing shift, not an answer.
    for viewer_index in [0, 1, MAX_OBSERVERS - 1, MAX_OBSERVERS, MAX_OBSERVERS + 7, usize::MAX] {
        let filtered = snapshot.filtered_for_viewer_with_observers(
            TankId(1),
            &[ObserverMask::MAX],
            viewer_index,
        );
        assert!(
            filtered.tanks.is_empty(),
            "viewer index {viewer_index} must resolve, not overflow"
        );
    }
}
