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
    /// Maximum uphill grade (rise/run) the tank can climb; steeper faces stall it. In the
    /// rigid-body model this is no longer a hard `if`: it is the longitudinal grip coefficient
    /// (`longitudinal_grip_mu`), so a face steeper than `tan(theta) = grade` cannot be held by
    /// track traction and the hull stalls (or creeps back) on its own.
    pub max_climb_grade: f32,
    /// Angular acceleration toward the commanded yaw rate (rad/s^2). This is the hull's rotational
    /// inertia knob: a finite ramp is why the hull no longer snaps to a new heading the instant
    /// the steer key is tapped. Heavier hulls spool slower.
    pub yaw_accel_rad_s2: f32,
    /// Longitudinal track grip coefficient (dimensionless mu). With gravity `g`, the tracks can
    /// deliver at most `mu * g * traction * cos(theta)` of forward thrust, which is what makes
    /// gradeability emergent rather than a clamp. Kept equal to `max_climb_grade`.
    pub longitudinal_grip_mu: f32,
    /// Lateral track grip coefficient (dimensionless mu). Sideways friction saturates at
    /// `mu * g * traction`; below it the hull tracks its nose, above it (a hard turn at speed, or
    /// a steep/low-traction face) it breaks loose and slides. High by default so the hull "grips"
    /// like WoT and only drifts in genuinely hard turns or on poor ground.
    pub lateral_grip_mu: f32,
}

/// Maximum uphill grade (rise/run) a tank can climb. Steeper faces -- like the railway
/// embankment -- stall it. ~0.6 is roughly 31 degrees, the classic ~60% tank gradeability.
const DEFAULT_MAX_CLIMB_GRADE: f32 = 0.6;

/// Lateral grip coefficient. 0.95 keeps the hull on its line through normal turns and only lets
/// it break loose near the top of the speed/turn-rate envelope or on low-traction ground (the
/// terrain `traction` scales this down). Raise toward "on rails", lower toward "ice".
const DEFAULT_LATERAL_GRIP_MU: f32 = 0.95;

impl TankControllerSettings {
    pub fn arcade_default() -> Self {
        Self::from_spec(&TankSpec::medium_test_tank())
    }

    pub fn from_spec(spec: &TankSpec) -> Self {
        let power_to_weight_mps2 = spec.engine_power_kw * 1000.0 / spec.mass_kg.max(1.0);
        let acceleration_mps2 = (power_to_weight_mps2 * 0.7).clamp(2.5, 12.0);

        // Heavier hulls take longer to reach their commanded yaw rate, so the rotation reads as
        // weight rather than an instant snap. spool ~0.25 s (light) to ~0.7 s (heavy).
        let yaw_spool_s = (spec.mass_kg / 120_000.0).clamp(0.25, 0.7);
        let yaw_accel_rad_s2 = (spec.turn_rate_rad_s / yaw_spool_s).max(0.1);

        Self {
            max_forward_speed_mps: spec.max_forward_speed_mps,
            max_reverse_speed_mps: spec.max_reverse_speed_mps,
            acceleration_mps2,
            brake_deceleration_mps2: (acceleration_mps2 * 2.1).clamp(8.0, 24.0),
            turn_rate_rad_s: spec.turn_rate_rad_s,
            ground_probe_length_m: 3.0,
            idle_drag_mps2: (acceleration_mps2 * 0.35).clamp(1.0, 4.0),
            max_climb_grade: DEFAULT_MAX_CLIMB_GRADE,
            yaw_accel_rad_s2,
            longitudinal_grip_mu: DEFAULT_MAX_CLIMB_GRADE,
            lateral_grip_mu: DEFAULT_LATERAL_GRIP_MU,
        }
    }
}
