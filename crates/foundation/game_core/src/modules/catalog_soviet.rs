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
            // The documented casting: 200 mm face, a side wall that starts at 160 mm behind the
            // cheeks and thins to the 65 mm rear, 30 mm roof. The single 90 mm "side" this used
            // to quote was an average of a wall that is nowhere 90: it made the T-54's cheeks
            // paper and its rear over-armoured at the same time.
            front_mm: 200.0,
            side_mm: 160.0,
            rear_mm: 65.0,
            roof_mm: Some(30.0),
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.42 },
            max_gun_caliber_mm: 105.0,
        },
        gun: gun_d10t(),
        radio: RadioModule { name: "10-RT".to_string(), mass_kg: 100.0, hit_points: 50 },
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
            roof_mm: None,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.44 },
            max_gun_caliber_mm: 85.0,
        },
        gun: gun_zis_s53(),
        radio: RadioModule { name: "9-RS".to_string(), mass_kg: 100.0, hit_points: 50 },
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
            // The T-34-85 turret: -5 / +22. Soviet depression pays for the low silhouette.
            depression_deg: 5.0,
            elevation_deg: 22.0,
            shell: ShellSpec::armor_piercing(85.0, 792.0, 145.0, 200),
            // O-365K, fully sourced: 9.54 kg at 793 m/s with 0.741 kg of TNT. The old
            // derivation flew it at 554 — thirty per cent slow, because Soviet tank HE was a
            // FULL-CHARGE round, not a reduced one. 300 HP is the anchor the other guns' HE is
            // scaled against by filler mass (cube-root blast scaling), which is why the 88 with a
            // near-identical shell lands on the same number.
            // BR-365P APCR, 4.95 kg of tungsten at a sourced 1,050 m/s (the catalog carried
            // 1,030 before this research pass).
            special_shell: Some(ShellSpec::apcr(85.0, 1_050.0, 170.0, 170)),
            he_shell: Some(ShellSpec::high_explosive(85.0, 793.0, 28.0, 300, 1.6)),
        },
        mass_kg: 1_700.0,
        hit_points: 140,
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
            roof_mm: None,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.36 },
            max_gun_caliber_mm: 130.0,
        },
        gun: gun_d25t(),
        radio: RadioModule { name: "10-RK-26".to_string(), mass_kg: 110.0, hit_points: 50 },
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
            // THE ARC THAT IS AN IDENTITY: -3 of depression, the shallowest in the fleet. The pike-nosed
            // brawler cannot use a ridge the way the Western turrets do - it fights on the flat,
            // nose-on, which is exactly what its armour layout wants anyway.
            depression_deg: 3.0,
            elevation_deg: 20.0,
            shell: ShellSpec::armor_piercing(122.0, 795.0, 175.0, 390),
            // NO SPECIAL ROUND: the D-25T fielded no tungsten shell, so two slots. OF-471 is
            // 25.53 kg with 3.605 kg of TNT (sourced); the ~800 m/s is convention rather than a
            // sourced figure and is marked GAP in the dossier. 510 HP is the 85 mm anchor scaled
            // by the cube root of filler mass — the physical driver of blast, not the AP alpha,
            // which used to give this gun and the 88 IDENTICAL high explosive.
            special_shell: None,
            he_shell: Some(ShellSpec::high_explosive(122.0, 800.0, 41.0, 510, 2.4)),
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
            // The D-10 family is ONE physical tube: 5350 mm monobloc (L/53.5), 5608 mm with
            // the breech. The D-10T and the later D-10T2S differ in the breech, the stabilizer
            // and the fume extractor — not in barrel length. The old 5.0 vs 5.9 split made the
            // upgrade visibly stretch the silhouette, which no photograph supports.
            barrel_length_m: 5.35,
            // -5 / +18: the documented arc of the D-10 in the T-54's low turret, and one of
            // the tank's defining weaknesses. The low cast dome that makes it hard to hit is
            // exactly what leaves the breech no room to drop — a Soviet medium cannot play the
            // ridge like a Centurion, and now it cannot in the game either.
            depression_deg: 5.0,
            elevation_deg: 18.0,
            shell: ShellSpec::armor_piercing(100.0, 895.0, 185.0, 320),
            // The D-10 family's fielded chemical round: penetration that ignores range, paid for
            // with the HEAT weaknesses the armour model enforces (spaced screens kill the jet,
            // extreme obliquity sheds it).
            // OF-412, fully sourced: 15.8 kg at 900 m/s with 2.16 kg of TNT. Note the
            // velocity — the HE round leaves this muzzle FASTER than its own AP (895), and the
            // derivation flew it at 626. Flight time and drop are what a player leads with.
            // BK-5 HEAT: 280 mm flat with range, and a spaced screen kills the jet. Authored
            // long before this pass and unchanged by it.
            special_shell: Some(ShellSpec::heat(100.0, 900.0, 280.0, 320)),
            he_shell: Some(ShellSpec::high_explosive(100.0, 900.0, 33.0, 430, 2.0)),
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
            // Same tube as the D-10T — see the note there.
            barrel_length_m: 5.35,
            // -5 / +18: the documented arc of the D-10 in the T-54's low turret, and one of
            // the tank's defining weaknesses. The low cast dome that makes it hard to hit is
            // exactly what leaves the breech no room to drop — a Soviet medium cannot play the
            // ridge like a Centurion, and now it cannot in the game either.
            depression_deg: 5.0,
            elevation_deg: 18.0,
            // Sidegrade vs the D-10T: faster, flatter, more penetration, but lower per-shot alpha
            // (320 -> 300) — a DPM/accuracy gun rather than a strict upgrade.
            shell: ShellSpec::armor_piercing(100.0, 895.0, 195.0, 300),
            // The same BK-5 the D-10T loads — one physical round for the whole gun family.
            // Same shell as the D-10T; the stabiliser does not change the ammunition.
            // The same BK-5 the D-10T fires.
            special_shell: Some(ShellSpec::heat(100.0, 900.0, 280.0, 320)),
            he_shell: Some(ShellSpec::high_explosive(100.0, 900.0, 33.0, 430, 2.0)),
        },
        mass_kg: 2_300.0,
        hit_points: 150,
    }
}
