use serde::{Deserialize, Serialize};

use crate::{ArmorProfile, GunSpec, ModuleHealth, VehicleKind};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HitboxProfile {
    pub half_width_m: f32,
    pub half_height_m: f32,
    pub half_length_m: f32,
    pub center_y_m: f32,
    /// Local-space Y threshold above which hits are resolved against turret/casemate armor.
    pub turret_min_y_m: f32,
}

impl HitboxProfile {
    pub fn new(
        half_width_m: f32,
        half_height_m: f32,
        half_length_m: f32,
        center_y_m: f32,
        turret_min_y_m: f32,
    ) -> Self {
        Self { half_width_m, half_height_m, half_length_m, center_y_m, turret_min_y_m }
    }

    /// Per-vehicle collision box. Heights are realistic full-vehicle heights (`2 * half_height`):
    /// the Soviet mediums are famously low (~2.4 m) while the German heavies are genuinely ~3 m.
    /// `center_y` is set so the floor sits ~5 cm below ground (`center_y = half_height - 0.05`),
    /// and `turret_min_y` (local Y, relative to `center_y`) is the hull/turret split — chosen to
    /// keep the *world-space* split height unchanged from the earlier taller boxes, so hit
    /// resolution (which armor a shot meets) is preserved. The visible mesh is sized to fill this
    /// box; see `client::vehicle_visual_specs` and its `body_fits_within_hitbox` test.
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        match kind {
            // height 2.40 m, hull/turret split at world y 1.80
            VehicleKind::PrototypeMedium => Self::new(1.70, 1.20, 3.20, 1.15, 0.65),
            // height 2.40 m, split at world y 1.80
            VehicleKind::T54_1951 => Self::new(1.75, 1.20, 3.15, 1.15, 0.65),
            // height 2.38 m, split at world y 1.80
            VehicleKind::T55A => Self::new(1.75, 1.19, 3.20, 1.14, 0.66),
            // height 2.92 m, split at world y 1.90
            VehicleKind::TigerI => Self::new(1.95, 1.46, 3.60, 1.41, 0.49),
            // height 3.08 m, split at world y 1.95
            VehicleKind::TigerII => Self::new(1.95, 1.54, 4.00, 1.49, 0.46),
            // height 2.94 m, split at world y 2.00
            VehicleKind::Jagdtiger => Self::new(2.00, 1.47, 4.10, 1.42, 0.58),
            // height 2.94 m, split at world y 1.85
            VehicleKind::PantherII => Self::new(1.85, 1.47, 3.70, 1.42, 0.43),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TankSpec {
    pub name: String,
    /// Stable identity used by networking and the renderer to tell vehicles apart.
    #[serde(default)]
    pub kind: VehicleKind,
    pub mass_kg: f32,
    pub engine_power_kw: f32,
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
    pub turn_rate_rad_s: f32,
    pub turret_rotation_rad_s: f32,
    pub hull: ArmorProfile,
    pub gun: GunSpec,
    pub hit_points: u32,
    pub module_health: ModuleHealth,
    pub hitbox: HitboxProfile,
}

impl TankSpec {
    pub fn medium_test_tank() -> Self {
        VehicleKind::PrototypeMedium.spec()
    }

    pub fn has_fixed_casemate(&self) -> bool {
        self.turret_rotation_rad_s == 0.0
    }

    pub fn effective_turret_yaw_rad(&self, turret_yaw_rad: f32) -> f32 {
        if self.has_fixed_casemate() { 0.0 } else { turret_yaw_rad }
    }
}
