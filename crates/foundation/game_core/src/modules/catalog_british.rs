//! The British line's authored modules (Centurion Mk 3): the Meteor petrol engine, Horstmann
//! bogie running gear, the cast Mk 3 turret, and the two fielded 20-pounder barrels. Numbers
//! are practical gameplay specs grounded in public historical data (see
//! `docs/vehicles/centurion.md`).

use super::{
    EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule,
    TurretTraverse, VehicleModules,
};
use crate::{GunSpec, ShellSpec};

pub(crate) fn centurion_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "Centurion Mk 3 hull".to_string(),
            mass_kg: 31_800.0,
            hit_points: 1_650,
            front_mm: 76.0,
            side_mm: 51.0,
            rear_mm: 38.0,
            // 34.6 km/h flat out: the Centurion is protection and gunnery, not pace.
            max_forward_speed_mps: 9.6,
            max_reverse_speed_mps: 3.4,
        },
        engine: EngineModule {
            name: "Rolls-Royce Meteor".to_string(),
            power_kw: 480.0,
            mass_kg: 1_450.0,
            hit_points: 150,
            // A petrol V12 burns more eagerly than the Soviet diesels around it.
            fire_chance: 0.14,
        },
        suspension: SuspensionModule {
            name: "Horstmann bogies".to_string(),
            mass_kg: 4_200.0,
            hit_points: 170,
            turn_rate_rad_s: 0.62,
            max_load_kg: 52_000.0,
        },
        turret: TurretModule {
            name: "Centurion Mk 3 turret".to_string(),
            mass_kg: 9_500.0,
            hit_points: 250,
            front_mm: 152.0,
            side_mm: 112.0,
            rear_mm: 90.0,
            roof_mm: None,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.44 },
            // British optics: the best view range in the Cold War park.
            view_range_m: 390.0,
            max_gun_caliber_mm: 84.0,
        },
        gun: gun_20pdr_type_a(),
        radio: RadioModule {
            name: "WS No. 19".to_string(),
            mass_kg: 90.0,
            hit_points: 50,
            signal_range_m: 680.0,
        },
    }
}

/// The Ordnance QF 20-pounder, Type A barrel (the Mk 3's original, no fume extractor): the
/// gunnery argument of the British line. Less alpha than the D-10 family, paid back with the
/// best accuracy and handling in the era — and APDS in the second slot, the fastest shell in
/// the game, whose penetration falls off hard with range like the sub-caliber round it is.
pub(crate) fn gun_20pdr_type_a() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "84 mm 20-pounder Type A".to_string(),
            reload_seconds: 8.0,
            dispersion_mrad: 2.4,
            aim_time_seconds: 2.2,
            movement_bloom_mrad: 4.6,
            shot_bloom_mrad: 3.6,
            max_dispersion_mrad: 16.0,
            barrel_length_m: 5.6,
            // The Centurion mount: -10 / +18, the deepest depression in the fleet. This gun lives on a
            // ridgeline - the arc, the APDS and the 152 mm turret face are one identity.
            depression_deg: 10.0,
            elevation_deg: 18.0,
            shell: ShellSpec::armor_piercing(84.0, 1_020.0, 230.0, 240),
            // The 20-pounder's HE is attested and its numbers are not: velocity, mass and
            // filler are all GAPs in the dossier, so this row is a BALANCE decision at the 84 mm
            // class. What is sourced is the shape of this gun's loadout — APDS was the normal
            // load and conventional APCBC "rarely used", which inverts the usual stock/special
            // relationship and is the Centurion's own identity.
            // APDS at a sourced 1,465 m/s for ~300 mm — and per the sources this was the gun's
            // NORMAL load, with conventional APCBC "rarely used". The Centurion's identity is
            // that its special round is the everyday one.
            special_shell: Some(ShellSpec::apcr(84.0, 1_465.0, 300.0, 220)),
            he_shell: Some(ShellSpec::high_explosive(84.0, 850.0, 28.0, 290, 1.6)),
        },
        mass_kg: 1_950.0,
        hit_points: 150,
    }
}

/// The Type B barrel with the fume extractor: the same ballistics through a stiffer tube — it
/// settles faster and shoots tighter, but the heavier barrel loads a touch slower. A sidegrade
/// in the garage's spirit, not a strict upgrade.
pub(crate) fn gun_20pdr_type_b() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "84 mm 20-pounder Type B".to_string(),
            reload_seconds: 8.3,
            dispersion_mrad: 2.2,
            aim_time_seconds: 2.0,
            movement_bloom_mrad: 4.4,
            shot_bloom_mrad: 3.4,
            max_dispersion_mrad: 15.0,
            barrel_length_m: 5.6,
            // Same mount as the Type A.
            depression_deg: 10.0,
            elevation_deg: 18.0,
            shell: ShellSpec::armor_piercing(84.0, 1_020.0, 230.0, 240),
            // Same ammunition as the Type A; the bore evacuator does not change the shell.
            // Same ammunition as the Type A.
            special_shell: Some(ShellSpec::apcr(84.0, 1_465.0, 300.0, 220)),
            he_shell: Some(ShellSpec::high_explosive(84.0, 850.0, 28.0, 290, 1.6)),
        },
        mass_kg: 2_050.0,
        hit_points: 150,
    }
}
