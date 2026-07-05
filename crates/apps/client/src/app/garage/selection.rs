//! Vehicle selection and the per-vehicle draft memory: switching vehicles stashes the outgoing
//! draft and restores the incoming one (or stock if never edited), instead of the old reset. The
//! disk half of persistence lives in `persistence.rs`; this is its in-memory counterpart.

use game_core::VehicleKind;

use super::{
    FitSlot, GarageState, HERO_ORBIT_DISTANCE, HERO_ORBIT_PITCH, HERO_ORBIT_YAW, LoadoutDraft,
};

impl GarageState {
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
            self.restore_hero_framing();
            self.rejected_slot = None;
            self.focused_slot = FitSlot::Gun;
            self.persist();
        }
    }

    pub(in crate::app) fn cycle(&mut self, delta: isize) {
        let len = VehicleKind::PLAYABLE.len() as isize;
        let index = (self.selected_index as isize + delta).rem_euclid(len) as usize;
        self.select_index(index);
    }

    fn restore_hero_framing(&mut self) {
        self.orbit_yaw = HERO_ORBIT_YAW;
        self.orbit_pitch = HERO_ORBIT_PITCH;
        self.orbit_distance = HERO_ORBIT_DISTANCE;
    }
}
