use game_core::TankSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankControllerSettings {
    pub max_forward_speed_mps: f32,
    pub max_reverse_speed_mps: f32,
    /// Specific effective drive power (m²/s³): engine power × drivetrain efficiency × the arcade
    /// scale, over mass. Thrust follows `P/v` — huge at a crawl (where track grip caps it), thin
    /// near top speed — so acceleration tapers the way tonnes behind an engine actually do.
    pub drive_power_mps3: f32,
    /// Speed floor for the `P/v` thrust curve, so a standing start has a finite (grip-capped)
    /// launch force instead of a division blow-up.
    pub min_force_speed_mps: f32,
    /// Rolling resistance (m/s²): the constant drag of tracks over ground.
    pub rolling_resist_mps2: f32,
    /// Quadratic drag coefficient, solved per vehicle so thrust and resistance balance exactly at
    /// the spec's top speed — vmax is an emergent equilibrium, not a clamp.
    pub drag_quadratic: f32,
    pub brake_deceleration_mps2: f32,
    pub turn_rate_rad_s: f32,
    pub ground_probe_length_m: f32,
    /// Coast deceleration with the throttle released (engine braking + rolling), before the
    /// quadratic term. A tank rolls out over many hull lengths, not a few metres.
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
    /// How strongly a turn bleeds forward speed (skid-steer scrub): decel = k * |yaw_rate| * |v|.
    pub turn_scrub: f32,
}

/// Maximum uphill grade (rise/run) a tank can climb. Steeper faces -- like the railway
/// embankment -- stall it. ~0.6 is roughly 31 degrees, the classic ~60% tank gradeability.
const DEFAULT_MAX_CLIMB_GRADE: f32 = 0.6;

/// Lateral grip coefficient. 0.95 keeps the hull on its line through normal turns and only lets
/// it break loose near the top of the speed/turn-rate envelope or on low-traction ground (the
/// terrain `traction` scales this down). Raise toward "on rails", lower toward "ice".
const DEFAULT_LATERAL_GRIP_MU: f32 = 0.95;

/// Drivetrain efficiency times the arcade condensation factor. Real tanks take 25+ seconds to
/// top out; the game wants the same *shape* (hard launch, grinding top end) compressed to roughly
/// half the wall-clock, so effective power is scaled up rather than the curve flattened.
const DRIVE_POWER_FACTOR: f32 = 0.75 * 2.2;

/// Rolling resistance of tracks on firm ground, m/s².
const ROLLING_RESIST_MPS2: f32 = 0.45;

impl TankControllerSettings {
    pub fn arcade_default() -> Self {
        Self::from_spec(&TankSpec::medium_test_tank())
    }

    pub fn from_spec(spec: &TankSpec) -> Self {
        let drive_power_mps3 =
            spec.engine_power_kw * 1000.0 * DRIVE_POWER_FACTOR / spec.mass_kg.max(1.0);
        let vmax = spec.max_forward_speed_mps.max(1.0);
        // Solve the quadratic drag so thrust == resistance exactly at the spec top speed.
        let drag_quadratic =
            ((drive_power_mps3 / vmax - ROLLING_RESIST_MPS2) / (vmax * vmax)).max(1.0e-4);

        // Heavier hulls take longer to reach their commanded yaw rate, so the rotation reads as
        // weight rather than an instant snap. spool ~0.25 s (light) to ~0.7 s (heavy).
        let yaw_spool_s = (spec.mass_kg / 120_000.0).clamp(0.25, 0.7);
        let yaw_accel_rad_s2 = (spec.turn_rate_rad_s / yaw_spool_s).max(0.1);

        Self {
            max_forward_speed_mps: spec.max_forward_speed_mps,
            max_reverse_speed_mps: spec.max_reverse_speed_mps,
            drive_power_mps3,
            min_force_speed_mps: 2.2,
            rolling_resist_mps2: ROLLING_RESIST_MPS2,
            drag_quadratic,
            // Tracks brake hard, but not "granite wall" hard: ~0.5 g reads as tonnes digging in.
            brake_deceleration_mps2: 4.8,
            turn_rate_rad_s: spec.turn_rate_rad_s,
            ground_probe_length_m: 3.0,
            idle_drag_mps2: 1.3,
            max_climb_grade: DEFAULT_MAX_CLIMB_GRADE,
            yaw_accel_rad_s2,
            longitudinal_grip_mu: DEFAULT_MAX_CLIMB_GRADE,
            lateral_grip_mu: DEFAULT_LATERAL_GRIP_MU,
            turn_scrub: 0.25,
        }
    }
}
