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
            throttle: self.throttle.clamp(-1.0, 1.0),
            steer: self.steer.clamp(-1.0, 1.0),
            brake: self.brake.clamp(0.0, 1.0),
            turret_yaw_delta: self.turret_yaw_delta.clamp(-1.0, 1.0),
            gun_pitch_delta: self.gun_pitch_delta.clamp(-1.0, 1.0),
            fire: self.fire,
            select_ammo: self.select_ammo,
        }
    }
}
