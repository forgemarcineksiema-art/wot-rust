use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ShellType {
    #[default]
    ArmorPiercing,
    Apcr,
    Heat,
    HighExplosive,
}

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
        }
    }

    /// Linear aerodynamic drag, in speed lost per second of flight. With `dv/dt = -c·v` a shell
    /// loses speed LINEARLY with distance (`v(s) = v0 − c·s`), so the flight integration
    /// ([`crate::math::integrate_shell_step`]), the HUD's penetration readout, and the server's
    /// impact math all agree in closed form. Full-bore AP holds its speed, light APCR cores
    /// bleed it fast, and the slow chemical rounds fly like bricks but never cared about speed.
    pub fn drag_per_s(self) -> f32 {
        match self.shell_type {
            ShellType::ArmorPiercing => 0.09,
            ShellType::Apcr => 0.21,
            ShellType::Heat | ShellType::HighExplosive => 0.05,
        }
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
    pub shell: ShellSpec,
    /// The gun's AUTHORED special round for rack slot 1, when the historical gun fielded one —
    /// e.g. the D-10's BK-5 HEAT. `None` derives the generic APCR from the stock shell (the
    /// pre-authoring formula), so guns without a real special round lose nothing.
    #[serde(default)]
    pub special_shell: Option<ShellSpec>,
}

const fn default_barrel_length_m() -> f32 {
    5.0
}

impl GunSpec {
    /// Shells the player can load for this gun — sidegrades, not strict upgrades. The stock AP is
    /// the balanced default; the special slot is the gun's AUTHORED round when it fielded one
    /// (HEAT holds its penetration at every range but a spaced screen kills the jet and steep
    /// plates shrug it off), otherwise a generic APCR derived from the stock shell (penetration
    /// and speed for alpha); HE trades penetration for damage and splash. No economy — every
    /// round is freely selectable, and the chosen shell is what the tank fires.
    pub fn ammo_options(&self) -> Vec<ShellSpec> {
        let stock = self.shell;
        let caliber = stock.caliber_mm;
        let special = self.special_shell.unwrap_or_else(|| {
            ShellSpec::apcr(
                caliber,
                stock.muzzle_velocity_mps * 1.20,
                stock.penetration_mm_at_100m * 1.25,
                ((stock.damage_hp as f32) * 0.85) as u32,
            )
        });
        let he = ShellSpec::high_explosive(
            caliber,
            stock.muzzle_velocity_mps * 0.70,
            stock.penetration_mm_at_100m * 0.35,
            ((stock.damage_hp as f32) * 1.4) as u32,
            1.5,
        );
        vec![stock, special, he]
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
    use crate::{ShellType, VehicleKind};

    #[test]
    fn the_d10_family_loads_its_authored_heat_and_it_ignores_range() {
        // The BK-5 activates ShellType::Heat end-to-end: the T-54's special slot is the authored
        // HEAT round, not the derived APCR — and chemical penetration is flat with distance,
        // where the kinetic rounds bleed theirs (the armour model already prices the trade:
        // spaced screens kill the jet, extreme obliquity sheds it).
        for kind in [VehicleKind::T54_1951, VehicleKind::T55A] {
            let options = kind.spec().gun.ammo_options();
            let special = options[1];
            assert_eq!(special.shell_type, ShellType::Heat, "{kind:?} slot 1 is the BK-5");
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
        }
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
        // Sidegrade, not strict upgrade: APCR gives up alpha for that penetration; HE is the
        // opposite trade (more damage, far less penetration).
        assert!(options[1].damage_hp < options[0].damage_hp, "APCR trades away alpha");
        assert!(options[2].damage_hp > options[0].damage_hp, "HE trades penetration for damage");
        assert!(options[2].penetration_mm_at_100m < options[0].penetration_mm_at_100m);
        let kinds: std::collections::HashSet<_> = options.iter().map(|s| s.shell_type).collect();
        assert!(kinds.len() >= 3, "the rounds are of distinct shell types");
    }
}
