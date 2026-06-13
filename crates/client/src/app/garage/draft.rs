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

    fn index(self) -> usize {
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

    /// Advance the chosen option for `slot`. Incompatible installs (caliber / load limit) are
    /// silently skipped, leaving the slot on its current module.
    pub(super) fn cycle_module(&mut self, slot: FitSlot, dir: isize) {
        let len = self.options_len(slot);
        if len <= 1 {
            return;
        }
        let current = self.option_index[slot.index()];
        let next = (current as isize + dir).rem_euclid(len as isize) as usize;
        let installed = match slot {
            FitSlot::Turret => self.modules.try_install_turret(self.kind.turret_options()[next].clone()).is_ok(),
            FitSlot::Gun => {
                let ok = self.modules.try_install_gun(self.kind.gun_options()[next].clone()).is_ok();
                if ok {
                    // A new gun has its own ammo list; keep the selection in range.
                    self.ammo_index = self.ammo_index.min(self.ammo_options().len().saturating_sub(1));
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
        }
    }

    pub(super) fn ammo_options(&self) -> Vec<ShellSpec> {
        self.modules.gun.spec.ammo_options()
    }

    /// Exposed barrel length (m) of the installed gun — drives the garage gun silhouette.
    pub(super) fn gun_barrel_length(&self) -> f32 {
        self.modules.gun.barrel_length_m()
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
        let ammo = self.ammo_options();
        spec.gun.shell = ammo[self.ammo_index.min(ammo.len() - 1)];
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
    fn selecting_apcr_changes_the_fired_shell_type() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerII);
        let stock = draft.assembled_spec().gun.shell.shell_type;
        draft.set_ammo(1);
        let chosen = draft.assembled_spec().gun.shell.shell_type;
        assert_ne!(chosen, stock);
        assert_eq!(chosen, game_core::ShellType::Apcr);
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
}
