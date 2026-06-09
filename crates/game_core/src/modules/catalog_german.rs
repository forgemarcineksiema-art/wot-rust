use super::{
    EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule,
    TurretTraverse, VehicleModules,
};
use crate::{GunSpec, ShellSpec};

pub(crate) fn tiger_i_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "Tiger Ausf. E hull".to_string(),
            mass_kg: 37_100.0,
            hit_points: 1_850,
            front_mm: 100.0,
            side_mm: 80.0,
            rear_mm: 80.0,
            max_forward_speed_mps: 10.56,
            max_reverse_speed_mps: 3.2,
        },
        engine: EngineModule {
            name: "Maybach HL230".to_string(),
            power_kw: 515.0,
            mass_kg: 2_100.0,
            hit_points: 170,
            fire_chance: 0.20,
        },
        suspension: SuspensionModule {
            name: "Tiger running gear".to_string(),
            mass_kg: 4_500.0,
            hit_points: 170,
            turn_rate_rad_s: 0.58,
            max_load_kg: 63_000.0,
        },
        turret: TurretModule {
            name: "Tiger turret".to_string(),
            mass_kg: 11_000.0,
            hit_points: 280,
            front_mm: 100.0,
            side_mm: 80.0,
            rear_mm: 80.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.33 },
            view_range_m: 380.0,
            max_gun_caliber_mm: 90.0,
        },
        gun: gun_kwk36(),
        radio: RadioModule {
            name: "FuG 5".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 650.0,
        },
    }
}

pub(crate) fn tiger_ii_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "Tiger II hull".to_string(),
            mass_kg: 47_300.0,
            hit_points: 2_050,
            front_mm: 150.0,
            side_mm: 80.0,
            rear_mm: 80.0,
            max_forward_speed_mps: 10.56,
            max_reverse_speed_mps: 3.0,
        },
        engine: EngineModule {
            name: "Maybach HL230 P30".to_string(),
            power_kw: 515.0,
            mass_kg: 2_100.0,
            hit_points: 170,
            fire_chance: 0.20,
        },
        suspension: SuspensionModule {
            name: "Tiger II running gear".to_string(),
            mass_kg: 5_000.0,
            hit_points: 180,
            turn_rate_rad_s: 0.45,
            max_load_kg: 76_000.0,
        },
        turret: TurretModule {
            name: "Serienturm".to_string(),
            mass_kg: 13_000.0,
            hit_points: 300,
            front_mm: 180.0,
            side_mm: 80.0,
            rear_mm: 80.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.28 },
            view_range_m: 390.0,
            max_gun_caliber_mm: 90.0,
        },
        gun: gun_kwk43(),
        radio: RadioModule {
            name: "FuG 5".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 650.0,
        },
    }
}

pub(crate) fn gun_kwk36() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "8.8 cm KwK 36 L/56".to_string(),
            reload_seconds: 7.8,
            dispersion_mrad: 2.4,
            aim_time_seconds: 2.4,
            movement_bloom_mrad: 4.5,
            shot_bloom_mrad: 3.6,
            max_dispersion_mrad: 16.0,
            shell: ShellSpec::armor_piercing(88.0, 773.0, 165.0, 360),
        },
        mass_kg: 2_200.0,
        hit_points: 150,
    }
}

pub(crate) fn gun_kwk43() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "8.8 cm KwK 43 L/71".to_string(),
            reload_seconds: 8.8,
            dispersion_mrad: 2.1,
            aim_time_seconds: 2.7,
            movement_bloom_mrad: 4.2,
            shot_bloom_mrad: 3.4,
            max_dispersion_mrad: 15.0,
            shell: ShellSpec::armor_piercing(88.0, 1_000.0, 202.0, 390),
        },
        mass_kg: 2_300.0,
        hit_points: 150,
    }
}
