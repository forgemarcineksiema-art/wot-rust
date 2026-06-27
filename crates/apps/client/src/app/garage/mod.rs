mod actions;
mod draft;
mod layout;
mod overlay;
mod panels;
mod types;

use game_core::{TankSpec, VehicleKind};

pub(crate) use self::draft::{FitSlot, LoadoutDraft};
pub(super) use self::types::{GarageHit, GarageView};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GarageState {
    open: bool,
    started: bool,
    selected_index: usize,
    draft: LoadoutDraft,
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

    pub(super) fn select_vehicle(&mut self, vehicle: VehicleKind) {
        if let Some(index) = VehicleKind::PLAYABLE.iter().position(|kind| *kind == vehicle) {
            self.select_index(index);
        }
    }

    pub(super) fn select_index(&mut self, index: usize) {
        if index < VehicleKind::PLAYABLE.len() && index != self.selected_index {
            self.selected_index = index;
            // A different vehicle starts from its own stock loadout, ammo, and crew, and the
            // keyboard focus returns to the gun slot — the most-edited slot.
            self.draft = LoadoutDraft::for_vehicle(self.selected_vehicle());
            self.restore_hero_framing();
            self.rejected_slot = None;
            self.focused_slot = FitSlot::Gun;
        }
    }

    pub(super) fn cycle(&mut self, delta: isize) {
        let len = VehicleKind::PLAYABLE.len() as isize;
        let index = (self.selected_index as isize + delta).rem_euclid(len) as usize;
        self.select_index(index);
    }

    fn restore_hero_framing(&mut self) {
        self.orbit_yaw = HERO_ORBIT_YAW;
        self.orbit_pitch = HERO_ORBIT_PITCH;
        self.orbit_distance = HERO_ORBIT_DISTANCE;
    }

    pub(super) fn cycle_module(&mut self, slot: FitSlot, dir: isize) {
        self.rejected_slot = None;
        if !self.draft.cycle_module(slot, dir) {
            self.rejected_slot = Some(slot);
        }
    }

    pub(super) fn set_ammo(&mut self, index: usize) {
        self.draft.set_ammo(index);
        self.rejected_slot = None;
    }

    pub(super) fn adjust_proficiency(&mut self, dir: isize) {
        self.draft.adjust_proficiency(dir);
        self.rejected_slot = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_a_new_vehicle_resets_the_draft_to_its_stock_loadout() {
        let mut garage = GarageState::default();
        garage.select_index(1);
        garage.cycle_module(FitSlot::Gun, 1);
        // Switching vehicles and back must discard the edit.
        garage.select_index(2);
        garage.select_index(1);
        let stock = VehicleKind::PLAYABLE[1].spec();
        assert_eq!(garage.draft().assembled_spec().gun.shell, stock.gun.shell);
    }

    #[test]
    fn garage_roster_starts_on_t54_and_rejects_t55a_legacy_clone() {
        let mut garage = GarageState::default();
        assert_eq!(garage.selected_vehicle(), VehicleKind::T54_1951);
        assert!(!VehicleKind::PLAYABLE.contains(&VehicleKind::T55A));

        garage.select_vehicle(VehicleKind::T55A);

        assert_eq!(garage.selected_vehicle(), VehicleKind::T54_1951);
    }

    #[test]
    fn selecting_a_vehicle_restores_the_safe_inspection_framing() {
        let mut garage = GarageState {
            orbit_yaw: -1.0,
            orbit_pitch: 1.0,
            orbit_distance: 6.0,
            ..Default::default()
        };

        garage.select_index(1);

        assert_eq!(garage.orbit_yaw, 0.60);
        assert_eq!(garage.orbit_pitch, 0.28);
        assert_eq!(garage.orbit_distance, 11.5);
    }

    #[test]
    fn confirm_returns_the_edited_spec_and_closes_the_garage() {
        let mut garage = GarageState::default();
        garage.select_vehicle(VehicleKind::TigerII);
        garage.adjust_proficiency(-1);
        let green = garage.draft().assembled_spec().gun.reload_seconds;
        let spec = garage.confirm();
        assert!(!garage.is_open() && garage.has_started());
        assert_eq!(spec.kind, VehicleKind::TigerII);
        assert!(
            (spec.gun.reload_seconds - green).abs() < 1.0e-6,
            "confirmed spec carries the edit"
        );
    }

    #[test]
    fn rejected_slot_starts_none_and_clears_on_successful_cycle() {
        let garage = GarageState::default();
        assert_eq!(garage.rejected_slot(), None);

        let mut garage = GarageState { rejected_slot: Some(FitSlot::Gun), ..Default::default() };
        assert_eq!(garage.rejected_slot(), Some(FitSlot::Gun));

        garage.cycle_module(FitSlot::Engine, 1);
        assert_eq!(garage.rejected_slot(), None, "successful cycle clears rejection");
    }

    #[test]
    fn rejected_slot_clears_on_vehicle_select() {
        let mut garage = GarageState { rejected_slot: Some(FitSlot::Gun), ..Default::default() };
        garage.select_index(1);
        assert_eq!(garage.rejected_slot(), None, "selecting a vehicle clears rejection");
    }

    #[test]
    fn rejected_slot_clears_on_ammo_select() {
        let mut garage = GarageState { rejected_slot: Some(FitSlot::Gun), ..Default::default() };
        garage.set_ammo(1);
        assert_eq!(garage.rejected_slot(), None, "selecting ammo clears rejection");
    }

    #[test]
    fn default_focus_is_gun() {
        let garage = GarageState::default();
        assert_eq!(garage.focused_slot(), FitSlot::Gun);
    }

    #[test]
    fn focus_adjacent_wraps_around_all_slots() {
        let mut garage = GarageState::default();
        // Gun (index 1) -> Hull -> ... -> Turret (index 0) -> Gun (wrap forward).
        for _ in 0..FitSlot::ALL.len() {
            garage.focus_adjacent(1);
        }
        assert_eq!(garage.focused_slot(), FitSlot::Gun, "full loop returns to gun");

        // Backward from Gun (index 1) -> Turret (index 0); backward from Turret wraps to Radio.
        garage.focus_adjacent(-1);
        assert_eq!(garage.focused_slot(), FitSlot::Turret);
        garage.focus_adjacent(-1);
        assert_eq!(garage.focused_slot(), FitSlot::Radio, "backward wraps to last slot");
    }

    #[test]
    fn cycle_focused_cycles_the_focused_slot_option() {
        let mut garage = GarageState::default();
        garage.select_vehicle(VehicleKind::T54_1951);
        let before = garage.draft().gun_barrel_length();
        // Focus starts on Gun; cycle forward.
        garage.cycle_focused(1);
        assert_ne!(garage.draft().gun_barrel_length(), before, "focused cycle changed the gun");
    }

    #[test]
    fn selecting_a_vehicle_resets_focus_to_gun() {
        let mut garage = GarageState::default();
        garage.focus_adjacent(1); // move focus off Gun
        assert_ne!(garage.focused_slot(), FitSlot::Gun);
        garage.select_index(1);
        assert_eq!(garage.focused_slot(), FitSlot::Gun, "vehicle select resets focus to gun");
    }

    #[test]
    fn default_view_is_hangar() {
        let garage = GarageState::default();
        assert_eq!(garage.view(), GarageView::Hangar);
    }

    #[test]
    fn open_tech_tree_switches_view() {
        let mut garage = GarageState::default();
        garage.open_tech_tree();
        assert_eq!(garage.view(), GarageView::TechTree);
    }

    #[test]
    fn close_tech_tree_restores_hangar() {
        let mut garage = GarageState::default();
        garage.open_tech_tree();
        garage.close_tech_tree();
        assert_eq!(garage.view(), GarageView::Hangar);
    }

    #[test]
    fn open_tech_tree_cancels_drag() {
        let mut garage = GarageState::default();
        garage.begin_drag();
        garage.open_tech_tree();
        assert!(!garage.is_dragging(), "opening tech tree cancels any in-progress drag");
    }
}
