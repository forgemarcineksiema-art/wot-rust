//! Vehicle selection and the per-vehicle draft memory: switching vehicles stashes the outgoing
//! draft and restores the incoming one (or stock if never edited), instead of the old reset. The
//! disk half of persistence lives in `persistence.rs`; this is its in-memory counterpart.

use game_core::VehicleKind;

use super::layout::{CAR_HALF, CAR_VISIBLE, CAR_Y, carousel_window, clamp_carousel_scroll};
use super::{
    FitSlot, GarageState, HERO_ORBIT_DISTANCE, HERO_ORBIT_PITCH, HERO_ORBIT_YAW, LoadoutDraft,
};

fn roster_len() -> usize {
    VehicleKind::PLAYABLE.len()
}

impl GarageState {
    /// Select a vehicle by kind. A convenience wrapper over `select_index`; used by tests (the
    /// UI selects by carousel index or tech-tree node, never by kind directly).
    #[cfg(test)]
    pub(in crate::app) fn select_vehicle(&mut self, vehicle: VehicleKind) {
        if let Some(index) = VehicleKind::PLAYABLE.iter().position(|kind| *kind == vehicle) {
            self.select_index(index);
        }
    }

    pub(in crate::app) fn select_index(&mut self, index: usize) {
        if index < roster_len() && index != self.selected_index {
            // Stash the outgoing vehicle's edits, then restore the incoming vehicle's own draft
            // (or stock if it has never been edited). The keyboard focus returns to the gun slot —
            // the most-edited slot — and the inspection framing resets.
            self.saved.insert(self.selected_vehicle(), self.draft.to_saved());
            self.selected_index = index;
            self.draft = match self.saved.get(&self.selected_vehicle()) {
                Some(saved) => LoadoutDraft::from_saved(self.selected_vehicle(), saved),
                None => LoadoutDraft::for_vehicle(self.selected_vehicle()),
            };
            self.scroll_selection_into_view();
            self.restore_hero_framing();
            self.rejected_slot = None;
            self.focused_slot = FitSlot::Gun;
            self.persist();
        }
    }

    pub(in crate::app) fn cycle(&mut self, delta: isize) {
        let len = roster_len() as isize;
        let index = (self.selected_index as isize + delta).rem_euclid(len) as usize;
        self.select_index(index);
    }

    /// Scroll the carousel window by one step (`-1` left, `+1` right), clamped to the roster.
    pub(in crate::app) fn scroll_carousel(&mut self, delta: i8) {
        let next = self.carousel_scroll as isize + delta as isize;
        self.carousel_scroll = clamp_carousel_scroll(roster_len(), next.max(0) as usize);
    }

    /// Whether the cursor is over the carousel row — used to route the mouse wheel to scrolling
    /// instead of camera zoom.
    pub(in crate::app) fn cursor_over_carousel(&self) -> bool {
        (self.cursor_clip[1] - CAR_Y).abs() <= CAR_HALF[1] + 0.02
    }

    /// Nudge the scroll so the selected vehicle sits inside the visible window.
    fn scroll_selection_into_view(&mut self) {
        let count = roster_len();
        if count <= CAR_VISIBLE {
            self.carousel_scroll = 0;
            return;
        }
        let window = carousel_window(count, self.carousel_scroll);
        if self.selected_index < window.start {
            self.carousel_scroll = self.selected_index;
        } else if self.selected_index >= window.end {
            self.carousel_scroll = self.selected_index + 1 - CAR_VISIBLE;
        }
        self.carousel_scroll = clamp_carousel_scroll(count, self.carousel_scroll);
    }

    fn restore_hero_framing(&mut self) {
        self.orbit_yaw = HERO_ORBIT_YAW;
        self.orbit_pitch = HERO_ORBIT_PITCH;
        self.orbit_distance = HERO_ORBIT_DISTANCE;
    }
}
