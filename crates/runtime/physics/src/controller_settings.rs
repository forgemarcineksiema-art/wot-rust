use game_core::TankSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankControllerSettings {
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
    pub acceleration_mps2: f32,
    pub brake_deceleration_mps2: f32,
    pub turn_rate_rad_s: f32,
    pub ground_probe_length_m: f32,
    pub idle_drag_mps2: f32,
    /// Maximum uphill grade (rise/run) the tank can climb; steeper faces stall it.
    pub max_climb_grade: f32,
}

/// Maximum uphill grade (rise/run) a tank can climb. Steeper faces -- like the railway
/// embankment -- stall it. ~0.6 is roughly 31 degrees, the classic ~60% tank gradeability.
const DEFAULT_MAX_CLIMB_GRADE: f32 = 0.6;

impl TankControllerSettings {
    pub fn arcade_default() -> Self {
        Self::from_spec(&TankSpec::medium_test_tank())
    }

    pub fn from_spec(spec: &TankSpec) -> Self {
        let power_to_weight_mps2 = spec.engine_power_kw * 1000.0 / spec.mass_kg.max(1.0);
        let acceleration_mps2 = (power_to_weight_mps2 * 0.7).clamp(2.5, 12.0);

        Self {
            max_forward_speed_mps: spec.max_forward_speed_mps,
            max_reverse_speed_mps: spec.max_reverse_speed_mps,
            acceleration_mps2,
            brake_deceleration_mps2: (acceleration_mps2 * 2.1).clamp(8.0, 24.0),
            turn_rate_rad_s: spec.turn_rate_rad_s,
            ground_probe_length_m: 3.0,
            idle_drag_mps2: (acceleration_mps2 * 0.35).clamp(1.0, 4.0),
            max_climb_grade: DEFAULT_MAX_CLIMB_GRADE,
        }
    }
}
