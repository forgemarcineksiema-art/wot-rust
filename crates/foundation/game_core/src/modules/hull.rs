use serde::{Deserialize, Serialize};

/// The fixed chassis. Unlike the five module slots this is not separately damageable: it
/// carries the vehicle's main hit-point pool, the base hull armor, the top speeds, and its
/// own structural mass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HullChassis {
    pub name: String,
    pub mass_kg: f32,
    /// The vehicle's main hit-point pool (the bar that, at zero, knocks the tank out).
    pub hit_points: u32,
    pub front_mm: f32,
    pub side_mm: f32,
    pub rear_mm: f32,
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
}
