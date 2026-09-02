use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ShellType {
    #[default]
    ArmorPiercing,
    Apcr,
    Heat,
    HighExplosive,
}

impl ShellType {
    /// Every shell the guns can chamber. Ammo racks, penetration tables and impact FX are all
    /// per-type, so a type missing here is a type some table forgot.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [ShellType; 4] =
        [ShellType::ArmorPiercing, ShellType::Apcr, ShellType::Heat, ShellType::HighExplosive];
}

/// Full-bore drag form factor, calibrated so the fleet's ten full-bore rounds MEAN ≈ 0.09
/// (the old AP constant): `0.09 / mean(1/SD)` over the authored catalog at B3 time.
const FULL_BORE_DRAG_FORM: f32 = 0.0130;
/// Tungsten-core form factor, same calibration against the old 0.21: light cores bleed speed,
/// and the two lightest (BR-365P, 20-pdr APDS) ride the band's ceiling by design.
const CORE_DRAG_FORM: f32 = 0.0167;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShellSpec {
    #[serde(default)]
    pub shell_type: ShellType,
    pub caliber_mm: f32,
    pub muzzle_velocity_mps: f32,
    pub penetration_mm_at_100m: f32,
    pub damage_hp: u32,
    #[serde(default)]
    pub explosive_radius_m: f32,
    /// The CONCRETE round this spec describes ([`crate::RoundId`]) — `None` only for synthetic
    /// test shells and legacy fixtures. Every fielded gun's slots carry `Some`, locked by
    /// `tests/ammo_identity.rs`.
    #[serde(default)]
    pub round: Option<crate::RoundId>,
    /// Projectile mass as fired, kg (the dossier's mass column). `0.0` = legacy/unspecified —
    /// consumers must fall back to their class behavior, never divide by it.
    #[serde(default)]
    pub mass_kg: f32,
    /// Bursting-charge mass, kg; `0.0` = solid shot (or an unsourced charge, stated in the
    /// catalog). Drives HE pricing (the O-365K anchor law), future APHE behavior, and spall
    /// severity.
    #[serde(default)]
    pub filler_kg: f32,
    /// What does the penetrating ([`crate::Penetrator`]): the terminal-ballistics identity the
    /// armor model will key on. Constructors set the class default; the catalog overrides where
    /// the real round differed (the Soviet blunt-nosed APBC family).
    #[serde(default)]
    pub penetrator: crate::Penetrator,
}

impl ShellSpec {
    /// Physical projectile radius used by continuous collision sweeps. Authored caliber is a
    /// diameter in millimetres; a small lower bound keeps malformed zero-caliber content from
    /// becoming an infinitely thin ray, while the upper bound prevents naval-scale mod content
    /// from inflating every nearby hitbox beyond readable tank-combat behavior.
    pub fn collision_radius_m(self) -> f32 {
        (self.caliber_mm * 0.0005).clamp(0.01, 0.10)
    }

    pub fn armor_piercing(
        caliber_mm: f32,
        muzzle_velocity_mps: f32,
        penetration_mm_at_100m: f32,
        damage_hp: u32,
    ) -> Self {
        Self {
            shell_type: ShellType::ArmorPiercing,
            caliber_mm,
            muzzle_velocity_mps,
            penetration_mm_at_100m,
            damage_hp,
            explosive_radius_m: 0.0,
            round: None,
            mass_kg: 0.0,
            filler_kg: 0.0,
            penetrator: crate::Penetrator::FullBoreSharp,
        }
    }

    pub fn apcr(
        caliber_mm: f32,
        muzzle_velocity_mps: f32,
        penetration_mm_at_100m: f32,
        damage_hp: u32,
    ) -> Self {
        Self {
            shell_type: ShellType::Apcr,
            caliber_mm,
            muzzle_velocity_mps,
            penetration_mm_at_100m,
            damage_hp,
            explosive_radius_m: 0.0,
            round: None,
            mass_kg: 0.0,
            filler_kg: 0.0,
            penetrator: crate::Penetrator::TungstenCore,
        }
    }

    pub fn heat(
        caliber_mm: f32,
        muzzle_velocity_mps: f32,
        penetration_mm_at_100m: f32,
        damage_hp: u32,
    ) -> Self {
        Self {
            shell_type: ShellType::Heat,
            caliber_mm,
            muzzle_velocity_mps,
            penetration_mm_at_100m,
            damage_hp,
            explosive_radius_m: 0.0,
            round: None,
            mass_kg: 0.0,
            filler_kg: 0.0,
            penetrator: crate::Penetrator::ShapedCharge,
        }
    }

    pub fn high_explosive(
        caliber_mm: f32,
        muzzle_velocity_mps: f32,
        penetration_mm_at_100m: f32,
        damage_hp: u32,
        explosive_radius_m: f32,
    ) -> Self {
        Self {
            shell_type: ShellType::HighExplosive,
            caliber_mm,
            muzzle_velocity_mps,
            penetration_mm_at_100m,
            damage_hp,
            explosive_radius_m,
            round: None,
            mass_kg: 0.0,
            filler_kg: 0.0,
            penetrator: crate::Penetrator::BlastCase,
        }
    }

    /// Stamp the concrete identity on a spec — the catalog's half of the promise that
    /// [`crate::RoundId::spec`] is the ONE authoring point for shell data.
    pub fn with_round(mut self, round: crate::RoundId) -> Self {
        self.round = Some(round);
        self
    }

    /// Author the projectile's physical data (catalog use): mass as fired and bursting charge.
    pub fn with_projectile(mut self, mass_kg: f32, filler_kg: f32) -> Self {
        self.mass_kg = mass_kg;
        self.filler_kg = filler_kg;
        self
    }

    /// Override the class-default penetrator (catalog use) — the Soviet APBC family is blunt
    /// where the constructor's plain-AP default is sharp.
    pub fn with_penetrator(mut self, penetrator: crate::Penetrator) -> Self {
        self.penetrator = penetrator;
        self
    }

    /// The steel around the burster: what any future fragmentation model throws.
    pub fn casing_mass_kg(self) -> f32 {
        (self.mass_kg - self.filler_kg).max(0.0)
    }

    /// Whether this round penetrates by BITING — the family the glance band, the ricochet
    /// overmatch escape and perforation continuation apply to.
    pub fn is_kinetic(self) -> bool {
        matches!(
            self.penetrator,
            crate::Penetrator::FullBoreSharp
                | crate::Penetrator::FullBoreBlunt
                | crate::Penetrator::TungstenCore
        )
    }

    /// How many degrees of obliquity the round's nose turns into the plate before the LOS steel
    /// is measured. B5 gave the Soviet APBC family its own row: the blunt nose was BUILT for
    /// sloped plate — it turns 8° into the armor where the sharp APCBC turns 5° — which is why
    /// the BR-412 fights a glacis differently than a Pzgr 39 with the same penetration column.
    pub fn normalization_deg(self) -> f32 {
        match self.penetrator {
            crate::Penetrator::FullBoreSharp => 5.0,
            crate::Penetrator::FullBoreBlunt => 8.0,
            crate::Penetrator::TungstenCore => 2.0,
            crate::Penetrator::ShapedCharge | crate::Penetrator::BlastCase => 0.0,
        }
    }

    /// The angle of incidence past which this round skids instead of biting; `None` never
    /// ricochets (a blast case bursts on whatever it touches). The blunt APBC digs in three
    /// degrees longer than the sharp nose (B5). The kinetic overmatch escape stays the armor
    /// model's business (`armor::resolve`), not the shell's.
    pub fn ricochet_angle_deg(self) -> Option<f32> {
        match self.penetrator {
            crate::Penetrator::FullBoreSharp | crate::Penetrator::TungstenCore => Some(70.0),
            crate::Penetrator::FullBoreBlunt => Some(73.0),
            crate::Penetrator::ShapedCharge => Some(85.0),
            crate::Penetrator::BlastCase => None,
        }
    }

    /// Kinetic energy at an impact speed, kJ (½mv²) — real mass, not a caliber estimate.
    /// Returns 0.0 for a legacy `mass_kg == 0.0` spec; consumers keep their class fallback.
    pub fn impact_energy_kj(self, speed_mps: f32) -> f32 {
        0.0005 * self.mass_kg * speed_mps * speed_mps
    }

    /// The round's recoil momentum at the muzzle, kg·m/s (`mass × muzzle velocity`) — what the
    /// gun throws back into the mount, the hull and the ground. `0.0` for a legacy spec without
    /// a mass; consumers use [`Self::recoil_scale`], which falls back to the reference.
    pub fn recoil_momentum_kg_mps(self) -> f32 {
        self.mass_kg * self.muzzle_velocity_mps
    }

    /// Every channel of the shot's feel scales through THIS ONE number (Inny Poziom S3): the
    /// muzzle flash and its smoke, the dust ring, the barrel's recoil stroke, the hull's rock,
    /// the camera's nudge — where each used to be a constant identical for a 75 mm and a
    /// 128 mm. The reference is the D-10's BR-412 (15.7 kg at 895 m/s), so the T-54's shot
    /// keeps the numbers every channel was tuned on; the square root keeps the 12.8 cm Pak 80
    /// (1.9× the momentum) at ~1.36× and the 7.5 cm KwK 42 (0.45×) at ~0.67× — felt, not
    /// cartoonish — inside a clamp no fielded gun reaches.
    pub fn recoil_scale(self) -> f32 {
        const REFERENCE_MOMENTUM_KG_MPS: f32 = 15.7 * 895.0;
        let momentum = self.recoil_momentum_kg_mps();
        if momentum <= 0.0 {
            return 1.0;
        }
        (momentum / REFERENCE_MOMENTUM_KG_MPS).sqrt().clamp(0.5, 1.8)
    }

    /// Linear aerodynamic drag, in speed lost per second of flight. With `dv/dt = -c·v` a shell
    /// loses speed LINEARLY with distance (`v(s) = v0 − c·s`), so the flight integration
    /// ([`crate::math::integrate_shell_step`]), the HUD's penetration readout, and the server's
    /// impact math all agree in closed form.
    ///
    /// Since Amunicja 3.0 B3 a kinetic round's drag comes from its OWN body: a per-class form
    /// factor over the sectional density (mass per bore area) — a heavy 12.8 cm shell carries
    /// its speed, a light tungsten core sheds it. The form factors are calibrated so the fleet's
    /// class MEANS land on the old constants (AP ≈ 0.09, APCR ≈ 0.21), and the class band clamp
    /// keeps every round readable as its class: a concrete shell may fly a little flatter or
    /// bleed a little faster, it may not impersonate another class. Chemical rounds keep the
    /// flat class constant (they never cared about speed), and a legacy `mass_kg == 0.0` spec
    /// keeps the old constants exactly — synthetic test shells and old fixtures do not move.
    pub fn drag_per_s(self) -> f32 {
        if self.mass_kg <= 0.0 {
            return match self.shell_type {
                ShellType::ArmorPiercing => 0.09,
                ShellType::Apcr => 0.21,
                ShellType::Heat | ShellType::HighExplosive => 0.05,
            };
        }
        match self.penetrator {
            crate::Penetrator::FullBoreSharp | crate::Penetrator::FullBoreBlunt => {
                (FULL_BORE_DRAG_FORM / self.sectional_density_kg_cm2()).clamp(0.07, 0.12)
            }
            crate::Penetrator::TungstenCore => {
                (CORE_DRAG_FORM / self.sectional_density_kg_cm2()).clamp(0.17, 0.24)
            }
            crate::Penetrator::ShapedCharge | crate::Penetrator::BlastCase => 0.05,
        }
    }

    /// Mass per unit bore area, kg/cm² — the ballistic "carry" of the projectile. Meaningful
    /// only for `mass_kg > 0.0` specs; [`Self::drag_per_s`] guards the legacy case.
    pub fn sectional_density_kg_cm2(self) -> f32 {
        let bore_cm = (self.caliber_mm * 0.1).max(0.1);
        self.mass_kg / (bore_cm * bore_cm)
    }

    /// Impact speed after `distance_m` of flight — the closed form of the linear-drag flight.
    pub fn speed_mps_at_distance(self, distance_m: f32) -> f32 {
        (self.muzzle_velocity_mps - self.drag_per_s() * distance_m.max(0.0))
            .max(self.muzzle_velocity_mps * 0.2)
    }

    /// Kinetic penetration falls out of the impact VELOCITY (a De Marre-style power of the
    /// speed ratio), not out of a separate distance table: one physics for the arc you see and
    /// the armor math that resolves it. Chemical energy does not care how fast it arrived.
    pub fn penetration_mm_at_distance(self, distance_m: f32) -> f32 {
        match self.shell_type {
            ShellType::Heat | ShellType::HighExplosive => self.penetration_mm_at_100m,
            ShellType::ArmorPiercing | ShellType::Apcr => {
                let reference = self.speed_mps_at_distance(100.0);
                if reference <= 0.0 {
                    return self.penetration_mm_at_100m;
                }
                let ratio = (self.speed_mps_at_distance(distance_m) / reference).clamp(0.2, 1.1);
                self.penetration_mm_at_100m * ratio.powf(1.5)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GunSpec {
    pub name: String,
    pub reload_seconds: f32,
    pub dispersion_mrad: f32,
    #[serde(default = "default_aim_time_seconds")]
    pub aim_time_seconds: f32,
    #[serde(default = "default_movement_bloom_mrad")]
    pub movement_bloom_mrad: f32,
    #[serde(default = "default_shot_bloom_mrad")]
    pub shot_bloom_mrad: f32,
    #[serde(default = "default_max_dispersion_mrad")]
    pub max_dispersion_mrad: f32,
    /// Exposed barrel length (m). Drives the gun silhouette and the muzzle (shell spawn) so a
    /// longer-barrelled gun visibly reaches further and fires from its real tip.
    #[serde(default = "default_barrel_length_m")]
    pub barrel_length_m: f32,
    /// How far the gun DEPRESSES below horizontal, in degrees (a positive number: 5.0 means
    /// -5 deg). This is a property of the gun in its mount — how much room the breech has
    /// under the turret roof before it fouls the ring — and it is one of the sharpest
    /// balance levers a tank has: depression is what lets a hull sit behind a crest with only
    /// its turret showing.
    ///
    /// The whole fleet used to share one hard-coded -8 deg / +20 deg pair, so the T-54 (a tank
    /// notorious for its poor -5) played like a British hull-down specialist.
    #[serde(default = "default_gun_depression_deg")]
    pub depression_deg: f32,
    /// How far the gun ELEVATES above horizontal, in degrees.
    #[serde(default = "default_gun_elevation_deg")]
    pub elevation_deg: f32,
    /// How fast the gun elevates and depresses (rad/s): the gunner's handwheel or power
    /// elevation, per gun (Inny Poziom A12). The whole fleet used to share one sim constant,
    /// 0.5 rad/s = 28.7 deg/s, slower than every hull's pitch rate; the default keeps old
    /// fixtures loading at that value.
    #[serde(default = "default_gun_elevation_rate_rad_s")]
    pub elevation_rate_rad_s: f32,
    pub shell: ShellSpec,
    /// The gun's AUTHORED special round for rack slot 1 — the second round this weapon actually
    /// fielded, when it fielded one.
    ///
    /// `None` now means what it says: **this gun had no second round**, so it carries two slots
    /// rather than three. It used to mean "derive a generic APCR from the stock shell", which
    /// handed the 12.8 cm Pak 80 and the 122 mm D-25T tungsten rounds neither ever fired — the
    /// no-clones rule broken by arithmetic instead of by copying. See `docs/ammunition.md`.
    #[serde(default)]
    pub special_shell: Option<ShellSpec>,
    /// The gun's AUTHORED high-explosive round.
    ///
    /// Optional only for wire compatibility; every gun in the catalog authors one and
    /// `every_gun_authors_its_high_explosive_round` holds them to it. It was derived from the
    /// stock AP round until 2026-08-02 — `x 0.70` velocity, `x 0.35` penetration, `x 1.4` damage —
    /// which made an 84 mm gun's HE the best-penetrating shell in the game and flew the D-10's HE
    /// at 626 m/s when the real round leaves the muzzle at 900, FASTER than its own AP.
    #[serde(default)]
    pub he_shell: Option<ShellSpec>,
}

/// The fleet's historical default until each vehicle's dossier states its own arc: the pair
/// the whole roster shared as `MIN_GUN_PITCH_RAD` / `MAX_GUN_PITCH_RAD`.
fn default_gun_depression_deg() -> f32 {
    8.0
}

fn default_gun_elevation_deg() -> f32 {
    20.1
}

fn default_gun_elevation_rate_rad_s() -> f32 {
    0.5
}

fn default_barrel_length_m() -> f32 {
    5.0
}

impl GunSpec {
    /// Shells the player can load for this gun — sidegrades, not strict upgrades, and every one of
    /// them AUTHORED. Nothing here is computed from anything else: a round's velocity, penetration
    /// and damage belong to that round, and deriving one shell from another is how an 84 mm gun
    /// ended up with the best high-explosive penetration in the game.
    ///
    /// Slot 0 is the stock armour-piercing round. Slot 1 is the second round this gun actually
    /// fielded, and a gun that fielded none has no slot 1 — the count is a property of the weapon.
    /// The last slot is high explosive. No economy: every round is freely selectable, and the
    /// chosen shell is what the tank fires.
    pub fn ammo_options(&self) -> Vec<ShellSpec> {
        let mut options = vec![self.shell];
        options.extend(self.special_shell);
        options.extend(self.he_shell);
        options
    }

    /// How many ammo slots this gun actually fields — `ammo_options().len()` without building the
    /// Vec. A gun that fielded no special or HE round has fewer than `MAX_AMMO_SLOTS`; selecting a
    /// slot at or beyond this count is a phantom the loader cannot serve, and must be refused rather
    /// than accepted (which would restart the reload for an always-empty slot and jam the gun).
    pub fn ammo_slot_count(&self) -> usize {
        1 + usize::from(self.special_shell.is_some()) + usize::from(self.he_shell.is_some())
    }
}

const fn default_aim_time_seconds() -> f32 {
    2.4
}

const fn default_movement_bloom_mrad() -> f32 {
    4.0
}

const fn default_shot_bloom_mrad() -> f32 {
    3.5
}

const fn default_max_dispersion_mrad() -> f32 {
    16.0
}

#[cfg(test)]
mod tests {
    use crate::{Penetrator, ShellType, VehicleKind};

    /// The terminal table, locked per penetrator (B4): the constants moved from the armor
    /// model onto the shell, so the lock moves with them. B5 may CHANGE a row deliberately —
    /// this test is where that change becomes a diff instead of a drift.
    #[test]
    fn the_terminal_table_is_locked_per_penetrator() {
        let spec_with = |penetrator| {
            crate::ShellSpec::armor_piercing(100.0, 900.0, 200.0, 320).with_penetrator(penetrator)
        };
        let mut rows = 0;
        for penetrator in Penetrator::ALL {
            let spec = spec_with(penetrator);
            let expected = match penetrator {
                Penetrator::FullBoreSharp => (5.0, Some(70.0), true),
                // B5's deliberate diff: the blunt APBC nose turns 8° into the slope and digs
                // in to 73° — the Soviet sloped-armor round finally fights like one.
                Penetrator::FullBoreBlunt => (8.0, Some(73.0), true),
                Penetrator::TungstenCore => (2.0, Some(70.0), true),
                Penetrator::ShapedCharge => (0.0, Some(85.0), false),
                Penetrator::BlastCase => (0.0, None, false),
            };
            assert_eq!(
                (spec.normalization_deg(), spec.ricochet_angle_deg(), spec.is_kinetic()),
                expected,
                "{penetrator:?}: the terminal row moved without a deliberate diff"
            );
            rows += 1;
        }
        assert_eq!(rows, 5, "every penetrator owns a locked row");
    }

    #[test]
    fn the_d10_family_loads_its_authored_heat_and_it_ignores_range() {
        // The BK-5 activates ShellType::Heat end-to-end: the T-54's special slot is the authored
        // HEAT round, not the derived APCR — and chemical penetration is flat with distance,
        // where the kinetic rounds bleed theirs (the armour model already prices the trade:
        // spaced screens kill the jet, extreme obliquity sheds it).
        let options = VehicleKind::T54_1951.spec().gun.ammo_options();
        let special = options[1];
        assert_eq!(special.shell_type, ShellType::Heat, "the T-54's slot 1 is the BK-5");
        assert_eq!(
            special.penetration_mm_at_distance(50.0),
            special.penetration_mm_at_distance(900.0),
            "HEAT penetration must not care about range"
        );
        let stock = options[0];
        assert!(
            stock.penetration_mm_at_distance(900.0) < stock.penetration_mm_at_100m,
            "the kinetic stock round still bleeds penetration downrange"
        );
        assert!(
            special.penetration_mm_at_100m > stock.penetration_mm_at_100m,
            "the BK-5 out-penetrates the AP round point blank too"
        );
        // A gun with no authored special round keeps the derived APCR — nothing regresses.
        let tiger = VehicleKind::TigerII.spec().gun.ammo_options();
        assert_eq!(tiger[1].shell_type, ShellType::Apcr, "unauthored guns keep the APCR slot");
    }

    #[test]
    fn ammo_options_offer_distinct_rounds_with_apcr_out_penetrating_ap() {
        let gun = VehicleKind::TigerII.spec().gun;
        let options = gun.ammo_options();
        assert!(options.len() >= 3, "stock + APCR + HE");
        assert_eq!(options[0], gun.shell, "first option is the stock round");
        assert!(
            options[1].penetration_mm_at_100m > options[0].penetration_mm_at_100m,
            "APCR out-penetrates the stock AP round"
        );
        // Sidegrade, not strict upgrade: APCR gives up alpha for that penetration.
        assert!(options[1].damage_hp < options[0].damage_hp, "APCR trades away alpha");
        // HE gives up penetration for BLAST — splash, tracks, a finisher — and on a high-velocity
        // anti-tank gun that is a 9.4 kg shell with 0.87 kg of filler, so it lands BELOW the
        // armour-piercing round's damage rather than above it. The assertion here used to be
        // `HE > AP`, which was true only because HE was AP x 1.4; the powerful-HE case belongs to
        // guns built for it (large caliber, low velocity), and this roster has none yet.
        assert!(options[2].penetration_mm_at_100m < options[0].penetration_mm_at_100m);
        assert!(options[2].explosive_radius_m > 0.0, "HE is the round that bursts");
        let kinds: std::collections::HashSet<_> = options.iter().map(|s| s.shell_type).collect();
        assert!(kinds.len() >= 3, "the rounds are of distinct shell types");
    }

    #[test]
    fn ammo_slot_count_matches_the_options_length_across_the_fleet() {
        // The cheap count feeds the ammo-switch guard (`state::advance`), so it must equal the
        // authoritative `ammo_options().len()` for every vehicle — a guard that over-counts lets a
        // player select a phantom, always-empty slot and jam the reload; one that under-counts
        // hides a real round. It also may never exceed the storage the counts array has.
        for kind in VehicleKind::ALL {
            let gun = kind.spec().gun;
            assert_eq!(
                gun.ammo_slot_count(),
                gun.ammo_options().len(),
                "{kind:?}: ammo_slot_count drifted from ammo_options()"
            );
            assert!(
                gun.ammo_slot_count() <= crate::MAX_AMMO_SLOTS,
                "{kind:?}: more ammo slots than the counts array can hold"
            );
        }
    }
}
