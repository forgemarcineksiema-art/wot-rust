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

pub(crate) fn t34_85_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "T-34-85 hull".to_string(),
            mass_kg: 18_000.0,
            hit_points: 1_300,
            // The 45 mm plate everywhere — the T-34 bet on SLOPE, not thickness: the 60-degree
            // glacis is the armour, the plate is just its material.
            front_mm: 45.0,
            side_mm: 45.0,
            rear_mm: 45.0,
            max_forward_speed_mps: 15.0,
            max_reverse_speed_mps: 4.0,
        },
        engine: EngineModule {
            name: "V-2-34".to_string(),
            power_kw: 368.0,
            mass_kg: 1_400.0,
            hit_points: 140,
            // The fuel cells ride in the fighting compartment sponsons — the T-34's documented
            // fire liability.
            fire_chance: 0.13,
        },
        suspension: SuspensionModule {
            name: "T-34 Christie gear".to_string(),
            mass_kg: 3_300.0,
            hit_points: 140,
            turn_rate_rad_s: 0.80,
            max_load_kg: 36_000.0,
        },
        turret: TurretModule {
            name: "T-34-85 turret".to_string(),
            mass_kg: 7_500.0,
            hit_points: 220,
            front_mm: 90.0,
            side_mm: 75.0,
            rear_mm: 52.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.44 },
            view_range_m: 350.0,
            max_gun_caliber_mm: 85.0,
        },
        gun: gun_zis_s53(),
        radio: RadioModule {
            name: "9-RS".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 550.0,
        },
    }
}

/// The 85 mm ZiS-S-53: the war-winning medium's gun. Fast-handling and quick to reload — the
/// Era II Soviet answer trades the German guns' penetration for cadence and mobility-friendly
/// bloom; the BR-365P arrowhead round buys back penetration at a per-shot damage cost.
pub(crate) fn gun_zis_s53() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "85 mm ZiS-S-53".to_string(),
            reload_seconds: 7.4,
            dispersion_mrad: 3.0,
            aim_time_seconds: 2.3,
            movement_bloom_mrad: 4.6,
            shot_bloom_mrad: 3.8,
            max_dispersion_mrad: 17.0,
            barrel_length_m: 4.645,
            shell: ShellSpec::armor_piercing(85.0, 792.0, 145.0, 200),
            special_shell: Some(ShellSpec::apcr(85.0, 1_030.0, 170.0, 170)),
        },
        mass_kg: 1_700.0,
        hit_points: 140,
    }
}

/// The KV-1 mod. 1942 (reinforced cast turret): Era II's anvil. 47 t of thick plate with almost
/// no slope anywhere — a 90 mm bow, 75 mm sides standing dead vertical, and a cast turret that is
/// 100 mm on the face AND on both cheeks. That turret flank is the thickest in Era II, which is
/// the vehicle's real armour identity: there is no cheap angle on it. It pays for that with the
/// slowest hull in the game, the worst optics, the clumsiest steering of any TURRETED vehicle
/// (only the casemate Jagdtiger turns worse), and a 76 mm that cannot open a Tiger from the
/// front. Dossier: docs/vehicles/kv-1.md.
pub(crate) fn kv1_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "KV-1 mod. 1942 hull".to_string(),
            mass_kg: 28_100.0,
            hit_points: 1_750,
            front_mm: 90.0,
            // 75 mm VERTICAL. Angling this hull buys nothing — and costs nothing either.
            side_mm: 75.0,
            rear_mm: 70.0,
            // The mod. 1941 did 35 km/h at 45 t; hanging the heavy cast turret on the same V-2K
            // cost that speed, and recovering it is precisely why the lightened KV-1S exists.
            max_forward_speed_mps: 7.8,
            max_reverse_speed_mps: 2.8,
        },
        engine: EngineModule {
            name: "V-2K".to_string(),
            // 600 hp.
            power_kw: 441.0,
            mass_kg: 1_550.0,
            hit_points: 155,
            // Diesel: well under the German petrol engines' 0.20.
            fire_chance: 0.12,
        },
        suspension: SuspensionModule {
            name: "KV torsion-bar gear".to_string(),
            // Six stations of torsion bar under the heavy 700 mm track.
            mass_kg: 5_000.0,
            hit_points: 190,
            // Clutch-and-brake steering under 47 t: it turns worse than a Tiger II.
            turn_rate_rad_s: 0.42,
            max_load_kg: 52_000.0,
        },
        turret: TurretModule {
            name: "KV-1 cast turret (1942)".to_string(),
            mass_kg: 11_000.0,
            hit_points: 260,
            // One casting all the way round. The base thickness, NOT the selective 110-120 mm
            // patches the real turret carried at its weak areas — those were local reinforcement,
            // and modelling them as a uniform face would overstate the tank.
            front_mm: 100.0,
            side_mm: 100.0,
            // The rear carries the DT ball and its armoured collar.
            rear_mm: 90.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.30 },
            // 1942 Soviet optics: the worst in the fleet.
            view_range_m: 330.0,
            max_gun_caliber_mm: 85.0,
        },
        gun: gun_zis5(),
        radio: RadioModule {
            name: "10R".to_string(),
            mass_kg: 100.0,
            hit_points: 50,
            signal_range_m: 500.0,
        },
    }
}

/// The 76 mm ZiS-5: honest about its limits. Quick to load and quick to settle, and it simply does
/// not have the penetration to open German heavy armour from the front — BR-350A's 86 mm against
/// a Tiger's sloped 100 mm plate is not a bad roll, it is arithmetic. The scarce BR-350P arrowhead
/// buys enough to punch a Tiger's bow at close range and never enough for a Tiger II's. Against
/// everything's flanks it works exactly as it should.
pub(crate) fn gun_zis5() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "76 mm ZiS-5".to_string(),
            reload_seconds: 6.2,
            dispersion_mrad: 3.2,
            aim_time_seconds: 2.1,
            movement_bloom_mrad: 4.4,
            shot_bloom_mrad: 3.2,
            max_dispersion_mrad: 17.0,
            // L/41.5.
            barrel_length_m: 3.16,
            shell: ShellSpec::armor_piercing(76.2, 680.0, 86.0, 160),
            special_shell: Some(ShellSpec::apcr(76.2, 950.0, 102.0, 140)),
        },
        mass_kg: 1_250.0,
        hit_points: 120,
    }
}

pub(crate) fn is3_loadout() -> VehicleModules {
    VehicleModules {
        hull: HullChassis {
            name: "IS-3 hull".to_string(),
            mass_kg: 26_000.0,
            hit_points: 1_900,
            front_mm: 110.0,
            side_mm: 90.0,
            rear_mm: 60.0,
            max_forward_speed_mps: 11.1,
            max_reverse_speed_mps: 3.9,
        },
        engine: EngineModule {
            name: "V-11".to_string(),
            power_kw: 382.0,
            mass_kg: 1_700.0,
            hit_points: 160,
            fire_chance: 0.12,
        },
        suspension: SuspensionModule {
            name: "IS-3 running gear".to_string(),
            mass_kg: 4_500.0,
            hit_points: 180,
            turn_rate_rad_s: 0.58,
            max_load_kg: 50_000.0,
        },
        turret: TurretModule {
            name: "IS-3 turret".to_string(),
            mass_kg: 11_000.0,
            hit_points: 280,
            front_mm: 250.0,
            side_mm: 160.0,
            rear_mm: 110.0,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.36 },
            view_range_m: 350.0,
            max_gun_caliber_mm: 130.0,
        },
        gun: gun_d25t(),
        radio: RadioModule {
            name: "10-RK-26".to_string(),
            mass_kg: 110.0,
            hit_points: 50,
            signal_range_m: 625.0,
        },
    }
}

/// The 122 mm D-25T: the heavy's argument. Slow to load, slow to settle, and it does not care —
/// one shell carries a medium's two. The role trade against the D-10 line is vertical alpha for
/// horizontal everything-else: DPM, handling, and shell speed all yield.
pub(crate) fn gun_d25t() -> GunModule {
    GunModule {
        spec: GunSpec {
            name: "122 mm D-25T".to_string(),
            reload_seconds: 12.6,
            dispersion_mrad: 3.4,
            aim_time_seconds: 3.0,
            movement_bloom_mrad: 5.6,
            shot_bloom_mrad: 4.8,
            max_dispersion_mrad: 20.0,
            barrel_length_m: 5.5,
            shell: ShellSpec::armor_piercing(122.0, 795.0, 175.0, 390),
            special_shell: None,
        },
        mass_kg: 2_600.0,
        hit_points: 160,
    }
}

/// The V-55 (580 hp) fitted to late T-54s and the T-55 line — a real retrofit over the V-54's
/// 520 hp. Mostly upside (it was a genuine improvement), at a little extra weight and heat; kept as
/// an authored alternate rather than a synthetic multiplier so the number means the real engine.
pub(crate) fn t54_engine_v55() -> EngineModule {
    EngineModule {
        name: "V-55".to_string(),
        power_kw: 433.0,
        mass_kg: 1_550.0,
        hit_points: 150,
        fire_chance: 0.11,
    }
}

/// The V-54K-IS the IS-3M modernization fitted — a modest, reliability-driven uprate over the V-11.
pub(crate) fn is3_engine_v54k() -> EngineModule {
    EngineModule {
        name: "V-54K-IS".to_string(),
        power_kw: 397.0,
        mass_kg: 1_720.0,
        hit_points: 165,
        fire_chance: 0.11,
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
            // The D-10 family's fielded chemical round: penetration that ignores range, paid for
            // with the HEAT weaknesses the armour model enforces (spaced screens kill the jet,
            // extreme obliquity sheds it).
            special_shell: Some(ShellSpec::heat(100.0, 900.0, 280.0, 320)),
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
            // The same BK-5 the D-10T loads — one physical round for the whole gun family.
            special_shell: Some(ShellSpec::heat(100.0, 900.0, 280.0, 320)),
        },
        mass_kg: 2_300.0,
        hit_points: 150,
    }
}
