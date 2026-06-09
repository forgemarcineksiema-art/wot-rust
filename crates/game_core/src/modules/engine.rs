use serde::{Deserialize, Serialize};

/// Powerplant. Drives `engine_power_kw`; when destroyed the vehicle loses drive (and may
/// catch fire). Mass adds to the assembled combat weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineModule {
    pub name: String,
    pub power_kw: f32,
    pub mass_kg: f32,
    pub hit_points: u32,
    /// Probability (0.0..1.0) that a destroying engine hit starts a fire.
    pub fire_chance: f32,
}
