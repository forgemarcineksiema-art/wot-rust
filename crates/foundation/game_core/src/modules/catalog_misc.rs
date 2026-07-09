use super::{
    EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule,
    TurretTraverse, VehicleModules,
};
use crate::{GunSpec, ShellSpec};

pub(crate) fn prototype_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "Prototype hull".to_string(),
            mass_kg: 17_500.0,
            hit_points: 1_450,
            front_mm: 120.0,
            side_mm: 80.0,
            rear_mm: 60.0,
            max_forward_speed_mps: 15.0,
            max_reverse_speed_mps: 5.0,
        },
        engine: EngineModule {
            name: "Prototype V12".to_string(),
            power_kw: 520.0,
            mass_kg: 1_600.0,
            hit_points: 160,
            fire_chance: 0.15,
        },
        suspension: SuspensionModule {
            name: "Prototype running gear".to_string(),
            mass_kg: 3_200.0,
            hit_points: 160,
            turn_rate_rad_s: 1.2,
            max_load_kg: 38_000.0,
        },
        turret: TurretModule {
            name: "Prototype turret".to_string(),
            mass_kg: 7_000.0,
            hit_points: 250,
            front_mm: 200.0,
            side_mm: 90.0,
            rear_mm: 60.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.75 },
            view_range_m: 390.0,
            max_gun_caliber_mm: 122.0,
        },
        gun: gun_prototype(),
        radio: RadioModule {
            name: "Prototype radio".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 700.0,
        },
    }
}

pub(crate) fn jagdtiger_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "Jagdtiger hull".to_string(),
            mass_kg: 49_400.0,
            hit_points: 2_200,
            front_mm: 150.0,
            side_mm: 80.0,
            rear_mm: 80.0,
            max_forward_speed_mps: 9.61,
            max_reverse_speed_mps: 2.4,
        },
        engine: EngineModule {
            name: "Maybach HL230 P30".to_string(),
            power_kw: 441.0,
            mass_kg: 2_000.0,
            hit_points: 170,
            fire_chance: 0.20,
        },
        suspension: SuspensionModule {
            name: "Jagdtiger running gear".to_string(),
            mass_kg: 5_200.0,
            hit_points: 190,
            turn_rate_rad_s: 0.32,
            max_load_kg: 82_000.0,
        },
        turret: TurretModule {
            name: "Jagdtiger casemate".to_string(),
            mass_kg: 15_000.0,
            hit_points: 320,
            front_mm: 250.0,
            side_mm: 80.0,
            rear_mm: 80.0,
            traverse: TurretTraverse::Fixed,
            view_range_m: 370.0,
            max_gun_caliber_mm: 130.0,
        },
        gun: gun_pak80(),
        radio: RadioModule {
            name: "FuG 5".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 650.0,
        },
    }
}

pub(crate) fn panther_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "Panther II hull".to_string(),
            mass_kg: 35_800.0,
            hit_points: 1_700,
            front_mm: 100.0,
            side_mm: 60.0,
            rear_mm: 40.0,
            max_forward_speed_mps: 12.78,
            max_reverse_speed_mps: 3.5,
        },
        engine: EngineModule {
            name: "Maybach HL234".to_string(),
            power_kw: 441.0,
            mass_kg: 1_800.0,
            hit_points: 170,
            fire_chance: 0.20,
        },
        suspension: SuspensionModule {
            name: "Panther II running gear".to_string(),
            mass_kg: 4_200.0,
            hit_points: 170,
            turn_rate_rad_s: 0.64,
            max_load_kg: 59_000.0,
        },
        turret: TurretModule {
            name: "Schmalturm".to_string(),
            mass_kg: 9_500.0,
            hit_points: 260,
            front_mm: 120.0,
            side_mm: 60.0,
            rear_mm: 45.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.38 },
            view_range_m: 380.0,
            max_gun_caliber_mm: 80.0,
        },
        gun: gun_kwk42(),
        radio: RadioModule {
            name: "FuG 5".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 650.0,
        },
    }
}

/// The Jagdtiger's narrow transport track (Verladekette): lighter, but it traverses worse and
/// carries less than the wide combat track. The same period trade the Tigers had.
pub(crate) fn jagdtiger_transport_track() -> SuspensionModule {
    SuspensionModule {
        name: "Jagdtiger transport track".to_string(),
        mass_kg: 4_800.0,
        hit_points: 185,
        turn_rate_rad_s: 0.28,
        max_load_kg: 78_000.0,
    }
}

/// The Panther II's transport track — narrow rail-loading gear: lighter, worse traverse, less load
/// than the wide combat track.
pub(crate) fn panther_transport_track() -> SuspensionModule {
    SuspensionModule {
        name: "Panther II transport track".to_string(),
        mass_kg: 3_850.0,
        hit_points: 165,
        turn_rate_rad_s: 0.57,
        max_load_kg: 55_000.0,
    }
}

/// The 8.8 cm Pak 43/3 L/71 of the *actually fielded* "88 Jagdtiger" — a batch built with the long
/// 88 when the 12.8 cm was in short supply. Half the alpha of the Pak 80 and less penetration, but
/// it reloads in two-thirds the time and handles far better: a DPM/accuracy trade against the
/// 12.8 cm's vertical punch. Same L/71 ballistics as the Tiger II's KwK 43.
pub(crate) fn gun_pak43_l71() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "8.8 cm Pak 43/3 L/71".to_string(),
            reload_seconds: 8.6,
            dispersion_mrad: 2.0,
            aim_time_seconds: 2.5,
            movement_bloom_mrad: 3.8,
            shot_bloom_mrad: 3.2,
            max_dispersion_mrad: 14.0,
            barrel_length_m: 6.6,
            shell: ShellSpec::armor_piercing(88.0, 1_000.0, 202.0, 390),
        },
        mass_kg: 2_400.0,
        hit_points: 170,
    }
}

pub(crate) fn gun_prototype() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "120 mm Prototype".to_string(),
            reload_seconds: 8.0,
            dispersion_mrad: 2.5,
            aim_time_seconds: 2.4,
            movement_bloom_mrad: 4.6,
            shot_bloom_mrad: 3.7,
            max_dispersion_mrad: 17.0,
            barrel_length_m: 5.6,
            shell: ShellSpec::armor_piercing(120.0, 900.0, 250.0, 390),
        },
        mass_kg: 2_600.0,
        hit_points: 160,
    }
}

pub(crate) fn gun_pak80() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "12.8 cm Pak 80 L/55".to_string(),
            reload_seconds: 13.5,
            dispersion_mrad: 1.9,
            aim_time_seconds: 2.9,
            movement_bloom_mrad: 3.2,
            shot_bloom_mrad: 3.5,
            max_dispersion_mrad: 14.0,
            barrel_length_m: 7.0,
            shell: ShellSpec::armor_piercing(128.0, 920.0, 223.0, 530),
        },
        mass_kg: 3_500.0,
        hit_points: 180,
    }
}

pub(crate) fn gun_kwk42() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "7.5 cm KwK 42 L/70".to_string(),
            reload_seconds: 6.6,
            dispersion_mrad: 2.0,
            aim_time_seconds: 2.0,
            movement_bloom_mrad: 4.0,
            shot_bloom_mrad: 3.2,
            max_dispersion_mrad: 15.0,
            barrel_length_m: 5.2,
            shell: ShellSpec::armor_piercing(75.0, 935.0, 138.0, 240),
        },
        mass_kg: 1_600.0,
        hit_points: 140,
    }
}
