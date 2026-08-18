use super::{
    EngineModule, GunModule, HullChassis, RadioModule, SuspensionModule, TurretModule,
    TurretTraverse, VehicleModules,
};
use crate::{GunSpec, RoundId};

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
            roof_mm: None,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.33 },
            max_gun_caliber_mm: 90.0,
        },
        gun: gun_kwk36(),
        radio: RadioModule { name: "FuG 5".to_string(), mass_kg: 100.0, hit_points: 50 },
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
            roof_mm: None,
            traverse: TurretTraverse::Rotating { rate_rad_s: 0.28 },
            max_gun_caliber_mm: 90.0,
        },
        gun: gun_kwk43(),
        radio: RadioModule { name: "FuG 5".to_string(), mass_kg: 100.0, hit_points: 50 },
    }
}

/// The Maybach HL210 P45 (650 PS) of the early-production Tiger, before the HL230 P45 (700 PS)
/// replaced it. An aluminium block, so it runs lighter and a touch cooler than the HL230 — a real
/// sidegrade (less power for less weight), not a synthetic detune.
pub(crate) fn tiger_i_engine_hl210() -> EngineModule {
    EngineModule {
        name: "Maybach HL210 P45".to_string(),
        power_kw: 478.0,
        mass_kg: 1_950.0,
        hit_points: 165,
        fire_chance: 0.19,
    }
}

/// The Tiger's narrow rail-loading track (Verladekette), swapped on to fit railway loading gauge.
/// A narrower contact patch traverses worse and carries less, but weighs less than the wide combat
/// track (Gefechtskette) — a genuine period trade, not a formula.
pub(crate) fn tiger_i_transport_track() -> SuspensionModule {
    SuspensionModule {
        name: "Tiger transport track".to_string(),
        mass_kg: 4_100.0,
        hit_points: 165,
        turn_rate_rad_s: 0.52,
        max_load_kg: 60_000.0,
    }
}

/// The Tiger II's transport track — the same narrow/wide trade as the Tiger I's, scaled to the
/// heavier hull.
pub(crate) fn tiger_ii_transport_track() -> SuspensionModule {
    SuspensionModule {
        name: "Tiger II transport track".to_string(),
        mass_kg: 4_600.0,
        hit_points: 175,
        turn_rate_rad_s: 0.40,
        max_load_kg: 72_000.0,
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
            barrel_length_m: 4.8,
            // The production Tiger turret: -8 of depression, +16 of elevation (some sources +15).
            depression_deg: 8.0,
            elevation_deg: 16.0,
            // The Sprgr L/4.5 is the SAME `RoundId` the KwK 43 fires — which is why their HE
            // damage matches and their AP damage does not. Sourcing and GAP notes live with the
            // numbers in `RoundId::spec`.
            shell: RoundId::Pzgr39.spec(),
            special_shell: Some(RoundId::Pzgr40.spec()),
            he_shell: Some(RoundId::SprgrL45.spec()),
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
            barrel_length_m: 6.4,
            // The Serienturm: -8 / +15.
            depression_deg: 8.0,
            elevation_deg: 15.0,
            // HE is the shared L/4.5 `RoundId` — same 300 HP as the Tiger I's, because it is
            // the same shell, which the old multiplier could never say.
            shell: RoundId::Pzgr39_43.spec(),
            special_shell: Some(RoundId::Pzgr40_43.spec()),
            he_shell: Some(RoundId::SprgrL45.spec()),
        },
        mass_kg: 2_300.0,
        hit_points: 150,
    }
}
