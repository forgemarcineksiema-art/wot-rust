use super::catalog_german::{gun_kwk36, gun_kwk43, tiger_i_loadout, tiger_ii_loadout};
use super::catalog_misc::{
    gun_kwk42, gun_pak80, gun_prototype, jagdtiger_loadout, panther_loadout, prototype_loadout,
};
use super::catalog_soviet::{
    gun_d10t, gun_d10t2s, gun_d25t, is3_loadout, t54_loadout, t55_loadout,
};
use super::{
    EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule,
    VehicleModules,
};
use crate::VehicleKind;

impl VehicleKind {
    /// The factory (stock) module loadout for this vehicle. Assembling it reproduces the
    /// vehicle's canonical [`crate::TankSpec`].
    pub fn default_loadout(self) -> VehicleModules {
        match self {
            VehicleKind::PrototypeMedium => prototype_loadout(),
            VehicleKind::T54_1951 => t54_loadout(),
            VehicleKind::T55A => t55_loadout(),
            VehicleKind::TigerI => tiger_i_loadout(),
            VehicleKind::TigerII => tiger_ii_loadout(),
            VehicleKind::Jagdtiger => jagdtiger_loadout(),
            VehicleKind::PantherII => panther_loadout(),
            VehicleKind::IS3 => is3_loadout(),
        }
    }

    /// The stock gun's barrel length (m) — the reference the silhouette and muzzle scale against.
    pub fn stock_barrel_length_m(self) -> f32 {
        self.default_loadout().gun.barrel_length_m()
    }

    /// Guns that can be mounted on this vehicle (first entry is the stock gun). Demonstrates
    /// swappable armament; mounting still goes through [`VehicleModules::try_install_gun`].
    pub fn gun_options(self) -> Vec<GunModule> {
        match self {
            VehicleKind::PrototypeMedium => vec![gun_prototype()],
            VehicleKind::T54_1951 => vec![gun_d10t(), gun_d10t2s()],
            VehicleKind::T55A => vec![gun_d10t2s(), gun_d10t()],
            VehicleKind::TigerI => vec![gun_kwk36()],
            VehicleKind::TigerII => vec![gun_kwk43()],
            VehicleKind::Jagdtiger => vec![gun_pak80()],
            VehicleKind::PantherII => vec![gun_kwk42()],
            VehicleKind::IS3 => vec![gun_d25t()],
        }
    }

    /// Turrets mountable on this vehicle (first is stock). Casemate TDs and vehicles without an
    /// alternate turret return just the stock turret — the slot is shown but not swappable.
    pub fn turret_options(self) -> Vec<TurretModule> {
        vec![self.default_loadout().turret]
    }

    /// Hull chassis options (first is stock). The hull is fixed today, so this is a single entry;
    /// it keeps the garage's slot list uniform and leaves room for alternates later.
    pub fn hull_options(self) -> Vec<HullChassis> {
        vec![self.default_loadout().hull]
    }

    /// Engines (first is stock). The uprated engine is a sidegrade, not a free win: more power, but
    /// heavier and more prone to catching fire when knocked out. No economy — both are selectable.
    pub fn engine_options(self) -> Vec<EngineModule> {
        let stock = self.default_loadout().engine;
        let uprated = EngineModule {
            name: format!("{} (uprated)", stock.name),
            power_kw: stock.power_kw * 1.15,
            mass_kg: stock.mass_kg * 1.10,
            fire_chance: (stock.fire_chance + 0.08).min(1.0),
            ..stock.clone()
        };
        vec![stock, uprated]
    }

    /// Running gear (first is stock). The reinforced set is a sidegrade: it turns faster and
    /// carries more, but is heavier (which eats into power-to-weight). The load limit it sets still
    /// gates heavier turret/gun installs.
    pub fn suspension_options(self) -> Vec<SuspensionModule> {
        let stock = self.default_loadout().suspension;
        let reinforced = SuspensionModule {
            name: format!("{} (reinforced)", stock.name),
            turn_rate_rad_s: stock.turn_rate_rad_s * 1.12,
            max_load_kg: stock.max_load_kg * 1.06,
            mass_kg: stock.mass_kg * 1.18,
            ..stock.clone()
        };
        vec![stock, reinforced]
    }

    /// Radios (first is stock). There is no meaningful radio sidegrade in this prototype, so the
    /// slot shows the stock radio and is not swappable rather than offering a free range bump.
    pub fn radio_options(self) -> Vec<RadioModule> {
        vec![self.default_loadout().radio]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_lists_the_stock_option_first_and_assembles() {
        for kind in VehicleKind::ALL {
            let default = kind.default_loadout();
            assert_eq!(kind.gun_options()[0], default.gun, "{kind:?} stock gun first");
            assert_eq!(kind.turret_options()[0], default.turret);
            assert_eq!(kind.hull_options()[0], default.hull);
            assert_eq!(kind.engine_options()[0], default.engine);
            assert_eq!(kind.suspension_options()[0], default.suspension);
            assert_eq!(kind.radio_options()[0], default.radio);

            // The reinforced running gear is a real upgrade (faster traverse, more load headroom).
            let suspensions = kind.suspension_options();
            let last = &suspensions[suspensions.len() - 1];
            assert!(last.turn_rate_rad_s > suspensions[0].turn_rate_rad_s);
            assert!(last.max_load_kg >= suspensions[0].max_load_kg);

            // The uprated engine is a real upgrade and any option still assembles to a valid spec.
            let engines = kind.engine_options();
            assert!(engines[engines.len() - 1].power_kw > engines[0].power_kw);
            for engine in engines {
                let mut modules = kind.default_loadout();
                modules.engine = engine;
                assert!(modules.assemble(kind).engine_power_kw > 0.0);
            }
        }
    }
}
