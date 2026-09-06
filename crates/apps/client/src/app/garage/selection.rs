//! Vehicle selection and the per-vehicle draft memory: switching vehicles stashes the outgoing
//! draft and restores the incoming one (or stock if never edited), instead of the old reset. The
//! disk half of persistence lives in `persistence.rs`; this is its in-memory counterpart. The
//! arrows and the carousel walk the ROSTER the chips let through (G9).

use game_core::VehicleKind;

use super::layout::{CAR_VISIBLE, carousel_window, clamp_carousel_scroll};
use super::{FitSlot, GarageState, LoadoutDraft};

impl GarageState {
    /// Select a vehicle by kind: the filtered cycle's word (the carousel and the tree select by
    /// absolute index).
    pub(in crate::app) fn select_vehicle(&mut self, vehicle: VehicleKind) {
        if let Some(index) = VehicleKind::PLAYABLE.iter().position(|kind| *kind == vehicle) {
            self.select_index(index);
        }
    }

    pub(in crate::app) fn select_index(&mut self, index: usize) {
        if index < VehicleKind::PLAYABLE.len() && index != self.selected_index {
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
            self.snap_to_hero_view();
            self.start_drive_in();
            self.rejected_slot = None;
            self.focused_slot = FitSlot::Gun;
            self.option_list = None;
            // G8: a new hull parks its turret straight; G3: the hull on the turntable compares
            // to nothing.
            self.hero_turret_yaw = 0.0;
            if self.compare == Some(self.selected_vehicle()) {
                self.compare = None;
            }
            self.persist();
        }
    }

    /// `←` / `→`: the next hull the chips let through (G9), wrapping; a selection the chips
    /// exclude steps onto the roster from its end.
    pub(in crate::app) fn cycle(&mut self, delta: isize) {
        let roster = self.roster();
        if roster.is_empty() {
            return;
        }
        let len = roster.len() as isize;
        let next = match roster.iter().position(|kind| *kind == self.selected_vehicle()) {
            Some(at) => (at as isize + delta).rem_euclid(len),
            None if delta >= 0 => 0,
            None => len - 1,
        };
        self.select_vehicle(roster[next as usize]);
    }

    /// Scroll the carousel window by one step (`-1` left, `+1` right), clamped to the roster.
    pub(in crate::app) fn scroll_carousel(&mut self, delta: i8) {
        let next = self.carousel_scroll as isize + delta as isize;
        self.carousel_scroll = clamp_carousel_scroll(self.roster().len(), next.max(0) as usize);
    }

    /// Whether the cursor is over the carousel row — used to route the mouse wheel to scrolling
    /// instead of camera zoom.
    pub(in crate::app) fn cursor_over_carousel(&self) -> bool {
        let ui = self.ui();
        super::screen::build_screen_list(self, &ui, None)
            .find(super::elements::GarageElement::CarouselPlate)
            .is_some_and(|plate| plate.rect.contains(self.cursor_px(&ui)))
    }

    /// Nudge the scroll so the selected vehicle sits inside the visible window.
    pub(super) fn scroll_selection_into_view(&mut self) {
        let roster = self.roster();
        let count = roster.len();
        if count <= CAR_VISIBLE {
            self.carousel_scroll = 0;
            return;
        }
        let at = roster.iter().position(|kind| *kind == self.selected_vehicle()).unwrap_or(0);
        let window = carousel_window(count, self.carousel_scroll);
        if at < window.start {
            self.carousel_scroll = at;
        } else if at >= window.end {
            self.carousel_scroll = at + 1 - CAR_VISIBLE;
        }
        self.carousel_scroll = clamp_carousel_scroll(count, self.carousel_scroll);
    }
}
