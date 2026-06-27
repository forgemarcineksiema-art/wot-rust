use serde::{Deserialize, Serialize};

use crate::GunSpec;

/// Main armament. Wraps the existing [`GunSpec`] (reload, dispersion, shell) and adds the
/// module's own mass and hit points so it can be swapped and damaged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GunModule {
    pub spec: GunSpec,
    pub mass_kg: f32,
    pub hit_points: u32,
}

impl GunModule {
    pub fn caliber_mm(&self) -> f32 {
        self.spec.shell.caliber_mm
    }

    /// Exposed barrel length (m) — drives the silhouette and the muzzle (shell spawn). Lives on the
    /// gun's [`GunSpec`] so it survives `assemble` into the battle [`crate::TankSpec`].
    pub fn barrel_length_m(&self) -> f32 {
        self.spec.barrel_length_m
    }
}
