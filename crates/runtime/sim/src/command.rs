use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TankCommand {
    pub throttle: f32,
    pub steer: f32,
    // `default` lets JSON replay fixtures omit `brake`; it does NOT give bincode wire
    // compatibility (bincode is positional/non-self-describing — see net::wire_codec).
    #[serde(default)]
    pub brake: f32,
    pub turret_yaw_delta: f32,
    /// Gun elevation rate input in [-1, 1] (+ = elevate). `default` keeps older JSON
    /// replay fixtures (which omit it) loading; it is not bincode wire-compatible.
    #[serde(default)]
    pub gun_pitch_delta: f32,
    pub fire: bool,
    /// Request to switch the loaded ammo slot (`GunSpec::ammo_options()` index). A real switch
    /// restarts the reload; out-of-range or same-slot requests are no-ops. `default` keeps older
    /// JSON replay fixtures loading; it is not bincode wire-compatible (protocol v15).
    #[serde(default)]
    pub select_ammo: Option<u8>,
}

impl TankCommand {
    pub fn idle() -> Self {
        Self {
            throttle: 0.0,
            steer: 0.0,
            brake: 0.0,
            turret_yaw_delta: 0.0,
            gun_pitch_delta: 0.0,
            fire: false,
            select_ammo: None,
        }
    }

    pub fn drive(throttle: f32, steer: f32) -> Self {
        Self { throttle, steer, ..Self::idle() }
    }

    pub fn drive_with_turret(throttle: f32, steer: f32, turret_yaw_delta: f32) -> Self {
        Self { throttle, steer, turret_yaw_delta, ..Self::idle() }
    }

    pub fn drive_with_brake(throttle: f32, steer: f32, brake: f32) -> Self {
        Self { throttle, steer, brake, ..Self::idle() }
    }

    pub fn clamped(self) -> Self {
        Self {
            throttle: nan_safe_clamp(self.throttle, -1.0, 1.0),
            steer: nan_safe_clamp(self.steer, -1.0, 1.0),
            brake: nan_safe_clamp(self.brake, 0.0, 1.0),
            turret_yaw_delta: nan_safe_clamp(self.turret_yaw_delta, -1.0, 1.0),
            gun_pitch_delta: nan_safe_clamp(self.gun_pitch_delta, -1.0, 1.0),
            fire: self.fire,
            select_ammo: self.select_ammo,
        }
    }
}

/// Clamp to `[lo, hi]`, treating NaN as the neutral `0.0` rather than letting it through.
/// `f32::clamp` passes NaN straight out — and a single NaN in `throttle` becomes NaN velocity, NaN
/// position, and, through the contact impulses, a NaN in every neighbour it touches. `±inf` needs
/// no special case: `clamp` already pins it to the axis bound (full input is a legal command). The
/// remote lane rejects non-finite commands too (`battle_host::remote_input`), but the authority
/// sanitises here so every path into a tick is cleaned at one door, not only the wire.
fn nan_safe_clamp(value: f32, lo: f32, hi: f32) -> f32 {
    if value.is_nan() { 0.0 } else { value.clamp(lo, hi) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_neutralises_nan_and_pins_infinities_to_the_bound() {
        let poisoned = TankCommand {
            throttle: f32::NAN,
            steer: f32::INFINITY,
            brake: f32::NEG_INFINITY,
            turret_yaw_delta: f32::NAN,
            gun_pitch_delta: -5.0,
            fire: true,
            select_ammo: Some(1),
        };
        let clean = poisoned.clamped();
        assert_eq!(clean.throttle, 0.0, "NaN throttle must not reach the tick");
        assert_eq!(clean.steer, 1.0, "+inf steer clamps to the axis limit, not through it");
        assert_eq!(clean.brake, 0.0, "-inf brake collapses to no braking");
        assert_eq!(clean.turret_yaw_delta, 0.0);
        assert_eq!(
            clean.gun_pitch_delta, -1.0,
            "a finite out-of-range value still clamps normally"
        );
        assert!(clean.throttle.is_finite() && clean.steer.is_finite() && clean.brake.is_finite());
        // Non-numeric fields pass through untouched.
        assert!(clean.fire);
        assert_eq!(clean.select_ammo, Some(1));
    }
}
