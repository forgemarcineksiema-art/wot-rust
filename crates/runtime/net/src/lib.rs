use bincode::Options;
use game_core::{
    DamageEvent, MODULE_SLOT_COUNT, ShellImpact, TankId, TeamId, VehicleKind, WeatherVariant,
};
use serde::{Deserialize, Serialize};
use sim::{SimulationState, TankCommand, TankState};
use terrain::MapId;
use thiserror::Error;

mod frame;
mod snapshot_filter;
mod snapshot_schedule;

pub use frame::{FRAME_HEADER_LEN, FRAME_MAGIC, decode_frame, encode_frame};
pub use snapshot_schedule::SnapshotSchedule;

/// v18: `ServerHello` names the match — `map_id` and `weather_variant` — so the client can
/// deterministically rebuild the same battlefield the server simulates (the map itself is
/// never sent) and dress it in the same sky. `ImpactSurface` gains `Water`.
pub const PROTOCOL_VERSION: u16 = 18;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("protocol codec failed: {0}")]
    Codec(#[from] Box<bincode::ErrorKind>),
    #[error("protocol frame is too short: {len} bytes")]
    FrameTooShort { len: usize },
    #[error("protocol frame magic does not match")]
    InvalidFrameMagic,
    #[error("protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolVersionMismatch { expected: u16, actual: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationConfig {
    pub snapshot_hz: u32,
    pub interpolation_delay_ticks: u32,
    pub max_prediction_ticks: u32,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self { snapshot_hz: 20, interpolation_delay_ticks: 2, max_prediction_ticks: 8 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientInputCommand {
    pub client_tick: u64,
    pub tank_id: TankId,
    pub command: TankCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClientVehicleSelection {
    pub client_tick: u64,
    pub requested_vehicle: VehicleKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TankSnapshot {
    pub tank_id: TankId,
    /// Team identity (protocol v12): the client splits live enemies (targets) from teammates
    /// and wrecks (shell blockers) with the same rule as the authoritative server.
    pub team: TeamId,
    /// Stable vehicle identity so the client can pick the right procedural silhouette.
    pub vehicle: VehicleKind,
    pub position: [f32; 3],
    pub yaw_rad: f32,
    /// Authoritative hull pitch (+nose up) from the running-gear support plane (protocol v14).
    /// Gun arcs, armor angles and the hitbox frame all tilt with it — see
    /// `game_core::math::hull_basis`.
    pub hull_pitch_rad: f32,
    /// Authoritative hull roll (+right side up); protocol v14 alongside `hull_pitch_rad`.
    pub hull_roll_rad: f32,
    pub turret_yaw_rad: f32,
    pub turret_yaw_velocity_rad_s: f32,
    pub gun_pitch_rad: f32,
    pub hit_points: u32,
    pub reload_remaining_s: f32,
    pub aim_dispersion_mrad: f32,
    /// Live module HP in `ModuleSlot::ALL` order.
    pub module_hit_points: [u32; MODULE_SLOT_COUNT],
    /// Bitset of destroyed modules in `ModuleSlot::ALL` order.
    pub destroyed_modules_mask: u8,
    /// Side-specific track damage bitset; see `TrackDamageMask`.
    pub track_damage_mask: u8,
    /// Rounds remaining per ammo slot, `GunSpec::ammo_options()` order (protocol v15).
    pub ammo_counts: [u16; game_core::MAX_AMMO_SLOTS],
    /// The ammo slot the next shot fires from (protocol v15).
    pub selected_ammo: u8,
    /// Bitmask of teams that can currently see this tank (bit `t` = `TeamId(t+1)`), from the LOS
    /// spotting pass (protocol v16). Local authoritative snapshots are filtered per viewer before
    /// they reach the client.
    pub spotted_by_teams_mask: u8,
}

impl TankSnapshot {
    /// The hull's replicated orientation as the one shared [`game_core::math::HullPose`] frame.
    pub fn hull_pose(&self) -> game_core::math::HullPose {
        game_core::math::HullPose {
            yaw_rad: self.yaw_rad,
            pitch_rad: self.hull_pitch_rad,
            roll_rad: self.hull_roll_rad,
        }
    }
}

impl From<&TankState> for TankSnapshot {
    fn from(tank: &TankState) -> Self {
        Self {
            tank_id: tank.id,
            team: tank.team,
            vehicle: tank.spec.kind,
            position: tank.position.to_array(),
            yaw_rad: tank.yaw_rad,
            hull_pitch_rad: tank.hull_pitch_rad,
            hull_roll_rad: tank.hull_roll_rad,
            turret_yaw_rad: tank.turret_yaw_rad,
            turret_yaw_velocity_rad_s: tank.turret_yaw_velocity_rad_s,
            gun_pitch_rad: tank.gun_pitch_rad,
            hit_points: tank.hit_points,
            reload_remaining_s: tank.reload_remaining_s,
            aim_dispersion_mrad: tank.aim_dispersion_mrad,
            module_hit_points: tank.modules.hit_points_by_slot(),
            destroyed_modules_mask: tank.modules.destroyed_mask(),
            track_damage_mask: tank.tracks.bits(),
            ammo_counts: tank.ammo_counts,
            selected_ammo: tank.selected_ammo,
            spotted_by_teams_mask: tank.spotted_mask,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShellSnapshot {
    pub owner: TankId,
    pub position: [f32; 3],
    pub velocity_mps: [f32; 3],
}

impl From<&sim::ShellState> for ShellSnapshot {
    fn from(shell: &sim::ShellState) -> Self {
        Self {
            owner: shell.owner,
            position: shell.position.to_array(),
            velocity_mps: shell.velocity_mps.to_array(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub server_tick: u64,
    pub tanks: Vec<TankSnapshot>,
    pub shells: Vec<ShellSnapshot>,
    pub damage_events: Vec<DamageEvent>,
    /// Shells absorbed without enemy damage (protocol v12): terrain, cover, wreck, or friendly
    /// hull. Lets the firing client mark where its shot died instead of it vanishing silently.
    pub shell_impacts: Vec<ShellImpact>,
}

impl From<&SimulationState> for Snapshot {
    fn from(state: &SimulationState) -> Self {
        Self {
            server_tick: state.tick(),
            tanks: state.tanks().iter().map(TankSnapshot::from).collect(),
            shells: state.shells().iter().map(ShellSnapshot::from).collect(),
            damage_events: state.damage_events().to_vec(),
            shell_impacts: state.shell_impacts().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtocolMessage {
    Input(ClientInputCommand),
    VehicleSelection(ClientVehicleSelection),
    Snapshot(Snapshot),
    Ping {
        client_time_us: u64,
    },
    Pong {
        client_time_us: u64,
        server_time_us: u64,
    },
    ClientHello {
        protocol_version: u16,
    },
    /// The server's opening word (protocol v18): which map to generate locally and which
    /// weather to dress it in. Both sides run the same deterministic map generator, so these
    /// two ids are all it takes to agree on a world.
    ServerHello {
        protocol_version: u16,
        map_id: MapId,
        weather_variant: WeatherVariant,
    },
}

/// Wire codec for all protocol messages. Byte-compatible with bincode's standalone
/// `serialize`/`deserialize` (little-endian, fixed-int) but additionally **rejects
/// trailing bytes**, so a prefix of a longer/newer message can no longer be silently
/// mis-decoded as a valid shorter one.
fn wire_codec() -> impl Options {
    bincode::DefaultOptions::new().with_fixint_encoding().reject_trailing_bytes()
}

pub fn encode_message(message: &ProtocolMessage) -> Result<Vec<u8>, NetError> {
    Ok(wire_codec().serialize(message)?)
}

pub fn decode_message(bytes: &[u8]) -> Result<ProtocolMessage, NetError> {
    Ok(wire_codec().deserialize(bytes)?)
}
