mod actions;
mod draft;
mod layout;
mod overlay;
mod panels;
mod persistence;
mod selection;
#[cfg(test)]
mod state_tests;
mod types;

use std::collections::HashMap;
use std::path::PathBuf;

use game_core::{TankSpec, VehicleKind};

pub(crate) use self::draft::{FitSlot, LoadoutDraft};
use self::persistence::SavedLoadout;
pub(super) use self::types::{GarageHit, GarageView};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GarageState {
    open: bool,
    started: bool,
    selected_index: usize,
    draft: LoadoutDraft,
    /// Edited loadouts for the *non-selected* vehicles, so switching back restores each tank's
    /// own draft instead of resetting to stock. The selected vehicle's live draft is `draft`.
    saved: HashMap<VehicleKind, SavedLoadout>,
    /// Where to persist edits, or `None` for a pure in-memory garage (tests, offscreen renders).
    save_path: Option<PathBuf>,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    cursor_clip: [f32; 2],
    dragging: bool,
    /// The slot whose last cycle attempt was rejected by compatibility, or `None`.
    /// Shown red in the loadout strip until any other interaction clears it.
    rejected_slot: Option<FitSlot>,
    /// The module slot highlighted by keyboard focus (`[`/`]`); `Q`/`E` cycle its option.
    focused_slot: FitSlot,
    /// Which garage screen is active (hangar vs tech tree).
    view: GarageView,
}

const HERO_ORBIT_YAW: f32 = 0.60;
const HERO_ORBIT_PITCH: f32 = 0.28;
const HERO_ORBIT_DISTANCE: f32 = 11.5;

impl Default for GarageState {
    fn default() -> Self {
        Self {
            open: true,
            started: false,
            selected_index: 0,
            draft: LoadoutDraft::for_vehicle(VehicleKind::PLAYABLE[0]),
            saved: HashMap::new(),
            save_path: None,
            orbit_yaw: HERO_ORBIT_YAW,
            orbit_pitch: HERO_ORBIT_PITCH,
            orbit_distance: HERO_ORBIT_DISTANCE,
            cursor_clip: [2.0, 2.0],
            dragging: false,
            rejected_slot: None,
            focused_slot: FitSlot::Gun,
            view: GarageView::Hangar,
        }
    }
}

/// Build the garage HUD overlay for an offscreen review render. `tech_tree = true` switches to
/// the browse-only tech tree view; `false` renders the default hangar overlay.
pub fn garage_overlay(tech_tree: bool, aspect: f32) -> Vec<renderer_api::HudVertex> {
    let mut state = GarageState::default();
    if tech_tree {
        state.open_tech_tree();
    }
    state.overlay_vertices(aspect)
}

impl GarageState {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn has_started(&self) -> bool {
        self.started
    }

    pub(super) fn open(&mut self) {
        self.open = true;
        self.dragging = false;
    }

    pub(super) fn close_if_started(&mut self) {
        if self.started {
            self.open = false;
        }
    }

    pub(super) fn selected_vehicle(&self) -> VehicleKind {
        VehicleKind::PLAYABLE[self.selected_index]
    }

    pub(super) fn cycle_module(&mut self, slot: FitSlot, dir: isize) {
        self.rejected_slot = None;
        if !self.draft.cycle_module(slot, dir) {
            self.rejected_slot = Some(slot);
        }
        self.persist();
    }

    pub(super) fn set_ammo(&mut self, index: usize) {
        self.draft.set_ammo(index);
        self.rejected_slot = None;
        self.persist();
    }

    pub(super) fn adjust_proficiency(&mut self, dir: isize) {
        self.draft.adjust_proficiency(dir);
        self.rejected_slot = None;
        self.persist();
    }

    /// Step keyboard focus between module slots (`[` prev, `]` next), wrapping around.
    pub(super) fn focus_adjacent(&mut self, dir: isize) {
        let len = FitSlot::ALL.len() as isize;
        let current = self.focused_slot.index() as isize;
        self.focused_slot = FitSlot::ALL[((current + dir).rem_euclid(len)) as usize];
    }

    /// Cycle the keyboard-focused module slot's option (`Q` backward, `E` forward).
    pub(super) fn cycle_focused(&mut self, dir: isize) {
        self.cycle_module(self.focused_slot, dir);
    }

    pub(super) fn focused_slot(&self) -> FitSlot {
        self.focused_slot
    }

    pub(super) fn view(&self) -> GarageView {
        self.view
    }

    #[cfg(test)]
    pub(super) fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub(super) fn open_tech_tree(&mut self) {
        self.view = GarageView::TechTree;
        self.dragging = false;
    }

    pub(super) fn close_tech_tree(&mut self) {
        self.view = GarageView::Hangar;
        self.dragging = false;
    }

    /// Commit the edited loadout: lock the garage and hand back the assembled spec to install.
    pub(super) fn confirm(&mut self) -> TankSpec {
        self.started = true;
        self.open = false;
        self.dragging = false;
        self.persist();
        self.draft.assembled_spec()
    }

    // --- accessors for the overlay (and hit test) -----------------------------------------

    pub(super) fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub(super) fn draft(&self) -> &LoadoutDraft {
        &self.draft
    }

    pub(super) fn cursor_clip(&self) -> [f32; 2] {
        self.cursor_clip
    }

    pub(super) fn rejected_slot(&self) -> Option<FitSlot> {
        self.rejected_slot
    }

    /// Length scale for the parked tank's gun submesh so swapping guns visibly changes the
    /// silhouette: the ratio of the installed barrel to the vehicle's stock barrel (the baked mesh
    /// represents the stock gun).
    pub(super) fn gun_silhouette_scale(&self) -> f32 {
        let stock = self.selected_vehicle().stock_barrel_length_m();
        if stock <= 0.0 {
            return 1.0;
        }
        (self.draft.gun_barrel_length() / stock).clamp(0.6, 1.6)
    }

    pub(super) fn overlay_vertices(&self, aspect: f32) -> Vec<renderer_api::HudVertex> {
        if !self.open {
            return Vec::new();
        }
        overlay::build(self, aspect)
    }

    pub(super) fn hit_test(&self, shift: bool) -> GarageHit {
        overlay::hit_test(self, shift)
    }
}
