use super::{
    EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule,
    TurretTraverse, VehicleModules,
};
use crate::{GunSpec, ShellSpec};

pub(crate) fn t54_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "T-54 hull".to_string(),
            mass_kg: 20_100.0,
            hit_points: 1_550,
            front_mm: 100.0,
            side_mm: 80.0,
            rear_mm: 45.0,
            max_forward_speed_mps: 13.89,
            max_reverse_speed_mps: 4.2,
        },
        engine: EngineModule {
            name: "V-54".to_string(),
            power_kw: 390.0,
            mass_kg: 1_500.0,
            hit_points: 150,
            fire_chance: 0.10,
        },
        suspension: SuspensionModule {
            name: "T-54 running gear".to_string(),
            mass_kg: 3_500.0,
            hit_points: 150,
            turn_rate_rad_s: 0.78,
            max_load_kg: 42_000.0,
        },
        turret: TurretModule {
            name: "T-54 turret".to_string(),
            mass_kg: 8_500.0,
            hit_points: 240,
            front_mm: 200.0,
            side_mm: 90.0,
            rear_mm: 65.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.42 },
            view_range_m: 370.0,
            max_gun_caliber_mm: 105.0,
        },
        gun: gun_d10t(),
        radio: RadioModule {
            name: "10-RT".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 700.0,
        },
    }
}

pub(crate) fn t55_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "T-55 hull".to_string(),
            mass_kg: 19_750.0,
            hit_points: 1_600,
            front_mm: 100.0,
            side_mm: 80.0,
            rear_mm: 45.0,
            max_forward_speed_mps: 15.28,
            max_reverse_speed_mps: 4.5,
        },
        engine: EngineModule {
            name: "V-55".to_string(),
            power_kw: 433.0,
            mass_kg: 1_550.0,
            hit_points: 150,
            fire_chance: 0.10,
        },
        suspension: SuspensionModule {
            name: "T-55 running gear".to_string(),
            mass_kg: 3_500.0,
            hit_points: 150,
            turn_rate_rad_s: 0.82,
            max_load_kg: 42_000.0,
        },
        turret: TurretModule {
            name: "T-55 turret".to_string(),
            mass_kg: 8_800.0,
            hit_points: 240,
            front_mm: 200.0,
            side_mm: 90.0,
            rear_mm: 65.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.46 },
            view_range_m: 380.0,
            max_gun_caliber_mm: 105.0,
        },
        gun: gun_d10t2s(),
        radio: RadioModule {
            name: "R-113".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 710.0,
        },
    }
}

pub(crate) fn gun_d10t() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "100 mm D-10T".to_string(),
            reload_seconds: 8.4,
            dispersion_mrad: 2.9,
            aim_time_seconds: 2.5,
            movement_bloom_mrad: 5.0,
            shot_bloom_mrad: 4.0,
            max_dispersion_mrad: 18.0,
            barrel_length_m: 5.0,
            shell: ShellSpec::armor_piercing(100.0, 895.0, 185.0, 320),
        },
        mass_kg: 2_300.0,
        hit_points: 150,
    }
}

pub(crate) fn gun_d10t2s() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "100 mm D-10T2S".to_string(),
            reload_seconds: 8.0,
            dispersion_mrad: 2.7,
            aim_time_seconds: 2.3,
            movement_bloom_mrad: 4.8,
            shot_bloom_mrad: 3.8,
            max_dispersion_mrad: 17.0,
            barrel_length_m: 5.9,
            // Sidegrade vs the D-10T: faster, flatter, more penetration, but lower per-shot alpha
            // (320 -> 300) — a DPM/accuracy gun rather than a strict upgrade.
            shell: ShellSpec::armor_piercing(100.0, 895.0, 195.0, 300),
        },
        mass_kg: 2_300.0,
        hit_points: 150,
    }
}
