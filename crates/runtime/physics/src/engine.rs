//! The engine through its gearbox (GDD §4, the one program's J6): the gear and the revs are a
//! function of the hull's speed and the box's data, the power on tap a function of the revs.
//! STATELESS by design — the gear is a function of the
//! hull's speed and the box's data, so the server, the predictor and the engine voice all read the
//! same gear without a byte on the wire, and a replay cannot drift on a clutch timer.
//!
//! The rating is the old P/v drive: at any speed the thrust is the rated power over the speed,
//! scaled by the share of the rating the revs in the current gear put on tap — so the launch, the
//! climbing envelope and the top-speed equilibrium keep their measured contracts, and the box
//! decides the REVS — what the engine voice sings and what a gear readout shows. (A thrust beat at
//! each shift was measured and rejected: stateless, it traps a hull cruising at a shift speed in
//! the dip forever — the turn radius moved 6.9 → 5.5 m; stateful, it costs a byte on the wire.)

use crate::controller_settings::TankControllerSettings;

/// Revs, as a fraction of the governed speed, below which the engine is idling.
pub const IDLE_RPM_NORM: f32 = 0.18;
/// The box shifts up when the revs in the current gear reach this fraction of the governor.
pub const SHIFT_UP_RPM_NORM: f32 = 0.96;
/// Power the engine delivers at idle revs, as a fraction of its rated power, and the rev band
/// where the full rating is on tap. Below `PEAK_FROM_RPM` a driver slips the clutch to keep the
/// engine there, which is why a standing start is not starved (see `engine_thrust_mps2`).
const POWER_AT_IDLE: f32 = 0.50;
const PEAK_FROM_RPM: f32 = 0.55;
const PEAK_TO_RPM: f32 = 0.85;
/// Power at the governor, as a fraction of the rating. Exactly 1.0: the resistance the settings
/// solve for is the rating over the top speed, so any roll-off here would move the top speed.
const POWER_AT_GOVERNOR: f32 = 1.0;
/// What the engine is doing at a given ground speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineState {
    /// 0-based gear the box is in.
    pub gear: usize,
    /// Revs as a fraction of the governed speed (idle floor included).
    pub rpm_norm: f32,
    /// Power on tap at those revs as a fraction of the rating, the shift dip included.
    pub power_norm: f32,
}

/// The gear and revs for `speed_abs` (m/s, unsigned) on this hull's box.
pub fn engine_state(settings: &TankControllerSettings, speed_abs: f32) -> EngineState {
    let vmax = settings.max_forward_speed_mps.max(0.1);
    let box_ = settings.gearbox;
    let gears = box_.gear_count();
    let mut gear = gears - 1;
    for index in 0..gears {
        if speed_abs < vmax * box_.top_speed_fraction(index) * SHIFT_UP_RPM_NORM {
            gear = index;
            break;
        }
    }
    let top = vmax * box_.top_speed_fraction(gear);
    let rpm_norm = (speed_abs / top).clamp(IDLE_RPM_NORM, 1.0);
    let power = power_curve(rpm_norm);
    EngineState { gear, rpm_norm, power_norm: power }
}

/// Power on tap as a fraction of the rating: rising from idle to the peak band, then rolling
/// off toward the governor.
fn power_curve(rpm_norm: f32) -> f32 {
    if rpm_norm <= PEAK_FROM_RPM {
        let t = ((rpm_norm - IDLE_RPM_NORM) / (PEAK_FROM_RPM - IDLE_RPM_NORM)).clamp(0.0, 1.0);
        POWER_AT_IDLE + (1.0 - POWER_AT_IDLE) * t
    } else if rpm_norm <= PEAK_TO_RPM {
        1.0
    } else {
        let t = ((rpm_norm - PEAK_TO_RPM) / (1.0 - PEAK_TO_RPM)).clamp(0.0, 1.0);
        1.0 + (POWER_AT_GOVERNOR - 1.0) * t
    }
}

/// Engine thrust (m/s², before the track grip cap) at `speed_abs` under `throttle_abs` (0..=1):
/// the rated drive power over the speed, scaled by the power on tap at the revs the gear puts
/// the engine at. Under `min_force_speed_mps` the clutch is slipping and the engine sits in its
/// peak band, so a standing start and a crawl up a face keep the full rating (the climbing
/// envelope is a contract, `climb_envelope.rs`); at the governor in top gear this is exactly
/// `drive_power / vmax`, the resistance the settings solve for.
pub fn engine_thrust_mps2(
    settings: &TankControllerSettings,
    speed_abs: f32,
    throttle_abs: f32,
) -> f32 {
    let floor = settings.min_force_speed_mps.max(0.1);
    let power_norm =
        if speed_abs < floor { 1.0 } else { engine_state(settings, speed_abs).power_norm };
    settings.drive_power_mps3 * throttle_abs * power_norm / speed_abs.max(floor)
}
