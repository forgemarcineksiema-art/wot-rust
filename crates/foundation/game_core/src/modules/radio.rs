use serde::{Deserialize, Serialize};

/// Radio. Drives spotting/signal range; the lightest module and the most fragile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadioModule {
    pub name: String,
    pub mass_kg: f32,
    pub hit_points: u32,
    pub signal_range_m: f32,
}
