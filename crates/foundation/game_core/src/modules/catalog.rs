use super::catalog_british::{centurion_loadout, gun_20pdr_type_b};
use super::catalog_german::{
    gun_kwk36, gun_kwk43, tiger_i_engine_hl210, tiger_i_loadout, tiger_i_transport_track,
    tiger_ii_loadout, tiger_ii_transport_track,
};
use super::catalog_misc::{
    gun_kwk42, gun_pak43_l71, gun_pak80, gun_prototype, jagdtiger_loadout,
    jagdtiger_transport_track, panther_loadout, panther_transport_track, prototype_loadout,
};
use super::catalog_soviet::{
    gun_d10t, gun_d10t2s, gun_d25t, is3_engine_v54k, is3_loadout, kv1_loadout, t34_85_loadout,
    t54_engine_v55, t54_loadout,
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
            VehicleKind::TigerI => tiger_i_loadout(),
            VehicleKind::TigerII => tiger_ii_loadout(),
            VehicleKind::Jagdtiger => jagdtiger_loadout(),
            VehicleKind::PantherII => panther_loadout(),
            VehicleKind::IS3 => is3_loadout(),
            VehicleKind::Centurion => centurion_loadout(),
            VehicleKind::T34_85 => t34_85_loadout(),
            VehicleKind::KV1_1942 => kv1_loadout(),
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
            VehicleKind::TigerI => vec![gun_kwk36()],
            VehicleKind::TigerII => vec![gun_kwk43()],
            // The actually-fielded "88 Jagdtiger": the long 88 as a DPM/handling trade against the
            // 12.8 cm's alpha (both fit the casemate's 130 mm caliber limit).
            VehicleKind::Jagdtiger => vec![gun_pak80(), gun_pak43_l71()],
            VehicleKind::PantherII => vec![gun_kwk42()],
            VehicleKind::IS3 => vec![gun_d25t()],
            // The two fielded 20-pounder barrels: the original Type A and the fume-extractor
            // Type B — same ballistics, a handling/reload sidegrade.
            VehicleKind::Centurion => {
                let stock = self.default_loadout().gun;
                vec![stock, gun_20pdr_type_b()]
            }
            // One service gun: the ZiS-S-53 was THE T-34-85; the earlier D-5T is a museum piece.
            VehicleKind::T34_85 => vec![self.default_loadout().gun],
            // One service gun: the ZiS-5 defines this model. The earlier F-32 belongs to the
            // mod. 1940, a different vehicle -- offering it here would be a synthetic sidegrade,
            // which this catalog forbids.
            VehicleKind::KV1_1942 => vec![self.default_loadout().gun],
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

    /// Engines (first is stock). Authored per-vehicle historical alternates, not a synthetic
    /// multiplier: only vehicles that genuinely had a second powerplant offer one, and each is a
    /// real trade (the early Tiger's lighter-but-weaker HL210, the T-54's V-55 retrofit, the IS-3M's
    /// V-54K). Vehicles with one service engine list only it — honest over a fake blanket uprate.
    pub fn engine_options(self) -> Vec<EngineModule> {
        let stock = self.default_loadout().engine;
        match self {
            VehicleKind::T54_1951 => vec![stock, t54_engine_v55()],
            VehicleKind::TigerI => vec![stock, tiger_i_engine_hl210()],
            VehicleKind::IS3 => vec![stock, is3_engine_v54k()],
            _ => vec![stock],
        }
    }

    /// Running gear (first is stock). The real period choice was track WIDTH: the German heavies
    /// carried a narrow rail-loading track (Verladekette) alongside the wide combat track
    /// (Gefechtskette). The transport track is lighter but traverses worse and carries less — a
    /// genuine trade. Vehicles without a documented second track list only their stock gear.
    pub fn suspension_options(self) -> Vec<SuspensionModule> {
        let stock = self.default_loadout().suspension;
        match self {
            VehicleKind::TigerI => vec![stock, tiger_i_transport_track()],
            VehicleKind::TigerII => vec![stock, tiger_ii_transport_track()],
            VehicleKind::Jagdtiger => vec![stock, jagdtiger_transport_track()],
            VehicleKind::PantherII => vec![stock, panther_transport_track()],
            _ => vec![stock],
        }
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
            assert_eq!(kind.engine_options()[0], default.engine, "{kind:?} stock engine first");
            assert_eq!(
                kind.suspension_options()[0],
                default.suspension,
                "{kind:?} stock gear first"
            );
            assert_eq!(kind.radio_options()[0], default.radio);

            // Every engine option (authored, real variants — never a copy of the stock) still
            // assembles to a valid spec.
            let engines = kind.engine_options();
            for (i, engine) in engines.iter().enumerate() {
                if i > 0 {
                    assert_ne!(
                        *engine, engines[0],
                        "{kind:?} alternate engine must differ from stock"
                    );
                }
                let mut modules = kind.default_loadout();
                modules.engine = engine.clone();
                assert!(modules.assemble(kind).engine_power_kw > 0.0);
            }
            // Alternate running gear and guns are likewise distinct authored variants.
            let suspensions = kind.suspension_options();
            for susp in &suspensions[1..] {
                assert_ne!(
                    *susp, suspensions[0],
                    "{kind:?} alternate track must differ from stock"
                );
                assert!(susp.turn_rate_rad_s > 0.0 && susp.max_load_kg > 0.0);
            }
            let guns = kind.gun_options();
            for gun in &guns[1..] {
                assert_ne!(*gun, guns[0], "{kind:?} alternate gun must differ from stock");
            }
        }
    }

    #[test]
    fn authored_alternates_are_real_trades_not_formula_upgrades() {
        // T-54: the V-55 retrofit genuinely out-powers the V-54.
        let t54_engines = VehicleKind::T54_1951.engine_options();
        assert_eq!(t54_engines.len(), 2);
        assert!(t54_engines[1].power_kw > t54_engines[0].power_kw, "V-55 out-powers the V-54");

        // Tiger I: the early HL210 is a sidegrade — LESS power for LESS weight, not a strict win.
        let tiger_engines = VehicleKind::TigerI.engine_options();
        assert!(tiger_engines[1].power_kw < tiger_engines[0].power_kw, "HL210 gives up power");
        assert!(tiger_engines[1].mass_kg < tiger_engines[0].mass_kg, "...for less weight");

        // Tiger II: the transport track trades traverse for weight (worse turn, lighter).
        let tiger2_susp = VehicleKind::TigerII.suspension_options();
        assert_eq!(tiger2_susp.len(), 2);
        assert!(
            tiger2_susp[1].turn_rate_rad_s < tiger2_susp[0].turn_rate_rad_s,
            "the transport track traverses worse than the combat track"
        );
        assert!(tiger2_susp[1].mass_kg < tiger2_susp[0].mass_kg, "...but weighs less");

        // Jagdtiger: the fielded 88 reloads faster but hits softer than the 12.8 cm.
        let jt_guns = VehicleKind::Jagdtiger.gun_options();
        assert_eq!(jt_guns.len(), 2);
        assert!(
            jt_guns[1].spec.reload_seconds < jt_guns[0].spec.reload_seconds,
            "the 88 reloads faster than the 12.8 cm"
        );
        assert!(
            jt_guns[1].spec.shell.damage_hp < jt_guns[0].spec.shell.damage_hp,
            "...for less alpha"
        );

        // A vehicle with one service engine / one track lists exactly one — no fake blanket uprate.
        assert_eq!(VehicleKind::TigerII.engine_options().len(), 1, "one service engine only");
        assert_eq!(VehicleKind::IS3.suspension_options().len(), 1, "one running gear only");
    }
}
