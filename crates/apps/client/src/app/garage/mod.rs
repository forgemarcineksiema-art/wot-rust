mod actions;
mod camera;
mod chrome;
mod draft;
mod drive_in;
mod elements;
mod filter;
mod hero_pick;
mod hints;
mod layout;
mod overlay;
pub(crate) mod persistence;
mod screen;
mod selection;
mod silhouette;
#[cfg(test)]
mod state_tests;
mod stats;
mod tree;
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
pub(super) use self::types::{GarageHit, GarageTab, GarageView};
use ui_kit::draw_list::DrawList;

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
    /// The interaction machine (G6): hover, press and the tooltip clock, over the screen's own
    /// draw list — a click is the release on what was pressed.
    interaction: ui_kit::interaction::Interaction<elements::GarageElement>,
    /// What a press on the scene took (G8): the camera, the turret, or nothing.
    drag: types::Drag,
    /// How far the press travelled: a press that never travels is a click on the hero.
    drag_travel_px: f32,
    /// The module under a press on the hero, until the release decides click or drag.
    hero_press: Option<FitSlot>,
    /// The turret's traverse on the parked hero (G8), hull-relative; a fresh hull parks at zero.
    hero_turret_yaw: f32,
    /// The carousel's chips (G9), persisted with the save.
    filter: filter::CarouselFilter,
    /// The hull the VEHICLE column is compared against (G3); a session's, never saved.
    compare: Option<VehicleKind>,
    /// The table's keys as the legend and the tooltips print them (G6), fed by the app.
    key_labels: hints::KeyLabels,
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
    /// How long the mechanic's round clock has been PAUSED across repair beats (R3): during
    /// a beat he steps toward the ring instead of walking his round, and the paused clock is
    /// what lets him resume the round exactly where the beat interrupted it — no snap.
    mechanic_pause_s: f32,
    /// The viewport the screen lays itself out in and the player's interface scale (G1): the
    /// app tells the garage every frame; the reference 1080p until it does.
    viewport_px: [f32; 2],
    ui_scale: f32,
    /// G14: seconds left in the battle the hull is still locked in; `None` when it is free.
    locked_remaining_s: Option<f32>,
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
            interaction: ui_kit::interaction::Interaction::default(),
            drag: types::Drag::None,
            drag_travel_px: 0.0,
            hero_press: None,
            hero_turret_yaw: 0.0,
            filter: filter::CarouselFilter::default(),
            compare: None,
            key_labels: hints::KeyLabels::default(),
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
            mechanic_pause_s: 0.0,
            viewport_px: [1920.0, 1080.0],
            ui_scale: 1.0,
            locked_remaining_s: None,
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

    /// The ARMOUR tab's word (G10): the inspector on or off, said rather than toggled.
    pub(in crate::app) fn set_inspector(&mut self, on: bool) {
        self.inspector = on;
    }

    /// The tab the garage's own screen is on (G10): GARAGE, TECH TREE or ARMOUR — the shell's
    /// pages cover the screen under their own title.
    pub(in crate::app) fn active_tab(&self) -> types::GarageTab {
        match self.view {
            GarageView::TechTree => types::GarageTab::TechTree,
            GarageView::Hangar if self.inspector => types::GarageTab::Armour,
            GarageView::Hangar => types::GarageTab::Garage,
        }
    }

    pub(in crate::app) fn daylight_override(&self) -> Option<scene_build::hangar::HangarLight> {
        self.daylight_override
    }

    /// The settings file's word (P6): the daylight lives there now; the garage's file keeps
    /// its copy for older builds.
    pub(in crate::app) fn set_daylight_override(
        &mut self,
        light: Option<scene_build::hangar::HangarLight>,
    ) {
        self.daylight_override = light;
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
    let mut state = GarageState::default();
    state.toggle_inspector();
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    state.set_viewport(ui.viewport().w as u32, ui.viewport().h as u32, 1.0);
    let mut list = screen::build_screen_list(&state, &ui, None);
    list.retain(|id| {
        matches!(
            id,
            elements::GarageElement::LegendPlate
                | elements::GarageElement::LegendTitle
                | elements::GarageElement::LegendSwatch(_)
                | elements::GarageElement::LegendLabel(_)
                | elements::GarageElement::LegendUnit
        )
    });
    list.emit(&ui, &ui_kit::theme::Theme::standard())
}

pub fn garage_overlay(tech_tree: bool, aspect: f32) -> Vec<renderer_api::HudVertex> {
    let mut state = GarageState::default();
    if tech_tree {
        state.open_tech_tree();
    }
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    state.set_viewport(ui.viewport().w as u32, ui.viewport().h as u32, 1.0);
    state.overlay_vertices(&ui, &ui_kit::theme::Theme::standard())
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
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    state.set_viewport(ui.viewport().w as u32, ui.viewport().h as u32, 1.0);
    state.overlay_vertices(&ui, &ui_kit::theme::Theme::standard())
}

/// The hangar with the VEHICLE column compared against a second hull (G3) — the review golden
/// `garage_compare`. `vehicle_index` and `other_index` are `VehicleKind::PLAYABLE` slots; out of
/// range clamps, and an `other` equal to the selection takes the next hull.
pub fn garage_overlay_compare(
    vehicle_index: usize,
    other_index: usize,
    aspect: f32,
) -> Vec<renderer_api::HudVertex> {
    let last = game_core::VehicleKind::PLAYABLE.len() - 1;
    let mut state = GarageState::default();
    state.select_index(vehicle_index.min(last));
    let mut other = other_index.min(last);
    if other == state.selected_index() {
        other = (other + 1) % (last + 1);
    }
    state.toggle_compare(other);
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    state.set_viewport(ui.viewport().w as u32, ui.viewport().h as u32, 1.0);
    state.overlay_vertices(&ui, &ui_kit::theme::Theme::standard())
}

/// The hangar with the armour inspector on (G10's ARMOUR tab) — the review golden
/// `garage_armour`: the legend, the tab in the lamp.
pub fn garage_overlay_armour(aspect: f32) -> Vec<renderer_api::HudVertex> {
    let mut state = GarageState::default();
    state.set_inspector(true);
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    state.set_viewport(ui.viewport().w as u32, ui.viewport().h as u32, 1.0);
    state.overlay_vertices(&ui, &ui_kit::theme::Theme::standard())
}

/// The hangar with one of the shell's pages over it (G10's BATTLES, REPLAYS, STATISTICS and
/// SETTINGS tabs), staged the way the HUD's review states stage the same pages — the review
/// goldens `garage_battles`, `garage_replays`, `garage_statistics`, `garage_settings`.
pub fn garage_overlay_page(
    screen: scene_build::review_views::GarageScreen,
    aspect: f32,
) -> Vec<renderer_api::HudVertex> {
    use scene_build::review_views::GarageScreen;
    let model = match screen {
        GarageScreen::Battles => crate::hud::shell::demo_battles_screen(),
        GarageScreen::Replays => crate::hud::shell::demo_replays_screen(),
        GarageScreen::Statistics => crate::hud::shell::demo_statistics_screen(),
        _ => crate::hud::shell::demo_settings_screen(),
    };
    let ui = ui_kit::ui::Ui::for_aspect(aspect);
    let theme = ui_kit::theme::Theme::standard();
    let mut vertices = garage_overlay(false, aspect);
    let mut list = ui_kit::draw_list::DrawList::new();
    let mut order: i16 = 0;
    crate::hud::shell::push_shell(&mut list, &ui, &theme, &model, &mut order);
    vertices.extend(list.emit(&ui, &theme));
    vertices
}

impl GarageState {
    /// Closes the garage regardless of `started`: the battle-mode tests need the battle.
    #[cfg(test)]
    pub(super) fn close_for_test(&mut self) {
        self.open = false;
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn has_started(&self) -> bool {
        self.started
    }

    pub(super) fn open(&mut self) {
        self.open = true;
        self.drag = types::Drag::None;
        self.interaction.clear();
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
        self.drag != types::Drag::None
    }

    /// Test hook: force the fitted turret's caliber limit under the alternate gun so cycling
    /// rejects — exercises the rejection feedback path without inventing an incompatible catalog.
    #[cfg(test)]
    pub(super) fn force_turret_caliber_limit_for_test(&mut self, max_mm: f32) {
        self.draft.force_turret_caliber_limit_for_test(max_mm);
    }

    pub(super) fn open_tech_tree(&mut self) {
        self.view = GarageView::TechTree;
        self.drag = types::Drag::None;
        self.option_list = None;
    }

    pub(super) fn close_tech_tree(&mut self) {
        self.view = GarageView::Hangar;
        self.drag = types::Drag::None;
    }

    /// Commit the edited loadout: lock the garage and hand back the assembled spec to install.
    pub(super) fn confirm(&mut self) -> TankSpec {
        self.locked_remaining_s = None;
        self.started = true;
        self.open = false;
        self.drag = types::Drag::None;
        self.interaction.clear();
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

    pub(super) fn rejected_slot(&self) -> Option<FitSlot> {
        self.rejected_slot
    }

    // --- G9: the chips and the roster they let through -----------------------------------

    pub(super) fn filter(&self) -> &filter::CarouselFilter {
        &self.filter
    }

    /// The hulls the chips let through, in the roster's order.
    pub(super) fn roster(&self) -> Vec<VehicleKind> {
        self.filter.roster()
    }

    /// Walk a chip's ring; the carousel follows, the choice persists.
    pub(super) fn cycle_chip(&mut self, chip: filter::Chip, dir: i8) {
        self.filter.cycle(chip, dir);
        self.scroll_selection_into_view();
        self.persist();
    }

    // --- G3: the compared hull --------------------------------------------------------------

    pub(super) fn compare(&self) -> Option<VehicleKind> {
        self.compare
    }

    /// Shift-click on a carousel cell: compare against that hull; the same cell again clears
    /// it, and the hull on the turntable compares to nothing.
    pub(super) fn toggle_compare(&mut self, index: usize) {
        let Some(kind) = VehicleKind::PLAYABLE.get(index).copied() else { return };
        self.compare = if kind == self.selected_vehicle() || self.compare == Some(kind) {
            None
        } else {
            Some(kind)
        };
    }

    /// The compared hull as it would deploy: its saved loadout when it has one, stock otherwise.
    pub(super) fn compare_spec(&self) -> Option<TankSpec> {
        let kind = self.compare?;
        Some(match self.saved.get(&kind) {
            Some(saved) => LoadoutDraft::from_saved(kind, saved).assembled_spec(),
            None => LoadoutDraft::for_vehicle(kind).assembled_spec(),
        })
    }

    // --- G6: the table's keys and the interaction machine -----------------------------------

    pub(super) fn key_labels(&self) -> &hints::KeyLabels {
        &self.key_labels
    }

    /// The table's keys, from the app, every garage frame — so a rebinding shows at once.
    pub(in crate::app) fn set_key_labels(&mut self, labels: hints::KeyLabels) {
        if self.key_labels != labels {
            self.key_labels = labels;
        }
    }

    pub(super) fn set_focused_slot(&mut self, slot: FitSlot) {
        self.focused_slot = slot;
    }

    /// The element the button is down on, if any.
    pub(super) fn pressed_key(&self) -> Option<elements::GarageElement> {
        self.interaction.pressed()
    }

    /// The element whose tooltip is due: hovered for the delay, not pressed.
    pub(super) fn tooltip_key(&self) -> Option<elements::GarageElement> {
        self.interaction.tooltip()
    }

    /// The machine sees the cursor over the screen as it is drawn NOW — once per frame from
    /// the tick, and at the press and the release, never per mouse event (the screen is a
    /// full layout, and a polling mouse would build it a thousand times a second).
    fn sync_interaction(&mut self) -> DrawList<elements::GarageElement> {
        let ui = self.ui();
        let list = screen::build_screen_list(self, &ui, None);
        let px = self.cursor_px(&ui);
        self.interaction.on_cursor(px, &list);
        list
    }

    /// The primary button went down over the screen.
    pub(super) fn press_cursor(&mut self) {
        let list = self.sync_interaction();
        self.interaction.on_press(&list);
    }

    /// The primary button came up: the click, if the cursor is still on what it pressed.
    pub(super) fn release_cursor(&mut self) -> Option<elements::GarageElement> {
        let list = self.sync_interaction();
        self.interaction.on_release(&list)
    }

    /// The screen changed under the press (a modal list closed on it): no release fires it.
    pub(super) fn cancel_press(&mut self) {
        self.interaction.cancel_press();
    }

    /// The tooltip clock, every garage frame, over where the cursor rests now.
    pub(in crate::app) fn tick_interaction(&mut self, dt: f32) {
        self.sync_interaction();
        self.interaction.tick(dt);
    }

    /// The screen as vertices, laid out in `ui` (the app's viewport at the player's scale).
    pub(super) fn overlay_vertices(
        &self,
        ui: &ui_kit::ui::Ui,
        theme: &ui_kit::theme::Theme,
    ) -> Vec<renderer_api::HudVertex> {
        if !self.open {
            return Vec::new();
        }
        overlay::build(self, ui, theme)
    }

    /// What the cursor is on, from the rectangles the screen draws.
    pub(super) fn hit_test(&self, shift: bool) -> GarageHit {
        overlay::hit_test(self, shift)
    }
}
