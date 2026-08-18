//! The concrete-round catalog: every shell the fleet's guns chamber, as MANUFACTURED.
//!
//! `ShellType` says what CLASS a round is; this file says WHICH round it is — the BR-412D is not
//! "an AP shell", it is a specific 100 mm projectile with its own mass, velocity and behavior,
//! and two guns that fired the same physical round (the KwK 36 and KwK 43 sharing one Sprgr
//! L/4.5, the whole D-10 family sharing one BK-5) share ONE identity here, which no per-gun
//! anonymous spec could express. Every number cites `docs/ammunition.md` (the Ammunition 2.3
//! research pass); rows the dossier marks GAP are stated balance decisions in the comment beside
//! them, exactly as the gun catalogs already record.

use serde::{Deserialize, Serialize};

use crate::ShellSpec;

/// What does the penetrating: type, material and nose form folded into the five combinations
/// the fleet actually fields. Separate material/nose axes would be pure combinatorics at 25
/// rounds — split them only if a round ever needs a sixth combination.
///
/// Append-only (asset identity — serialized inside `ShellSpec` into the vehicle snapshots).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Penetrator {
    /// Sharp or capped full-bore steel — the German APCBC family. The default a legacy spec
    /// deserializes to, because the fleet's plain-AP behavior is this one.
    #[default]
    FullBoreSharp,
    /// Blunt-nosed full-bore steel — the Soviet APBC family (BR-412/BR-471B/BR-365K), built
    /// for sloped-plate fighting.
    FullBoreBlunt,
    /// Tungsten-carbide subcaliber core: APCR composite-rigid and APDS alike.
    TungstenCore,
    /// Shaped-charge jet (HEAT).
    ShapedCharge,
    /// Thin-walled blast/fragmentation case (HE-Frag).
    BlastCase,
}

impl Penetrator {
    /// Locked variant-by-variant against the declaration by `quality`.
    pub const ALL: [Penetrator; 5] = [
        Penetrator::FullBoreSharp,
        Penetrator::FullBoreBlunt,
        Penetrator::TungstenCore,
        Penetrator::ShapedCharge,
        Penetrator::BlastCase,
    ];
}

/// A CONCRETE round — the shell as manufactured, not its class.
///
/// Append-only: the id is stored in the generated vehicle assets and (from wire v47) rides the
/// wire, so variants append and never reorder — the same rule as [`crate::ShellType`]. Named
/// `RoundId` because `ShellId` is the projectile-INSTANCE id (`crate::ShellId`): one names a
/// kind of ammunition, the other names a shot in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoundId {
    // 100 mm D-10T / D-10T2S — T-54. The gun family is one tube; the stock rounds differ
    // (BR-412 vs the improved BR-412D), the chemical and HE rounds are shared.
    Br412,
    Br412D,
    Bk5,
    Of412,
    // 85 mm ZiS-S-53 — T-34-85.
    Br365K,
    Br365P,
    O365K,
    // 122 mm D-25T — IS-3. No tungsten round was ever fielded: two identities, not three.
    Br471B,
    Of471,
    // 8.8 cm KwK 36 L/56 — Tiger I. The Sprgr L/4.5 is the SAME shell the KwK 43 fires.
    Pzgr39,
    Pzgr40,
    SprgrL45,
    // 8.8 cm KwK 43 L/71 / Pak 43/3 L/71 — Tiger II and the "88 Jagdtiger": the same weapon in
    // two mounts, so the same rounds.
    Pzgr39_43,
    Pzgr40_43,
    // 12.8 cm Pak 80 L/55 — Jagdtiger. The tungsten shortage is a fact about this gun: no APCR.
    Pzgr43,
    SprgrPak80,
    // 7.5 cm KwK 42 L/70 — Panther II.
    Pzgr39_42,
    Pzgr40_42,
    Sprgr42,
    // 84 mm 20-pounder — Centurion. APDS was the NORMAL load, APCBC "rarely used" — the inverted
    // stock/special relationship is this vehicle's own identity.
    TwentyPdrApcbc,
    TwentyPdrApds,
    TwentyPdrHe,
    // 120 mm Prototype — fictional test vehicle; every number invented by definition and says so.
    Prototype120Ap,
    Prototype120Apcr,
    Prototype120He,
}

impl RoundId {
    /// Every round the fleet chambers.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [RoundId; 25] = [
        RoundId::Br412,
        RoundId::Br412D,
        RoundId::Bk5,
        RoundId::Of412,
        RoundId::Br365K,
        RoundId::Br365P,
        RoundId::O365K,
        RoundId::Br471B,
        RoundId::Of471,
        RoundId::Pzgr39,
        RoundId::Pzgr40,
        RoundId::SprgrL45,
        RoundId::Pzgr39_43,
        RoundId::Pzgr40_43,
        RoundId::Pzgr43,
        RoundId::SprgrPak80,
        RoundId::Pzgr39_42,
        RoundId::Pzgr40_42,
        RoundId::Sprgr42,
        RoundId::TwentyPdrApcbc,
        RoundId::TwentyPdrApds,
        RoundId::TwentyPdrHe,
        RoundId::Prototype120Ap,
        RoundId::Prototype120Apcr,
        RoundId::Prototype120He,
    ];

    /// The round's full authored spec — the ONE authoring point for shell data. The gun catalogs
    /// (`modules::catalog_*`) pull from here; a gun stating its own numbers is the drift this
    /// file exists to end.
    pub fn spec(self) -> ShellSpec {
        // Mass and filler come from the dossier's columns; where a row is GAP, the value here is
        // a stated BALANCE decision and the comment says which. HE fillers with no source are
        // BACK-DERIVED from the authored damage through the anchor law (`O-365K`: 0.741 kg →
        // 300 HP, cube-root blast scaling) — the same law forward pricing used, run in reverse,
        // so the filler column and the damage column can never disagree.
        let spec =
            match self {
                // ——— 100 mm D-10 family ———
                // BR-412 APBC, 15.7 kg at 895 m/s with 0.6 kg of TNT (sourced, high confidence) —
                // a genuine Soviet APHE identity, blunt-nosed for sloped plate.
                RoundId::Br412 => ShellSpec::armor_piercing(100.0, 895.0, 185.0, 320)
                    .with_projectile(15.7, 0.6)
                    .with_penetrator(Penetrator::FullBoreBlunt),
                // BR-412D: the improved APBC the D-10T2S loads — flatter arc through better shape,
                // more penetration, less per-shot alpha (a sidegrade, not an upgrade). Not massed
                // separately in the dossier: same shell body, so the BR-412's 15.7/0.6 (decision).
                RoundId::Br412D => ShellSpec::armor_piercing(100.0, 895.0, 195.0, 300)
                    .with_projectile(15.7, 0.6)
                    .with_penetrator(Penetrator::FullBoreBlunt),
                // BK-5 HEAT: 280 mm flat with range; a spaced screen kills the jet. Mass ~12.2 kg
                // (decision — the dossier carries no mass column for it); the shaped charge is not
                // a bursting charge, so filler stays 0.0 until a source lands.
                RoundId::Bk5 => {
                    ShellSpec::heat(100.0, 900.0, 280.0, 320).with_projectile(12.2, 0.0)
                }
                // OF-412, fully sourced: 15.8 kg at 900 m/s with 2.16 kg of TNT — FASTER than its
                // own AP round, the finding that killed the ×0.70 HE derivation.
                RoundId::Of412 => ShellSpec::high_explosive(100.0, 900.0, 33.0, 430, 2.0)
                    .with_projectile(15.8, 2.16),
                // ——— 85 mm ZiS-S-53 ———
                // BR-365K: 9.2 kg with 0.16 kg RDX-Al (sourced, high confidence).
                RoundId::Br365K => ShellSpec::armor_piercing(85.0, 792.0, 145.0, 200)
                    .with_projectile(9.2, 0.16)
                    .with_penetrator(Penetrator::FullBoreBlunt),
                // BR-365P APCR, 4.95 kg of tungsten at a sourced 1,050 m/s.
                RoundId::Br365P => {
                    ShellSpec::apcr(85.0, 1_050.0, 170.0, 170).with_projectile(4.95, 0.0)
                }
                // O-365K: 9.54 kg at 793 m/s with 0.741 kg of TNT — the fleet's HE damage anchor
                // (300 HP; every other gun's HE scales from here by cube-root of filler mass).
                RoundId::O365K => ShellSpec::high_explosive(85.0, 793.0, 28.0, 300, 1.6)
                    .with_projectile(9.54, 0.741),
                // ——— 122 mm D-25T ———
                // BR-471B: 25.0 kg with 0.156 kg RDX-Al (sourced).
                RoundId::Br471B => ShellSpec::armor_piercing(122.0, 795.0, 175.0, 390)
                    .with_projectile(25.0, 0.156)
                    .with_penetrator(Penetrator::FullBoreBlunt),
                // OF-471: 25.53 kg with 3.605 kg of TNT (sourced); ~800 m/s is convention, marked
                // GAP in the dossier. 510 HP is the 85 mm anchor scaled by filler.
                RoundId::Of471 => ShellSpec::high_explosive(122.0, 800.0, 41.0, 510, 2.4)
                    .with_projectile(25.53, 3.605),
                // ——— 8.8 cm KwK 36 L/56 ———
                // Pzgr 39: 10.2 kg; filler mirrors the sourced 39/43's 59 g Amatol — same shell
                // family (decision).
                RoundId::Pzgr39 => {
                    ShellSpec::armor_piercing(88.0, 773.0, 165.0, 360).with_projectile(10.2, 0.059)
                }
                // Pzgr 40: the round existed and its 100 m figure did not survive into any sourced
                // table (dossier GAP) — velocity and penetration are a BALANCE decision between the
                // KwK 36's own AP and the KwK 43's sourced 40/43. Mass 7.3 kg is attested.
                RoundId::Pzgr40 => {
                    ShellSpec::apcr(88.0, 930.0, 217.0, 320).with_projectile(7.3, 0.0)
                }
                // Sprgr Patr L/4.5: 9.4 kg, 750 m/s, 0.870 kg of filler — fired by BOTH 88s, which
                // is why their HE damage matches and their AP damage does not.
                RoundId::SprgrL45 => ShellSpec::high_explosive(88.0, 750.0, 29.0, 300, 1.6)
                    .with_projectile(9.4, 0.870),
                // ——— 8.8 cm KwK 43 L/71 / Pak 43/3 ———
                // Pzgr 39/43: 10.4 kg with 59 g Amatol (sourced, high confidence).
                RoundId::Pzgr39_43 => ShellSpec::armor_piercing(88.0, 1_000.0, 202.0, 390)
                    .with_projectile(10.4, 0.059),
                // Pzgr 40/43 at its sourced 1,130 m/s and 7.3 kg; the 100 m penetration is a
                // balance decision (the sourced tables start at 500 m).
                RoundId::Pzgr40_43 => {
                    ShellSpec::apcr(88.0, 1_130.0, 237.0, 330).with_projectile(7.3, 0.0)
                }
                // ——— 12.8 cm Pak 80 L/55 ———
                // Pzgr 43: 28.3 kg (sourced); its APHE burster is unsourced — 0.55 kg (decision,
                // scaled from the 88's shell-to-burster proportion).
                RoundId::Pzgr43 => {
                    ShellSpec::armor_piercing(128.0, 920.0, 223.0, 530).with_projectile(28.3, 0.55)
                }
                // The 28 kg HE shell is sourced; velocity is a dossier GAP filled as a decision and
                // the filler is back-derived from the authored 520 HP through the anchor law.
                RoundId::SprgrPak80 => ShellSpec::high_explosive(128.0, 750.0, 43.0, 520, 2.2)
                    .with_projectile(28.0, 3.86),
                // ——— 7.5 cm KwK 42 L/70 ———
                // Pzgr 39/42: 6.8 kg (sourced); the 18 g burster is the German small-caliber APHE
                // proportion (decision).
                RoundId::Pzgr39_42 => {
                    ShellSpec::armor_piercing(75.0, 935.0, 138.0, 240).with_projectile(6.8, 0.018)
                }
                // Pzgr 40/42, fully sourced: 4.75 kg at 1,120 m/s for 194 mm at 100 m.
                RoundId::Pzgr40_42 => {
                    ShellSpec::apcr(75.0, 1_120.0, 194.0, 200).with_projectile(4.75, 0.0)
                }
                // Sprgr 42: 5.74 kg at 700 m/s (sourced); filler back-derived from the authored
                // 250 HP through the anchor law.
                RoundId::Sprgr42 => ShellSpec::high_explosive(75.0, 700.0, 25.0, 250, 1.4)
                    .with_projectile(5.74, 0.429),
                // ——— 84 mm 20-pounder ———
                // The gun's NAME is the shot's mass: a 20-pound (9.07 kg) solid AP projectile
                // (decision — the dossier's mass row is a GAP, but the designation is not).
                RoundId::TwentyPdrApcbc => {
                    ShellSpec::armor_piercing(84.0, 1_020.0, 230.0, 240).with_projectile(9.07, 0.0)
                }
                // APDS at a sourced 1,465 m/s — the gun's NORMAL load. The flying tungsten
                // sub-projectile is ~4.5 kg (decision; the sabot is left at the muzzle).
                RoundId::TwentyPdrApds => {
                    ShellSpec::apcr(84.0, 1_465.0, 300.0, 220).with_projectile(4.5, 0.0)
                }
                // HE attested, numbers not: velocity, mass and filler are GAPs — mass at the 84 mm
                // class (decision), filler back-derived from the authored 290 HP.
                RoundId::TwentyPdrHe => ShellSpec::high_explosive(84.0, 850.0, 28.0, 290, 1.6)
                    .with_projectile(9.0, 0.669),
                // ——— 120 mm Prototype (fictional — every number invented by definition) ———
                RoundId::Prototype120Ap => {
                    ShellSpec::armor_piercing(120.0, 900.0, 250.0, 390).with_projectile(25.0, 0.0)
                }
                RoundId::Prototype120Apcr => {
                    ShellSpec::apcr(120.0, 1_000.0, 300.0, 330).with_projectile(12.0, 0.0)
                }
                RoundId::Prototype120He => ShellSpec::high_explosive(120.0, 800.0, 40.0, 480, 2.0)
                    .with_projectile(26.0, 3.04),
            };
        spec.with_round(self)
    }

    /// Manufacturer designation, for the garage, the reticle tooltip and the killfeed: the
    /// player is told WHICH round bounced, not which class.
    pub fn designation(self) -> &'static str {
        match self {
            RoundId::Br412 => "BR-412",
            RoundId::Br412D => "BR-412D",
            RoundId::Bk5 => "BK-5",
            RoundId::Of412 => "OF-412",
            RoundId::Br365K => "BR-365K",
            RoundId::Br365P => "BR-365P",
            RoundId::O365K => "O-365K",
            RoundId::Br471B => "BR-471B",
            RoundId::Of471 => "OF-471",
            RoundId::Pzgr39 => "Pzgr. 39",
            RoundId::Pzgr40 => "Pzgr. 40",
            RoundId::SprgrL45 => "Sprgr. L/4.5",
            RoundId::Pzgr39_43 => "Pzgr. 39/43",
            RoundId::Pzgr40_43 => "Pzgr. 40/43",
            RoundId::Pzgr43 => "Pzgr. 43",
            RoundId::SprgrPak80 => "12.8 cm Sprgr.",
            RoundId::Pzgr39_42 => "Pzgr. 39/42",
            RoundId::Pzgr40_42 => "Pzgr. 40/42",
            RoundId::Sprgr42 => "Sprgr. 42",
            RoundId::TwentyPdrApcbc => "20-pdr AP Mk. 1",
            RoundId::TwentyPdrApds => "20-pdr APDS Mk. 1",
            RoundId::TwentyPdrHe => "20-pdr HE Mk. 1",
            RoundId::Prototype120Ap => "120 mm AP (prototype)",
            RoundId::Prototype120Apcr => "120 mm APCR (prototype)",
            RoundId::Prototype120He => "120 mm HE (prototype)",
        }
    }
}
