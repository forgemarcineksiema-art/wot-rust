//! The working loadout the player edits in the garage before committing to battle. It tracks the
//! installed modules, the selected ammo, and the crew, and assembles them into the live
//! [`TankSpec`] the stat panel previews and the battle uses. No economy — every option is freely
//! selectable; compatibility (gun caliber, load limit) is still enforced via `try_install_*`.

use game_core::{Crew, MAX_AMMO_SLOTS, ShellSpec, TankSpec, VehicleKind, VehicleModules};

use super::persistence::SavedLoadout;

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

/// One selectable option for a fitting slot, with the single headline stat the garage option list
/// compares. The stat is read straight off the catalogue module (no crew scaling), so two options
/// compare on the module's own merit and the delta between them is stable.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleOption {
    pub name: String,
    pub stat: f32,
    pub unit: &'static str,
    /// Whether a higher `stat` is the better outcome (drives the green/red delta colour).
    pub higher_is_better: bool,
    /// Whether this is the option currently fitted.
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoadoutDraft {
    kind: VehicleKind,
    modules: VehicleModules,
    option_index: [usize; 6],
    ammo_index: usize,
    /// Rounds per rack slot — the honest-ammo pillar's editable half. Constrained to the
    /// vehicle's authored capacity and at least one round total.
    ammo_counts: [u16; MAX_AMMO_SLOTS],
    crew: Crew,
}

impl LoadoutDraft {
    pub(super) fn for_vehicle(kind: VehicleKind) -> Self {
        let modules = kind.default_loadout();
        // The fill is shaped by the stock gun's REAL slot count: a gun that never fielded a
        // special round (the IS-3's D-25T, the Jagdtiger's Pak 80) has no slot to hide a quarter
        // of the rack in, so those rounds go to the rounds it does carry.
        let slots = modules.gun.spec.ammo_options().len();
        Self {
            kind,
            option_index: [0; 6],
            ammo_index: 0,
            ammo_counts: game_core::AmmoLoadout::default_for_slots(kind.ammo_capacity(), slots)
                .counts,
            modules,
            crew: Crew::default(),
        }
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
        self.try_install_index(slot, next)
    }

    /// Install a specific option index for `slot`, updating `option_index` only if the fit is
    /// accepted by compatibility. Returns `true` on success. Shared by keyboard/click cycling and
    /// saved-loadout restore.
    fn try_install_index(&mut self, slot: FitSlot, index: usize) -> bool {
        let installed = match slot {
            FitSlot::Turret => {
                self.modules.try_install_turret(self.kind.turret_options()[index].clone()).is_ok()
            }
            FitSlot::Gun => {
                let old_options = self.ammo_options();
                let ok =
                    self.modules.try_install_gun(self.kind.gun_options()[index].clone()).is_ok();
                if ok {
                    self.remap_ammo_to_new_gun(&old_options);
                }
                ok
            }
            FitSlot::Hull => {
                self.modules.hull = self.kind.hull_options()[index].clone();
                true
            }
            FitSlot::Engine => {
                self.modules.engine = self.kind.engine_options()[index].clone();
                true
            }
            FitSlot::Suspension => {
                self.modules.suspension = self.kind.suspension_options()[index].clone();
                true
            }
            FitSlot::Radio => {
                self.modules.radio = self.kind.radio_options()[index].clone();
                true
            }
        };
        if installed {
            self.option_index[slot.index()] = index;
        }
        installed
    }

    /// A new gun has its own ammo list; the rack follows each round's SHELL TYPE across the swap,
    /// not its slot index. Rounds of a type the new gun cannot chamber pour into the stock slot —
    /// they never vanish and never impersonate another round — and the selection likewise re-finds
    /// its type or falls back to stock. The total is conserved, so the rack stays within the
    /// vehicle's capacity.
    fn remap_ammo_to_new_gun(&mut self, old_options: &[ShellSpec]) {
        let new_options = self.ammo_options();
        let selected_type = old_options.get(self.ammo_index).map(|shell| shell.shell_type);
        let mut counts = [0u16; MAX_AMMO_SLOTS];
        for (slot, &count) in self.ammo_counts.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let target = old_options
                .get(slot)
                .and_then(|shell| {
                    new_options.iter().position(|new| new.shell_type == shell.shell_type)
                })
                .unwrap_or(0);
            counts[target] += count;
        }
        self.ammo_counts = counts;
        self.ammo_index = selected_type
            .and_then(|ty| new_options.iter().position(|new| new.shell_type == ty))
            .unwrap_or(0);
    }

    /// The persistable snapshot of the current choices.
    pub(super) fn to_saved(&self) -> SavedLoadout {
        SavedLoadout {
            option_index: self.option_index,
            ammo_index: self.ammo_index,
            ammo_counts: Some(self.ammo_counts),
            crew_proficiency: self.crew.proficiency(),
        }
    }

    /// Rebuild a draft from a persisted snapshot. Each option is applied through the checked
    /// install path in `FitSlot::ALL` order (turret before gun, so the caliber limit is set first)
    /// — an out-of-range or now-incompatible index silently stays at stock instead of panicking.
    pub(super) fn from_saved(kind: VehicleKind, saved: &SavedLoadout) -> Self {
        let mut draft = Self::for_vehicle(kind);
        for slot in FitSlot::ALL {
            let target = saved.option_index[slot.index()];
            if target != 0 && target < draft.options_len(slot) {
                draft.try_install_index(slot, target);
            }
        }
        draft.set_ammo(saved.ammo_index);
        // A stored rack fill applies only if it still fits this build's authored capacity (a
        // rebalance may have shrunk the rack), is not empty, and puts no rounds in a slot the
        // restored gun does not have; anything stale degrades to the stock-heavy default instead
        // of an invalid fill. The sum runs in u32: the file is user-editable, and three u16::MAX
        // entries must degrade here, not overflow.
        if let Some(counts) = saved.ammo_counts {
            let total: u32 = counts.iter().map(|&count| u32::from(count)).sum();
            let slots = draft.ammo_options().len();
            let phantom = counts.iter().skip(slots).any(|&count| count > 0);
            if total >= 1 && total <= u32::from(kind.ammo_capacity()) && !phantom {
                draft.ammo_counts = counts;
            }
        }
        draft.crew = Crew::new(saved.crew_proficiency);
        draft
    }

    pub(super) fn ammo_options(&self) -> Vec<ShellSpec> {
        self.modules.gun.spec.ammo_options()
    }

    /// Test hook: shrink the installed turret's gun-caliber limit so the next gun swap is
    /// guaranteed to be rejected by compatibility.
    #[cfg(test)]
    pub(super) fn force_turret_caliber_limit_for_test(&mut self, max_mm: f32) {
        self.modules.turret.max_gun_caliber_mm = max_mm;
    }

    /// Name of the installed gun — the identity a test should assert on. (Barrel LENGTH is not
    /// an identity: the T-54's two D-10 variants share one physical tube.)
    #[cfg(test)]
    pub(super) fn gun_name(&self) -> String {
        self.modules.gun.spec.name.clone()
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
            // Mass, not a fake "signal range": nothing in the battle reads a radio range, and
            // a stat printed where the player picks modules must be one the fight honours.
            FitSlot::Radio => format!("{}kg", self.modules.radio.mass_kg.round() as i32),
        }
    }

    /// The selectable options for `slot` with each one's name and headline stat, for the garage
    /// option list. The headline is the slot's characteristic number — turret/hull armour and HP,
    /// engine power, suspension traverse, radio range, and for the gun its reload (where two guns of
    /// equal calibre still differ). `installed` marks the fitted option.
    pub(super) fn module_options(&self, slot: FitSlot) -> Vec<ModuleOption> {
        let installed = self.option_index[slot.index()];
        let (unit, higher_is_better) = match slot {
            FitSlot::Turret => ("mm", true),
            FitSlot::Gun => ("s", false),
            FitSlot::Hull => ("HP", true),
            FitSlot::Engine => ("kW", true),
            FitSlot::Suspension => ("d/s", true),
            // A radio's honest headline is its mass (lighter is better) — see `RadioModule`.
            FitSlot::Radio => ("kg", false),
        };
        (0..self.options_len(slot))
            .map(|i| {
                let (name, stat) = self.option_name_stat(slot, i);
                ModuleOption { name, stat, unit, higher_is_better, installed: i == installed }
            })
            .collect()
    }

    /// The catalogue name and headline stat of option `index` in `slot` (raw module value, no crew).
    fn option_name_stat(&self, slot: FitSlot, index: usize) -> (String, f32) {
        match slot {
            FitSlot::Turret => {
                let m = &self.kind.turret_options()[index];
                (m.name.clone(), m.front_mm)
            }
            FitSlot::Gun => {
                let m = &self.kind.gun_options()[index];
                (m.spec.name.clone(), m.spec.reload_seconds)
            }
            FitSlot::Hull => {
                let m = &self.kind.hull_options()[index];
                (m.name.clone(), m.hit_points as f32)
            }
            FitSlot::Engine => {
                let m = &self.kind.engine_options()[index];
                (m.name.clone(), m.power_kw)
            }
            FitSlot::Suspension => {
                let m = &self.kind.suspension_options()[index];
                (m.name.clone(), m.turn_rate_rad_s.to_degrees())
            }
            FitSlot::Radio => {
                let m = &self.kind.radio_options()[index];
                (m.name.clone(), m.mass_kg)
            }
        }
    }

    /// Install a specific option index for `slot` — the option list's direct pick. Compatibility is
    /// checked through the same `try_install_index` path as cycling; returns whether it took.
    pub(super) fn set_option(&mut self, slot: FitSlot, index: usize) -> bool {
        if index >= self.options_len(slot) {
            return false;
        }
        // A new gun keeps the ammo selection in range (mirrors the gun arm of `try_install_index`).
        self.try_install_index(slot, index)
    }

    pub(super) fn ammo_index(&self) -> usize {
        self.ammo_index
    }

    pub(super) fn set_ammo(&mut self, index: usize) {
        if index < self.ammo_options().len() {
            self.ammo_index = index;
        }
    }

    pub(super) fn ammo_counts(&self) -> [u16; MAX_AMMO_SLOTS] {
        self.ammo_counts
    }

    /// The vehicle's authored rack capacity — the hard budget the counts edit inside.
    pub(super) fn rack_capacity(&self) -> u16 {
        self.kind.ammo_capacity()
    }

    pub(super) fn rack_total(&self) -> u16 {
        self.ammo_counts.iter().sum()
    }

    /// Move `delta` rounds into (`+`) or out of (`-`) rack slot `slot`, clamped to the honest
    /// bounds: a slot never goes negative, the rack never exceeds the vehicle's capacity, and at
    /// least one round stays aboard (an empty rack cannot fight). Returns whether anything moved
    /// — partial application (e.g. +5 into 2 free spaces) still counts as a change.
    pub(super) fn adjust_ammo_count(&mut self, slot: usize, delta: i32) -> bool {
        // A slot the fitted gun does not have takes no rounds — the bound lives here in the
        // data, not only in the hit test that currently never offers such a slot.
        if slot >= self.ammo_options().len() || delta == 0 {
            return false;
        }
        let count = i32::from(self.ammo_counts[slot]);
        let total = i32::from(self.rack_total());
        let capacity = i32::from(self.rack_capacity());
        let applied = if delta > 0 {
            delta.min(capacity - total)
        } else {
            // Floor at an empty slot AND at one round total across the rack.
            delta.max(-count).max(1 - total)
        };
        if applied == 0 {
            return false;
        }
        self.ammo_counts[slot] = (count + applied) as u16;
        true
    }

    /// Compose modules + crew + selected ammo into the live spec the stat panel previews and the
    /// battle installs.
    pub(super) fn assembled_spec(&self) -> TankSpec {
        let mut spec = self.modules.assemble(self.kind);
        self.crew.apply(&mut spec);
        let selected = self.ammo_index.min(self.ammo_options().len() - 1);
        // The rack carries the garage-edited per-slot fill with the chosen slot pre-loaded. The
        // sim fires `TankState::selected_shell()` and the reticle reads the predictor's selected
        // shell — `gun.shell` stays the stock round.
        spec.ammo =
            game_core::AmmoLoadout { counts: self.ammo_counts, initial_selected: selected as u8 };
        spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_vehicle_starts_from_the_stock_loadout() {
        let draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        assert_eq!(draft.assembled_spec().gun.shell, VehicleKind::T54_1951.spec().gun.shell);
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
    fn a_saved_loadout_round_trips_back_into_the_same_spec() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        draft.cycle_module(FitSlot::Gun, 1);
        draft.set_ammo(1);

        let restored = LoadoutDraft::from_saved(VehicleKind::T54_1951, &draft.to_saved());

        assert_eq!(
            restored.assembled_spec(),
            draft.assembled_spec(),
            "restore reproduces the spec"
        );
        assert_eq!(restored.ammo_index(), draft.ammo_index());
    }

    #[test]
    fn from_saved_degrades_a_stale_index_to_stock_without_panicking() {
        // A save from an older build (or a since-removed option) points every slot out of range.
        let stale = SavedLoadout {
            option_index: [999; 6],
            ammo_index: 999,
            ammo_counts: None,
            crew_proficiency: 0.8,
        };
        let draft = LoadoutDraft::from_saved(VehicleKind::T54_1951, &stale);

        assert_eq!(draft.option_index, [0; 6], "unknown options fall back to stock");
        assert!(draft.ammo_index < draft.ammo_options().len(), "ammo clamps into range");
        // W1: the historical dial value migrates UP to the pin — no save can undertrain a crew.
        assert!((draft.crew.proficiency() - 1.0).abs() < 1.0e-6, "the pin migrates old saves");
    }

    #[test]
    fn cycling_the_suspension_changes_assembled_turn_rate() {
        // The Tiger II carries a real second track (narrow transport vs wide combat), so cycling
        // moves the assembled traverse. The transport track is a sidegrade (worse turn for less
        // weight), so this locks that the swap *changes* traverse, not that it improves it.
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerII);
        let before = draft.assembled_spec().turn_rate_rad_s;
        draft.cycle_module(FitSlot::Suspension, 1);
        assert_ne!(
            draft.assembled_spec().turn_rate_rad_s,
            before,
            "the alternate track moves traverse"
        );
    }

    #[test]
    fn swapping_the_t54_gun_installs_a_different_gun_on_the_same_tube() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let (before_name, before_length) = (draft.gun_name(), draft.gun_barrel_length());
        draft.cycle_module(FitSlot::Gun, 1);
        assert_ne!(draft.gun_name(), before_name, "the alternate gun is installed");
        // ...and the silhouette does NOT change: the D-10T and D-10T2S are one physical tube.
        assert_eq!(
            draft.gun_barrel_length(),
            before_length,
            "the D-10 variants share their barrel — swapping must not stretch the gun"
        );
    }

    #[test]
    fn cycling_the_engine_changes_assembled_power() {
        // The T-54 has a real engine choice (V-54 stock, V-55 retrofit); the V-55 lifts power.
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
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
    fn the_default_rack_fill_matches_the_vehicles_authored_capacity() {
        // The honest-ammo pillar reaches the garage: a fresh T-54 draft carries its historical
        // 34-round rack, full, stock-heavy — not the old flat 40.
        let draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        assert_eq!(draft.rack_capacity(), VehicleKind::T54_1951.ammo_capacity());
        assert_eq!(draft.rack_total(), draft.rack_capacity(), "a fresh rack is full");
        assert_eq!(draft.assembled_spec().ammo.counts, draft.ammo_counts());
    }

    #[test]
    fn adjusting_ammo_counts_respects_capacity_and_never_empties_the_rack() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        // A full rack cannot take one more round anywhere.
        assert!(!draft.adjust_ammo_count(0, 1), "a full rack rejects +1");
        // Freeing two HE rounds opens exactly two spaces; +5 into slot 1 applies partially.
        assert!(draft.adjust_ammo_count(2, -2));
        let before = draft.ammo_counts()[1];
        assert!(draft.adjust_ammo_count(1, 5), "a partial top-up still counts as a change");
        assert_eq!(draft.ammo_counts()[1], before + 2, "only the free space is filled");
        assert_eq!(draft.rack_total(), draft.rack_capacity());
        // A slot floors at zero without disturbing the rest...
        assert!(draft.adjust_ammo_count(2, -999));
        assert_eq!(draft.ammo_counts()[2], 0);
        // ...and the rack as a whole floors at ONE round: drain everything and one AP stays.
        assert!(draft.adjust_ammo_count(1, -999));
        assert!(draft.adjust_ammo_count(0, -999));
        assert_eq!(draft.rack_total(), 1, "an empty rack cannot fight; one round stays aboard");
        assert!(!draft.adjust_ammo_count(0, -1), "the last round is not removable");
    }

    #[test]
    fn a_fresh_draft_never_fills_a_slot_the_gun_does_not_have() {
        // The phantom-slot defect: the old flat 3-way split gave a 2-slot gun (the IS-3's D-25T,
        // the Jagdtiger's Pak 80) rounds in a slot the garage never draws — and in battle the
        // clamped index fired a DIFFERENT round type. The fill must follow the gun's real slots.
        for kind in VehicleKind::PLAYABLE {
            let draft = LoadoutDraft::for_vehicle(kind);
            let slots = draft.ammo_options().len();
            for (i, &count) in draft.ammo_counts().iter().enumerate() {
                assert!(
                    i < slots || count == 0,
                    "{kind:?} slot {i} is a phantom holding {count} rounds"
                );
            }
            assert_eq!(draft.rack_total(), draft.rack_capacity(), "{kind:?} still fills full");
        }
        // The IS-3 concretely: two real slots, the special quarter folded back into them.
        let is3 = LoadoutDraft::for_vehicle(VehicleKind::IS3);
        assert_eq!(is3.ammo_options().len(), 2);
        assert!(is3.ammo_counts()[1] > 0, "the D-25T's second slot is HE, and it is stocked");
        assert_eq!(is3.ammo_counts()[2], 0);
    }

    #[test]
    fn swapping_the_gun_moves_the_rack_by_shell_type_not_slot_index() {
        // Jagdtiger: the Pak 80 carries 2 slots (AP, HE), the fielded 88 carries 3 (AP, APCR,
        // HE) — the one vehicle whose gun swap changes the slot count on the live catalog.
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::Jagdtiger);
        let he_rounds = draft.ammo_counts()[1];
        assert!(he_rounds > 0, "the Pak 80's slot 1 is its HE fill");
        draft.set_ammo(1); // select HE on the Pak 80
        assert!(draft.cycle_module(FitSlot::Gun, 1));
        assert_eq!(draft.ammo_options().len(), 3);
        assert_eq!(draft.ammo_counts()[2], he_rounds, "HE rounds follow the HE slot");
        assert_eq!(draft.ammo_counts()[1], 0, "no rounds materialise in the new APCR slot");
        assert_eq!(draft.rack_total(), draft.rack_capacity(), "the swap conserves the rack");
        assert_eq!(draft.ammo_index(), 2, "the HE selection follows its type");
        assert_eq!(
            draft.ammo_options()[draft.ammo_index()].shell_type,
            game_core::ShellType::HighExplosive
        );
    }

    #[test]
    fn rounds_the_new_gun_cannot_chamber_pour_into_the_stock_slot() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::Jagdtiger);
        assert!(draft.cycle_module(FitSlot::Gun, 1)); // onto the 3-slot 88
        // Load the APCR slot and select it, then swap back to the 2-slot Pak 80.
        assert!(draft.adjust_ammo_count(0, -10));
        assert!(draft.adjust_ammo_count(1, 10));
        draft.set_ammo(1);
        assert!(draft.cycle_module(FitSlot::Gun, 1)); // the two-gun ring wraps back to stock
        assert_eq!(draft.ammo_options().len(), 2);
        assert_eq!(draft.rack_total(), draft.rack_capacity(), "no round vanishes in the swap");
        assert_eq!(draft.ammo_counts()[2], 0, "no phantom slot survives the swap back");
        assert_eq!(draft.ammo_index(), 0, "a selection whose type is gone falls back to stock");
    }

    #[test]
    fn from_saved_rejects_an_overflowing_or_phantom_rack_without_panicking() {
        // The file is user-editable JSON: three u16::MAX counts used to overflow the u16 sum
        // (a debug panic), and a fill in a slot the gun doesn't have loaded verbatim.
        let mut stale = LoadoutDraft::for_vehicle(VehicleKind::IS3).to_saved();
        stale.ammo_counts = Some([u16::MAX; 3]);
        let overflow = LoadoutDraft::from_saved(VehicleKind::IS3, &stale);
        assert_eq!(overflow.rack_total(), overflow.rack_capacity(), "overflow degrades stock");

        stale.ammo_counts = Some([14, 10, 4]); // sums to the IS-3's 28, but slot 2 is a phantom
        let phantom = LoadoutDraft::from_saved(VehicleKind::IS3, &stale);
        assert_eq!(phantom.ammo_counts()[2], 0, "a phantom-slot fill degrades to the default");
        assert_eq!(phantom.rack_total(), phantom.rack_capacity());
    }

    #[test]
    fn an_edited_rack_fill_round_trips_and_a_stale_fill_degrades_to_default() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::TigerII);
        draft.adjust_ammo_count(0, -10);
        draft.adjust_ammo_count(2, 10);
        let restored = LoadoutDraft::from_saved(VehicleKind::TigerII, &draft.to_saved());
        assert_eq!(restored.ammo_counts(), draft.ammo_counts(), "the edited fill survives");
        assert_eq!(restored.assembled_spec().ammo.counts, draft.ammo_counts());

        // A save whose fill exceeds this build's capacity (a rebalance shrank the rack) or is
        // empty degrades to the stock default instead of loading an invalid rack.
        let mut stale = draft.to_saved();
        stale.ammo_counts = Some([999, 999, 999]);
        let degraded = LoadoutDraft::from_saved(VehicleKind::TigerII, &stale);
        assert_eq!(degraded.rack_total(), degraded.rack_capacity(), "overflow falls back full");
        stale.ammo_counts = Some([0, 0, 0]);
        let empty = LoadoutDraft::from_saved(VehicleKind::TigerII, &stale);
        assert!(empty.rack_total() >= 1, "an empty stored rack falls back to the default fill");
        // A pre-editor save (no counts at all) keeps the default fill.
        stale.ammo_counts = None;
        let legacy = LoadoutDraft::from_saved(VehicleKind::TigerII, &stale);
        assert_eq!(legacy.rack_total(), legacy.rack_capacity());
    }

    /// W1: proficiency is pinned — the assembled spec carries the gun's RATED reload, with
    /// no crew tax anywhere between the screen and the battle.
    #[test]
    fn the_assembled_spec_carries_the_rated_reload() {
        let draft = LoadoutDraft::for_vehicle(VehicleKind::TigerI);
        let rated = VehicleKind::TigerI.spec().gun.reload_seconds;
        assert!((draft.assembled_spec().gun.reload_seconds - rated).abs() < 1.0e-6);
    }

    #[test]
    fn current_module_summary_is_compact_and_changes_on_swap() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let before = draft.current_module_summary(FitSlot::Gun);
        assert!(!before.is_empty());
        let eng_before = draft.current_module_summary(FitSlot::Engine);
        draft.cycle_module(FitSlot::Engine, 1);
        let eng_after = draft.current_module_summary(FitSlot::Engine);
        assert_ne!(eng_before, eng_after, "the V-55 retrofit shows a different kW value");
    }

    #[test]
    fn the_radio_slot_states_a_stat_the_battle_honours() {
        // The audit's dead-number rule: a stat printed where the player picks modules must be
        // one the fight reads. The radio used to print "700m" of signal range that nothing in
        // the sim consumed; its honest headline is its mass (it rides the assembled weight).
        let draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let summary = draft.current_module_summary(FitSlot::Radio);
        assert!(summary.ends_with("kg"), "the radio summary is its mass, got {summary}");
        let radios = draft.module_options(FitSlot::Radio);
        assert_eq!(radios[0].unit, "kg");
        assert!(!radios[0].higher_is_better, "a lighter radio is the better radio");
    }

    #[test]
    fn cycle_module_returns_true_for_successful_install() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        assert!(draft.cycle_module(FitSlot::Gun, 1), "gun swap should succeed");
        assert!(draft.cycle_module(FitSlot::Engine, 1), "engine swap should succeed");
    }

    #[test]
    fn module_options_reports_names_stats_and_marks_the_installed() {
        let draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let guns = draft.module_options(FitSlot::Gun);
        assert_eq!(guns.len(), 2, "the T-54 has two guns to choose between");
        assert!(guns.iter().all(|o| !o.name.is_empty()), "every option carries a catalogue name");
        assert!(guns[0].installed, "the stock gun is the installed one");
        assert!(!guns[1].installed);
        // Reload is the gun's headline stat, and a shorter reload is better.
        assert!(!guns[0].higher_is_better, "lower reload is better → higher_is_better is false");
        assert_ne!(guns[0].stat, guns[1].stat, "the two guns differ on their headline stat");

        // A single-option slot (radio) reports exactly one option, installed.
        let radios = draft.module_options(FitSlot::Radio);
        assert_eq!(radios.len(), 1);
        assert!(radios[0].installed);
    }

    #[test]
    fn set_option_installs_a_specific_index_and_rejects_out_of_range() {
        let mut draft = LoadoutDraft::for_vehicle(VehicleKind::T54_1951);
        let before = draft.gun_name();
        assert!(draft.set_option(FitSlot::Gun, 1), "installing a valid option succeeds");
        assert_ne!(draft.gun_name(), before, "the picked option is installed");
        assert!(!draft.set_option(FitSlot::Gun, 99), "an out-of-range index is rejected");
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
