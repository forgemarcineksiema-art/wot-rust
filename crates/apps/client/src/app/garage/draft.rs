//! The working loadout the player edits in the garage before committing to battle. It tracks the
//! installed modules, the selected ammo, and the crew, and assembles them into the live
//! [`TankSpec`] the stat panel previews and the battle uses. No economy — every option is freely
//! selectable; compatibility (gun caliber, load limit) is still enforced via `try_install_*`.

use game_core::{Crew, ShellSpec, TankSpec, VehicleKind, VehicleModules};

/// The fitting slots the garage exposes. The hull is the chassis and the suspension stays stock
/// (it gates the load limit); the rest mirror WoT's module slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FitSlot {
    Turret,
    Gun,
    Hull,
    Engine,
    Suspension,
    Radio,
}

impl FitSlot {
    pub(super) const ALL: [FitSlot; 6] = [
        FitSlot::Turret,
        FitSlot::Gun,
        FitSlot::Hull,
        FitSlot::Engine,
        FitSlot::Suspension,
        FitSlot::Radio,
    ];

    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|slot| *slot == self).expect("slot is in ALL")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoadoutDraft {
    kind: VehicleKind,
    modules: VehicleModules,
    option_index: [usize; 6],
    ammo_index: usize,
    crew: Crew,
}

impl LoadoutDraft {
    pub(super) fn for_vehicle(kind: VehicleKind) -> Self {
        Self {
            kind,
            modules: kind.default_loadout(),
            option_index: [0; 6],
            ammo_index: 0,
            crew: Crew::default(),
        }
    }

    pub(super) fn crew(&self) -> Crew {
        self.crew
    }

    pub(super) fn options_len(&self, slot: FitSlot) -> usize {
        match slot {
            FitSlot::Turret => self.kind.turret_options().len(),
            FitSlot::Gun => self.kind.gun_options().len(),
            FitSlot::Hull => self.kind.hull_options().len(),
            FitSlot::Engine => self.kind.engine_options().len(),
            FitSlot::Suspension => self.kind.suspension_options().len(),
            FitSlot::Radio => self.kind.radio_options().len(),
        }
    }

    pub(super) fn has_choice(&self, slot: FitSlot) -> bool {
        self.options_len(slot) > 1
    }

    /// Advance the chosen option for `slot`. Returns `true` if the option changed (or there was
    /// only one option so nothing to cycle), `false` if the install was rejected by compatibility
    /// (gun caliber exceeds turret limit, or turret overloads suspension).
    pub(super) fn cycle_module(&mut self, slot: FitSlot, dir: isize) -> bool {
        let len = self.options_len(slot);
        if len <= 1 {
            return true;
        }
        let current = self.option_index[slot.index()];
        let next = (current as isize + dir).rem_euclid(len as isize) as usize;
        let installed = match slot {
            FitSlot::Turret => {
                self.modules.try_install_turret(self.kind.turret_options()[next].clone()).is_ok()
            }
            FitSlot::Gun => {
                let ok =
                    self.modules.try_install_gun(self.kind.gun_options()[next].clone()).is_ok();
                if ok {
                    // A new gun has its own ammo list; keep the selection in range.
                    self.ammo_index =
                        self.ammo_index.min(self.ammo_options().len().saturating_sub(1));
                }
                ok
            }
            FitSlot::Hull => {
                self.modules.hull = self.kind.hull_options()[next].clone();
                true
            }
            FitSlot::Engine => {
                self.modules.engine = self.kind.engine_options()[next].clone();
                true
            }
            FitSlot::Suspension => {
                self.modules.suspension = self.kind.suspension_options()[next].clone();
                true
            }
            FitSlot::Radio => {
                self.modules.radio = self.kind.radio_options()[next].clone();
                true
            }
        };
        if installed {
            self.option_index[slot.index()] = next;
            true
        } else {
            false
        }
    }

    pub(super) fn ammo_options(&self) -> Vec<ShellSpec> {
        self.modules.gun.spec.ammo_options()
    }

    /// Exposed barrel length (m) of the installed gun — drives the garage gun silhouette.
    pub(super) fn gun_barrel_length(&self) -> f32 {
        self.modules.gun.barrel_length_m()
    }

    /// One-line key stat of the installed module, short enough to fit inside a loadout slot.
    pub(super) fn current_module_summary(&self, slot: FitSlot) -> String {
        match slot {
            FitSlot::Turret => format!("{}mm", self.modules.turret.front_mm.round() as i32),
            FitSlot::Gun => format!("{}mm", self.modules.gun.caliber_mm().round() as i32),
            FitSlot::Hull => format!("{}", self.modules.hull.hit_points),
            FitSlot::Engine => format!("{}kW", self.modules.engine.power_kw.round() as i32),
            FitSlot::Suspension => {
                format!(
                    "{}d/s",
                    self.modules.suspension.turn_rate_rad_s.to_degrees().round() as i32
                )
            }
            FitSlot::Radio => format!("{}m", self.modules.radio.signal_range_m.round() as i32),
        }
    }

    pub(super) fn ammo_index(&self) -> usize {
        self.ammo_index
    }

    pub(super) fn set_ammo(&mut self, index: usize) {
        if index < self.ammo_options().len() {
            self.ammo_index = index;
        }
    }

    pub(super) fn adjust_proficiency(&mut self, dir: isize) {
        self.crew = Crew::new(self.crew.proficiency() + 0.05 * dir as f32);
    }

    /// Compose modules + crew + selected ammo into the live spec the stat panel previews and the
    /// battle installs.
    pub(super) fn assembled_spec(&self) -> TankSpec {
        let mut spec = self.modules.assemble(self.kind);
        self.crew.apply(&mut spec);
        let selected = self.ammo_index.min(self.ammo_options().len() - 1);
        // The rack carries the default fill with the garage-chosen slot pre-loaded (per-slot
        // count editing is a follow-up). The sim fires `TankState::selected_shell()` and the
        // reticle reads the predictor's selected shell — `gun.shell` stays the stock round.
        spec.ammo = game_core::AmmoLoadout {
            initial_selected: selected as u8,
            ..game_core::AmmoLoadout::default_for(spec.ammo_capacity)
        };
        spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_vehicle_starts_from_the_stock_loadout() {
        let draft = LoadoutDraft::for_vehicle(VehicleKind::T55A);
        assert_eq!(draft.assembled_spec().gun.shell, VehicleKind::T55A.spec().gun.shell);
    }

    #[test]
    fn cycling_the_gun_changes_the_assembled_reload() {
        // The T-54 has two real gun options, so cycling must move a stat.
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        assert!(draft.has_choice(FitSlot::Gun));
        let before = draft.assembled_spec().gun.reload_seconds;
        draft.cycle_module(FitSlot::Gun, 1);
        assert_ne!(draft.assembled_spec().gun.reload_seconds, before);
    }

    #[test]
    fn cycling_the_suspension_changes_assembled_turn_rate() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerII);
        let before = draft.assembled_spec().turn_rate_rad_s;
        draft.cycle_module(FitSlot::Suspension, 1);
        assert!(draft.assembled_spec().turn_rate_rad_s > before);
    }

    #[test]
    fn swapping_the_t54_gun_changes_the_barrel_length() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let before = draft.gun_barrel_length();
        draft.cycle_module(FitSlot::Gun, 1);
        assert_ne!(draft.gun_barrel_length(), before, "the alternate gun has a different barrel");
    }

    #[test]
    fn cycling_the_engine_changes_assembled_power() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerII);
        let before = draft.assembled_spec().engine_power_kw;
        draft.cycle_module(FitSlot::Engine, 1);
        assert!(draft.assembled_spec().engine_power_kw > before);
    }

    #[test]
    fn selecting_apcr_preloads_that_slot_without_touching_the_stock_shell() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerII);
        assert_eq!(draft.assembled_spec().ammo.initial_selected, 0);
        draft.set_ammo(1);
        let spec = draft.assembled_spec();
        assert_eq!(spec.ammo.initial_selected, 1, "the chosen slot spawns loaded");
        // gun.shell stays the stock round: the sim fires selected_shell() and the reticle reads
        // the predictor's selection — nothing bakes the choice into the spec anymore.
        assert_eq!(spec.gun.shell.shell_type, game_core::ShellType::ArmorPiercing);
        assert!(spec.ammo.total() > 0 && spec.ammo.total() <= spec.ammo_capacity);
    }

    #[test]
    fn higher_proficiency_lowers_reload() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerI);
        let mid = draft.assembled_spec().gun.reload_seconds;
        draft.adjust_proficiency(-1);
        assert!(draft.assembled_spec().gun.reload_seconds >= mid, "greener crew reloads slower");
        draft.adjust_proficiency(1);
        draft.adjust_proficiency(1);
        assert!(draft.assembled_spec().gun.reload_seconds <= mid, "better crew reloads faster");
    }

    #[test]
    fn current_module_summary_is_compact_and_changes_on_swap() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let before = draft.current_module_summary(FitSlot::Gun);
        assert!(!before.is_empty());
        let eng_before = draft.current_module_summary(FitSlot::Engine);
        draft.cycle_module(FitSlot::Engine, 1);
        let eng_after = draft.current_module_summary(FitSlot::Engine);
        assert_ne!(eng_before, eng_after, "uprated engine shows a different kW value");
    }

    #[test]
    fn cycle_module_returns_true_for_successful_install() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        assert!(draft.cycle_module(FitSlot::Gun, 1), "gun swap should succeed");
        assert!(draft.cycle_module(FitSlot::Engine, 1), "engine swap should succeed");
    }

    #[test]
    fn cycle_module_returns_false_when_gun_exceeds_turret_caliber() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        draft.modules.turret.max_gun_caliber_mm = 99.0;
        assert!(
            !draft.cycle_module(FitSlot::Gun, 1),
            "gun should be rejected when caliber exceeds turret limit"
        );
        assert_eq!(
            draft.option_index[FitSlot::Gun.index()],
            0,
            "rejected cycle must not advance the option index"
        );
    }
}
