use bincode::Options;
use game_core::{DamageEvent, MODULE_SLOT_COUNT, ShellImpact, TankId, TeamId, VehicleKind};
use serde::{Deserialize, Serialize};
use sim::{SimulationState, TankCommand, TankState};
use thiserror::Error;

mod snapshot_schedule;

pub use snapshot_schedule::SnapshotSchedule;

pub const PROTOCOL_VERSION: u16 = 12;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("protocol codec failed: {0}")]
    Codec(#[from] Box<bincode::ErrorKind>),
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
}

impl From<&TankState> for TankSnapshot {
    fn from(tank: &TankState) -> Self {
        Self {
            tank_id: tank.id,
            team: tank.team,
            vehicle: tank.spec.kind,
            position: tank.position.to_array(),
            yaw_rad: tank.yaw_rad,
            turret_yaw_rad: tank.turret_yaw_rad,
            turret_yaw_velocity_rad_s: tank.turret_yaw_velocity_rad_s,
            gun_pitch_rad: tank.gun_pitch_rad,
            hit_points: tank.hit_points,
            reload_remaining_s: tank.reload_remaining_s,
            aim_dispersion_mrad: tank.aim_dispersion_mrad,
            module_hit_points: tank.modules.hit_points_by_slot(),
            destroyed_modules_mask: tank.modules.destroyed_mask(),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    Ping { client_time_us: u64 },
    Pong { client_time_us: u64, server_time_us: u64 },
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
