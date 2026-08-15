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
mod wear;

use std::collections::HashMap;

// The rest framing lives in `scene_build::hangar` so the live camera, the review golden and the
// human-review example cannot disagree about where the hero shot is taken from. Standard framing
// in the 5-17 m boom: the roomy maintenance-hall look with headroom to scroll both ways. The
// 9.5 m repair-bay framing was tried and REJECTED as too close to the hero — locked by
// `the_hero_framing_is_the_roomy_cathedral_shot`. (The hall is sized so this range never reaches
// the walls — see camera.rs.)
use scene_build::hangar::{HERO_ORBIT_DISTANCE, HERO_ORBIT_PITCH, HERO_ORBIT_YAW};
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
    /// Stored loadouts whose vehicle slug this build does not know (a removed vehicle, a save
    /// from a newer build). Carried by slug and written back verbatim on every persist, so an
    /// entry this build cannot use is preserved for the build that can — never destroyed.
    foreign_loadouts: HashMap<String, SavedLoadout>,
    save_path: Option<PathBuf>,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    // Camera feel (`camera.rs`) + roll-in animation (`drive_in.rs`).
    pivot_offset: Vec3,
    camera_target: Option<CameraTarget>,
    idle_seconds: f32,
    /// The turntable's slow showcase rotation (E2): accumulates while the garage idles, so a
    /// walked-away-from hall keeps presenting its vehicle; reset when a new vehicle rolls in.
    idle_turntable_yaw: f32,
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
    /// Pre-battle map choice. `None` = AUTO: keep the environment/default behaviour
    /// (`RandomBattleConfig::runtime_from_env`), which is also what the editor's Ctrl+P
    /// playtest relies on (`WOT_MAP` set to a `.map.ron` path). `Some` overrides the
    /// battle's map with a shipped catalog entry.
    selected_map: Option<terrain::MapId>,
    /// What the player's own clock says the hall's daylight is (H1). Set by the app from the
    /// LOCAL time at startup and on each garage open; defaults to the canonical Day, so tests
    /// and offscreen renders never depend on when they run.
    auto_daylight: scene_build::hangar::HangarLight,
    /// The player's manual daylight choice (`L` cycles Auto → Morning → Day → Evening),
    /// persisted with the garage save. `None` = follow the clock.
    daylight_override: Option<scene_build::hangar::HangarLight>,
    /// The armor inspector (I1): `I` toggles the translucent armor-volume overlay on the
    /// parked hero. Session-local — inspection is a moment, not a preference.
    inspector: bool,
    /// Field dust on the hero (J2), 0 clean .. 1 fresh off the battlefield. Set on every
    /// return from a running battle, settles slowly while the hall stands, and a freshly
    /// selected vehicle rolls in clean — it was not the one that fought.
    dust: f32,
    /// The battle state the parked hero still wears (L1): damage masks, thrown belt, hit
    /// decals — captured at the same return seam as the dust. `None` = clean machine.
    wear: Option<wear::FieldWear>,
    /// The repair beat's elapsed seconds while one plays (L2); `None` = shop idle.
    repair: Option<f32>,
}

impl Default for GarageState {
    fn default() -> Self {
        Self {
            open: true,
            started: false,
            selected_index: 0,
            carousel_scroll: 0,
            draft: LoadoutDraft::for_vehicle(VehicleKind::PLAYABLE[0]),
            saved: HashMap::new(),
            foreign_loadouts: HashMap::new(),
            save_path: None,
            orbit_yaw: HERO_ORBIT_YAW,
            orbit_pitch: HERO_ORBIT_PITCH,
            orbit_distance: HERO_ORBIT_DISTANCE,
            pivot_offset: Vec3::ZERO,
            camera_target: None,
            idle_seconds: 0.0,
            idle_turntable_yaw: 0.0,
            drive_in: drive_in::DriveIn::default(),
            cursor_clip: [2.0, 2.0],
            dragging: false,
            rejected_slot: None,
            focused_slot: FitSlot::Gun,
            option_list: None,
            view: GarageView::Hangar,
            selected_map: None,
            auto_daylight: scene_build::hangar::HangarLight::Day,
            daylight_override: None,
            inspector: false,
            dust: 0.0,
            wear: None,
            repair: None,
        }
    }
}

impl GarageState {
    /// The hall's daylight this frame (H1): the manual override when one is set, the
    /// player's clock otherwise.
    pub(in crate::app) fn hangar_light(&self) -> scene_build::hangar::HangarLight {
        self.daylight_override.unwrap_or(self.auto_daylight)
    }

    /// The clock's verdict, fed in by the app (the state itself never reads the wall clock —
    /// tests and offscreen renders stay deterministic).
    pub(in crate::app) fn set_auto_daylight(&mut self, light: scene_build::hangar::HangarLight) {
        self.auto_daylight = light;
    }

    /// The hero came back from the field (J2): the hull wears the battle's dust.
    pub(in crate::app) fn dust_from_the_field(&mut self) {
        self.dust = 1.0;
    }

    /// Field dust this frame (J2), for `set_vehicle_dust`.
    pub(in crate::app) fn hangar_dust(&self) -> f32 {
        self.dust
    }

    /// The dust settles while the hall stands: clean again after ~two quiet minutes.
    pub(in crate::app) fn tick_dust(&mut self, dt: f32) {
        self.dust = (self.dust - dt / 120.0).max(0.0);
    }

    /// `I` in the hangar: the armor inspector on or off (I1).
    pub(in crate::app) fn toggle_inspector(&mut self) {
        self.inspector = !self.inspector;
    }

    pub(in crate::app) fn inspector_on(&self) -> bool {
        self.inspector
    }

    /// `L` in the hangar: cycle Auto → Morning → Day → Evening → Auto, persisted.
    pub(in crate::app) fn cycle_daylight(&mut self) {
        use scene_build::hangar::HangarLight;
        self.daylight_override = match self.daylight_override {
            None => Some(HangarLight::Morning),
            Some(HangarLight::Morning) => Some(HangarLight::Day),
            Some(HangarLight::Day) => Some(HangarLight::Evening),
            Some(HangarLight::Evening) => None,
        };
        self.persist();
    }
}

/// The hall's daylight for an hour on the player's own clock (H1). Pure and locked, so the
/// mapping is a fact and not a vibe: early hours wake the hall, the working day is the
/// canonical light, and everything from late afternoon through the night is the dusk hall —
/// a true night variant is future work, and lamps-forward evening reads closest until then.
pub(in crate::app) fn daylight_for_hour(hour: u32) -> scene_build::hangar::HangarLight {
    use scene_build::hangar::HangarLight;
    match hour {
        5..=10 => HangarLight::Morning,
        11..=16 => HangarLight::Day,
        _ => HangarLight::Evening,
    }
}

/// [`daylight_for_hour`] on the actual local clock — the one seam where the wall clock
/// enters the game.
pub(in crate::app) fn daylight_for_local_clock() -> scene_build::hangar::HangarLight {
    use chrono::Timelike;
    daylight_for_hour(chrono::Local::now().hour())
}

/// Build the garage HUD overlay for an offscreen review render (`tech_tree` picks the view).
/// The armor inspector's mm legend alone (Hala v4 R1) — what the review harness hangs over
/// the inspector golden, so the locked frame carries the ramp's unit exactly the way the
/// live screen does.
pub fn garage_inspector_legend(aspect: f32) -> Vec<renderer_api::HudVertex> {
    let mut v = Vec::new();
    panels::inspector_legend::draw(&mut v, aspect);
    v
}

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

    pub(super) fn selected_map(&self) -> Option<terrain::MapId> {
        self.selected_map
    }

    /// Cycle the pre-battle map choice: AUTO → each shipped map ([`terrain::MapId::SHIPPED`]) →
    /// back to AUTO. AUTO keeps the env/default map resolution, so the choice ring always
    /// has `ALL.len() + 1` stops and never lands on `Scratch`.
    pub(super) fn cycle_map(&mut self, dir: i8) {
        let ring = terrain::MapId::SHIPPED.len() as isize + 1;
        let current = match self.selected_map {
            None => 0,
            Some(map) => terrain::MapId::SHIPPED
                .iter()
                .position(|&id| id == map)
                .map_or(0, |i| i as isize + 1),
        };
        let next = (current + dir as isize).rem_euclid(ring);
        self.selected_map =
            if next == 0 { None } else { Some(terrain::MapId::SHIPPED[next as usize - 1]) };
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

    /// Test hook: force the fitted turret's caliber limit under the alternate gun so cycling
    /// rejects — exercises the rejection feedback path without inventing an incompatible catalog.
    #[cfg(test)]
    pub(super) fn force_turret_caliber_limit_for_test(&mut self, max_mm: f32) {
        self.draft.force_turret_caliber_limit_for_test(max_mm);
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
