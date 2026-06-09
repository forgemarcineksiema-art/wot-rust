use super::catalog_german::{gun_kwk36, gun_kwk43, tiger_i_loadout, tiger_ii_loadout};
use super::catalog_misc::{
    gun_kwk42, gun_pak80, gun_prototype, jagdtiger_loadout, panther_loadout, prototype_loadout,
};
use super::catalog_soviet::{gun_d10t, gun_d10t2s, t54_loadout, t55_loadout};
use super::{GunModule, VehicleModules};
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
        }
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
        }
    }
}
