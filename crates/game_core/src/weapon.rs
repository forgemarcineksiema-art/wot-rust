use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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

    pub fn penetration_mm_at_distance(self, distance_m: f32) -> f32 {
        let beyond_100m = (distance_m - 100.0).max(0.0);
        let retention = match self.shell_type {
            ShellType::ArmorPiercing => (1.0 - beyond_100m * 0.00015).clamp(0.65, 1.0),
            ShellType::Apcr => (1.0 - beyond_100m * 0.00028).clamp(0.45, 1.0),
            ShellType::Heat | ShellType::HighExplosive => 1.0,
        };
        self.penetration_mm_at_100m * retention
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
    pub shell: ShellSpec,
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
