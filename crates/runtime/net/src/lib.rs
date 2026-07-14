use bincode::Options;
use game_core::{
    ArmorBreachSet, DamageEvent, MODULE_SLOT_COUNT, ShellId, ShellImpact, ShellType, TRACK_HP_MAX,
    TankId, TeamId, VehicleKind, WeatherVariant,
};
use serde::{Deserialize, Serialize};
use sim::{SimulationState, TankCommand, TankState};
use terrain::MapId;
use thiserror::Error;

mod frame;
pub mod session;
mod snapshot_filter;
mod snapshot_schedule;
pub mod transport;

pub use frame::{FRAME_HEADER_LEN, FRAME_MAGIC, decode_frame, encode_frame};
pub use snapshot_schedule::SnapshotSchedule;

/// v26: persistent breaches carry ammo-specific deterministic aperture contours, age, energy and
/// ingress/egress identity. The same payload drives server clearance, first-frame clipping,
/// late-join presentation and replay; damage-mesh revisions remain client-local.
///
/// v25: `TankSnapshot` carries persistent armor breaches, engine fire, and per-side track break
/// positions. `DamageEvent` carries all modules touched/destroyed by one internal through-flight.
///
/// v24: in-flight shells carry stable identity plus type, caliber, drag, and age so presentation
/// preserves projectile identity and extrapolates with the authoritative flight physics.
///
/// v23: `WeatherVariant` gains `GoldenEvening` and `Overcast` (appended — the variant order is
/// wire identity), the Prokhorovka time-of-day looks from the lighting 2.0 program. No message
/// layout changes; only the value domain of an existing field grows.
///
/// v22: `TankSnapshot` carries `track_hp` — the graded per-side track pool `[left, right]` — so
/// the client predictor drives the same damaged-track mobility the server does (the broken-only
/// `track_damage_mask` cannot express the Damaged tier). `DamageEvent` carries `track_hit` — which
/// side a shell struck and whether it threw the track — so the client can call it out, log it and
/// sound it. Both appended, `serde(default)`, a pure wire-layout extension.
///
/// v21: `Snapshot` carries `cover_states` — one phase byte per static-cover object (intact /
/// rubble / gone), so the client can draw collapsed buildings and cleared foliage. Global world
/// state, re-sent every snapshot (a late joiner converges), index-aligned with the map's cover.
///
/// v20: `Snapshot` carries `detached_turrets` — the wrecks whose turret an ammo-rack detonation
/// blew off. Re-sent every snapshot (so a late joiner converges), it drives the client's ballistic
/// turret pop-off and matches the server's wreck trace, which now skips the detached turret.
///
/// v19: `DamageEvent` carries the struck plate's world normal and the shell's heading
/// (`plate_normal`, `shell_direction`), so the client can seat impact marks flush on the visual
/// armor instead of guessing a cardinal facing normal. Both are appended, `serde(default)`, so
/// the change is a pure wire-layout extension.
///
/// v18: `ServerHello` names the match — `map_id` and `weather_variant` — so the client can
/// deterministically rebuild the same battlefield the server simulates (the map itself is
/// never sent) and dress it in the same sky. `ImpactSurface` gains `Water`.
pub const PROTOCOL_VERSION: u16 = 28;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("transport: {0}")]
    Transport(String),
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

/// Serde default for `TankSnapshot::track_hp`: a full pool both sides (pre-v22 fixtures had no
/// track HP, so they load as healthy rather than as a phantom double-broken hull).
fn full_track_hp() -> [u8; 2] {
    [TRACK_HP_MAX; 2]
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
    /// Side-specific *broken* track bitset (derived from the HP pool); see `TrackDamageMask`.
    /// A side appears here exactly when its pool is at zero.
    pub track_damage_mask: u8,
    /// Graded per-side track HP `[left, right]` (protocol v22) — the Damaged-tier state the broken
    /// mask cannot carry, so the predictor drives the same degraded mobility as the server.
    /// `serde(default)` (full pool) keeps pre-v22 fixtures loading healthy.
    #[serde(default = "full_track_hp")]
    pub track_hp: [u8; 2],
    /// Rounds remaining per ammo slot, `GunSpec::ammo_options()` order (protocol v15).
    pub ammo_counts: [u16; game_core::MAX_AMMO_SLOTS],
    /// The ammo slot the next shot fires from (protocol v15).
    pub selected_ammo: u8,
    /// Bitmask of teams that can currently see this tank (bit `t` = `TeamId(t+1)`), from the LOS
    /// spotting pass (protocol v16). Local authoritative snapshots are filtered per viewer before
    /// they reach the client.
    pub spotted_by_teams_mask: u8,
    /// Persistent per-instance perforations (protocol v25); late join and replay converge.
    #[serde(default)]
    pub armor_breaches: ArmorBreachSet,
    /// Normalized thrown-belt gap position `[left, right]` (protocol v25).
    #[serde(default)]
    pub track_break_t: [Option<f32>; 2],
    /// Engine compartment is visibly burning (protocol v25).
    #[serde(default)]
    pub engine_fire: bool,
    /// v27: a holed fuel tank burns as itself, independent of the engine's fire.
    #[serde(default)]
    pub fuel_fire: bool,
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
            track_damage_mask: tank.tracks.broken_mask().bits(),
            track_hp: tank.tracks.hp_pair(),
            ammo_counts: tank.ammo_counts,
            selected_ammo: tank.selected_ammo,
            spotted_by_teams_mask: tank.spotted_mask,
            armor_breaches: tank.armor_breaches.clone(),
            track_break_t: tank.track_break_t,
            engine_fire: tank.engine_fire,
            fuel_fire: tank.fuel_fire,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct ShellSnapshot {
    pub shell_id: ShellId,
    pub owner: TankId,
    pub position: [f32; 3],
    pub velocity_mps: [f32; 3],
    pub shell_type: ShellType,
    pub caliber_mm: f32,
    pub drag_per_s: f32,
    pub age_seconds: f32,
}

impl From<&sim::ShellState> for ShellSnapshot {
    fn from(shell: &sim::ShellState) -> Self {
        Self {
            shell_id: shell.id,
            owner: shell.owner,
            position: shell.position.to_array(),
            velocity_mps: shell.velocity_mps.to_array(),
            shell_type: shell.shell.shell_type,
            caliber_mm: shell.shell.caliber_mm,
            drag_per_s: shell.shell.drag_per_s(),
            age_seconds: shell.age_seconds,
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
    /// Wrecks whose turret an ammo-rack detonation blew off (protocol v20). Derived fresh from the
    /// full sim state every snapshot, so a late joiner converges on the same decapitated wrecks;
    /// the client starts (or, on join, resolves) the ballistic turret pop-off from this list.
    /// `serde(default)` keeps pre-v20 fixtures loading with every turret attached.
    #[serde(default)]
    pub detached_turrets: Vec<TankId>,
    /// One phase byte per static-cover object (protocol v21): 0 intact, 1 rubble, 2 gone.
    /// Index-aligned with the map's cover (deterministic per map), so the client dresses each
    /// object by its phase. `serde(default)` keeps pre-v21 fixtures loading with whole cover.
    #[serde(default)]
    pub cover_states: Vec<u8>,
}

impl From<&SimulationState> for Snapshot {
    fn from(state: &SimulationState) -> Self {
        Self {
            server_tick: state.tick(),
            tanks: state.tanks().iter().map(TankSnapshot::from).collect(),
            shells: state.shells().iter().map(ShellSnapshot::from).collect(),
            damage_events: state.damage_events().to_vec(),
            shell_impacts: state.shell_impacts().to_vec(),
            detached_turrets: state
                .tanks()
                .iter()
                .filter(|tank| tank.turret_detached)
                .map(|tank| tank.id)
                .collect(),
            cover_states: state.cover_states().iter().map(|state| state.phase.to_wire()).collect(),
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
    /// v28: the client's per-tick send over a LOSSY wire — the latest few commands, newest
    /// last. A dropped datagram costs nothing: the next batch re-carries the recent history
    /// and the server consumes only commands for ticks it has not yet simulated.
    InputBatch {
        commands: Vec<ClientInputCommand>,
    },
    /// v28: an orderly goodbye, so a leaving client frees its lobby slot immediately instead
    /// of aging out through the heartbeat timeout.
    Disconnect {
        reason: DisconnectReason,
    },
}

/// Why a peer said goodbye (v28). Wire-stable: append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisconnectReason {
    /// The player quit deliberately.
    Quit,
    /// The client is out of protocol or otherwise refused by the server.
    Refused,
    /// The battle ended and the session is over.
    BattleOver,
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
