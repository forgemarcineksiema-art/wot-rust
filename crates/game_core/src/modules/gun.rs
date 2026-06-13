use serde::{Deserialize, Serialize};

use crate::GunSpec;

/// Main armament. Wraps the existing [`GunSpec`] (reload, dispersion, shell) and adds the
/// module's own mass and hit points so it can be swapped and damaged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GunModule {
    pub spec: GunSpec,
    pub mass_kg: f32,
    pub hit_points: u32,
    /// Visible length of the exposed barrel (m). Drives the garage silhouette when a gun is
    /// swapped; purely cosmetic — the firing muzzle still comes from the vehicle's mount frames.
    #[serde(default = "default_barrel_length_m")]
    pub barrel_length_m: f32,
}

fn default_barrel_length_m() -> f32 {
    5.0
}

impl GunModule {
    pub fn caliber_mm(&self) -> f32 {
        self.spec.shell.caliber_mm
    }
}
