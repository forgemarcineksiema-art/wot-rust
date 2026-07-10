mod actions;
mod camera;
mod draft;
mod drive_in;
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
use glam::Vec3;

use self::camera::CameraTarget;
pub(crate) use self::draft::{FitSlot, LoadoutDraft};
use self::persistence::SavedLoadout;
pub(super) use self::types::{GarageHit, GarageView};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GarageState {
    open: bool,
    started: bool,
    selected_index: usize,
    /// First roster index visible in the carousel window (0 until it overflows `CAR_VISIBLE`).
    carousel_scroll: usize,
    draft: LoadoutDraft,
    /// Edited loadouts for the non-selected vehicles (the selected one's live draft is `draft`),
    /// so switching back restores each tank's own draft; persisted to `save_path` when set.
    saved: HashMap<VehicleKind, SavedLoadout>,
    save_path: Option<PathBuf>,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    // Camera feel (`camera.rs`) + roll-in animation (`drive_in.rs`).
    pivot_offset: Vec3,
    camera_target: Option<CameraTarget>,
    idle_seconds: f32,
    drive_in: drive_in::DriveIn,
    cursor_clip: [f32; 2],
    dragging: bool,
    /// Slot whose last cycle was rejected by compatibility (shown red until any interaction clears).
    rejected_slot: Option<FitSlot>,
    /// Module slot with keyboard focus (`[`/`]` move it, `Q`/`E` cycle it).
    focused_slot: FitSlot,
    /// The slot whose option list is currently open (clicking a swappable slot opens it), or `None`.
    /// The list is the informed swap path — names + stat deltas; `Q`/`E` stay the express cycle.
    option_list: Option<FitSlot>,
    view: GarageView,
}

const HERO_ORBIT_YAW: f32 = 0.60;
const HERO_ORBIT_PITCH: f32 = 0.28;
// Standard framing in the 5–17 m boom: a roomy hangar look with headroom to scroll both ways.
// (The hall is sized so this range never reaches the walls — see camera.rs.)
const HERO_ORBIT_DISTANCE: f32 = 14.0;

impl Default for GarageState {
    fn default() -> Self {
        Self {
            open: true,
            started: false,
            selected_index: 0,
            carousel_scroll: 0,
            draft: LoadoutDraft::for_vehicle(VehicleKind::PLAYABLE[0]),
            saved: HashMap::new(),
            save_path: None,
            orbit_yaw: HERO_ORBIT_YAW,
            orbit_pitch: HERO_ORBIT_PITCH,
            orbit_distance: HERO_ORBIT_DISTANCE,
            pivot_offset: Vec3::ZERO,
            camera_target: None,
            idle_seconds: 0.0,
            drive_in: drive_in::DriveIn::default(),
            cursor_clip: [2.0, 2.0],
            dragging: false,
            rejected_slot: None,
            focused_slot: FitSlot::Gun,
            option_list: None,
            view: GarageView::Hangar,
        }
    }
}

/// Build the garage HUD overlay for an offscreen review render (`tech_tree` picks the view).
pub fn garage_overlay(tech_tree: bool, aspect: f32) -> Vec<renderer_api::HudVertex> {
    let mut state = GarageState::default();
    if tech_tree {
        state.open_tech_tree();
    }
    state.overlay_vertices(aspect)
}

/// Build the garage overlay with a module option list open — for offscreen review of the picker.
/// `vehicle_index` selects a `VehicleKind::PLAYABLE` roster slot and `slot_index` a `FitSlot`
/// (0=Turret, 1=Gun, 2=Hull, 3=Engine, 4=Suspension, 5=Radio); out-of-range values clamp. The list
/// only opens for slots that have a real choice.
pub fn garage_overlay_option_list(
    vehicle_index: usize,
    slot_index: usize,
    aspect: f32,
) -> Vec<renderer_api::HudVertex> {
    let mut state = GarageState::default();
    state.select_index(vehicle_index.min(game_core::VehicleKind::PLAYABLE.len() - 1));
    state.open_option_list(FitSlot::ALL[slot_index.min(FitSlot::ALL.len() - 1)]);
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

    /// Edit the rack fill: move `delta` rounds into/out of ammo slot `index` (clamped to the
    /// vehicle's capacity and a non-empty rack). Persists only when something actually moved.
    pub(super) fn adjust_ammo_count(&mut self, index: usize, delta: i32) {
        if self.draft.adjust_ammo_count(index, delta) {
            self.rejected_slot = None;
            self.persist();
        }
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

    /// The slot whose option list is currently open, if any.
    pub(super) fn option_list(&self) -> Option<FitSlot> {
        self.option_list
    }

    /// Open the informed option list for `slot` (clicking a swappable slot). A slot with no real
    /// choice never opens a list — there is nothing to pick. Opening also focuses the slot so the
    /// `Q`/`E` express cycle and the list act on the same slot.
    pub(super) fn open_option_list(&mut self, slot: FitSlot) {
        if self.draft.has_choice(slot) {
            self.option_list = Some(slot);
            self.focused_slot = slot;
        }
    }

    pub(super) fn close_option_list(&mut self) {
        self.option_list = None;
    }

    /// Install the option `index` for `slot` (the list's direct pick), then close the list. A pick
    /// rejected by compatibility flashes the slot red, mirroring a rejected cycle.
    pub(super) fn select_option(&mut self, slot: FitSlot, index: usize) {
        self.rejected_slot = None;
        if !self.draft.set_option(slot, index) {
            self.rejected_slot = Some(slot);
        }
        self.option_list = None;
        self.persist();
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
        self.option_list = None;
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
        self.option_list = None;
        self.persist();
        self.draft.assembled_spec()
    }

    // --- accessors for the overlay (and hit test) -----------------------------------------

    pub(super) fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub(super) fn carousel_scroll(&self) -> usize {
        self.carousel_scroll
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
