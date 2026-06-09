use serde::{Deserialize, Serialize};

/// Running gear (tracks + suspension). Drives hull `turn_rate_rad_s`; when destroyed the
/// vehicle is immobilised (a thrown track).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuspensionModule {
    pub name: String,
    pub mass_kg: f32,
    pub hit_points: u32,
    pub turn_rate_rad_s: f32,
    /// Maximum combat weight this running gear can carry (compatibility headroom).
    pub max_load_kg: f32,
}
