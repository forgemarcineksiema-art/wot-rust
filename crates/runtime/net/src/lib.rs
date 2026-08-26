use bincode::Options;
use game_core::{
    ArmorBreach, ArmorBreachSet, DamageEvent, MODULE_SLOT_COUNT, MatchWeather, ShellId,
    ShellImpact, ShellType, TRACK_HP_MAX, TankId, TeamId, VehicleKind,
};
use serde::{Deserialize, Serialize};
use sim::{SimulationState, TankCommand, TankState};
use terrain::MapId;
use thiserror::Error;

mod frame;
pub mod recording;
pub mod session;
mod snapshot_filter;
mod snapshot_schedule;
pub mod transport;

pub use frame::{FRAME_HEADER_LEN, FRAME_MAGIC, decode_frame, encode_frame};
pub use snapshot_schedule::SnapshotSchedule;

/// v39: persistent armour perforations leave the world snapshot for the reliable lane.
///
/// `TankSnapshot::armor_breaches` was a whole `ArmorBreachSet` re-sent for every tank in every
/// snapshot, so a battle's wire cost grew monotonically with the shooting and never came back
/// down. Measured on a full 7v7 at the sim's own `MAX_ARMOR_BREACHES`: 31 695 B, which is 28 of
/// the transport's 28 fragments — and the reachable case (one shot owning both an ingress and an
/// egress fragment) is 87 471 B, 2.7x a message the transport can carry at all. Past the ceiling
/// the host's send fails and that crew simply stops receiving the world.
///
/// A perforation is append-only per tank and never changes once carved, which is precisely the
/// shape the v38 lane exists for. `CombatEvent::ArmorBreach` now carries each new perforation
/// exactly once, reliably and in order; the client replays them through the same
/// `ArmorBreachSet::add` the server ran, so both sides converge on the same set — including the
/// merge and capacity decisions, which depend on order. Unlike the personal damage/impact events
/// beside them, breaches are queued to EVERY crew: a tank that is invisible now may be visible
/// later, and a viewer who missed its perforations could never dress it correctly again.
///
/// v38: transient personal combat feedback has a small sequenced lane independent of fragmented
/// world snapshots. `CombatEventBatch` repeats until `CombatEventAck`; delivery sequence is
/// per-recipient so server-side spotting/audience filtering creates no gaps for the client.
///
/// v37: every remote-session and lifecycle message carries a `session_id`. A reconnect from the
/// same socket is therefore a new wire identity: delayed packets from the previous incarnation
/// cannot refresh liveness, advance state, apply input, or end the new battle. `InputAck` provides
/// an ACK-only path between snapshot deliveries while the delivery envelope remains
/// snapshot-aligned.
///
/// v36: remote snapshots travel in a per-client `SnapshotDelivery` envelope. The neutral
/// `Snapshot` remains reusable by local play/replays; the envelope carries the last input sequence
/// consumed for this crew plus authoritative hull motion for prediction reconciliation.
///
/// v34: `ServerHello` carries `MatchWeather` (program plus deterministic seed), so every client
/// and late joiner evaluates the same presentation timeline from authoritative battle tick time.
///
/// v32: `Snapshot` carries `cover_scars` — replicated shell wounds on static-cover faces
/// (kinetic insets and HE bites), purely visual, re-sent whole so a late joiner dresses the
/// same walls. Appended, `serde(default)`.
///
/// v31: `Snapshot` carries `craters` — the battle's quantized high-explosive crater ledger
/// (true terrain deformation). Global world state, re-sent every snapshot (a late joiner
/// converges); the battlefield owner folds it into the heightmap overlay, so physics, spotting,
/// the predictor and the bots all read the same deformed ground. Appended, `serde(default)`.
///
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
///
/// v33: the T-55A clone leaves the roster and its `VehicleKind` variant is deleted outright,
/// shifting every discriminant after it — a deliberate wire break (no live players yet;
/// the roster rule is "no clones").
///
/// v40: `ArmorZone::HullDeck` is appended (after `Skirt`, same rule) — the hull deck was
/// sharing the turret roof's zone, so a deck hit reported `TurretFront` on the wire and both
/// plates resolved against one derived thickness. The zone rides `DamageEvent.armor_zone` and
/// `ArmorBreach`, so the append is a wire change even though the layout is unchanged: an old
/// client would decode a deck hit as whatever variant now sits at that discriminant.
/// The armour VALUES that move with it (a cast turret's wall tapering aft, the T-54's
/// documented 200/160→65 and its 30 mm roof) are geometry, not wire — the deterministic bake
/// resolves them identically on both sides.
///
/// v44: a third-party projectile's OWNER is intel, not world state. `ShellSnapshot.owner` and
/// `ShellImpact.owner` become `Option<TankId>` — `None` when the viewer has not spotted the
/// firing tank — and a `ShotFired` from an unspotted shooter is dropped by the per-viewer
/// filter (it was never drawable and its shooter+shell_id pairing was the sharpest leak). The
/// tracer and the impact still replicate for everyone; only the identity is withheld. A wire
/// break because `owner`'s type changed. See `docs/spotting-policy.md`.
///
/// v45: `StartBattle` carries `time_limit_tick` — the sim tick the clock expires on, or `None`
/// for an untimed battle. Constant per battle, so it rides the one-shot seat word rather than
/// every snapshot; the client counts down locally against the `server_tick` it already tracks.
/// Before it, a remote HUD hid the battle timer because it knew the current tick but not the
/// deadline. Appended field — a wire break by layout.
/// v46: crew battle wounds (crew-damage foundation) — `TankSnapshot` gains the two crew masks
/// and the first-aid countdowns (team-private, see `snapshot_filter::conceal_enemy_crew_state`),
/// `DamageEvent` gains `crew_hits_mask` (the shooter's one-shot callout). All appends with
/// `serde(default)`; older fixtures load with a whole crew.
///
/// v47: concrete-round identity on the wire (Amunicja 3.0). `ShellSnapshot` and `DamageEvent`
/// gain `round: Option<RoundId>` — the id, never the spec: `shell_type`/`caliber_mm`/
/// `drag_per_s` stay explicit so the client never guesses flight from a catalog that could
/// skew, and the HUD names WHICH round hit ("BR-412D"), not which class. `DamageEvent` also
/// gains `shattered` — the brittle tungsten core's death on the plate (A4), which the sim has
/// routed on since then and FX can finally read. All appends with `serde(default)`; a legacy
/// fixture decodes with `round: None` and the client degrades to the type glyph.
///
/// v48: the test-only `VehicleKind::PrototypeMedium` (wire discriminant 0) is deleted outright,
/// shifting every remaining vehicle down by one — a deliberate wire break (no live players yet;
/// the roster rule is "no clones", and ALL must name every variant). Same class as v33.
pub const PROTOCOL_VERSION: u16 = 49;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("transport: {0}")]
    Transport(String),
    #[error("protocol codec failed: {0}")]
    Codec(#[from] Box<bincode::ErrorKind>),
    #[error("protocol frame is too short: {len} bytes")]
    FrameTooShort { len: usize },
    #[error("recording frame is too long: {len} bytes (max {max})")]
    RecordingFrameTooLong { len: usize, max: usize },
    #[error("protocol frame magic does not match")]
    InvalidFrameMagic,
    #[error("protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolVersionMismatch { expected: u16, actual: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationConfig {
    pub snapshot_hz: u32,
    pub max_prediction_ticks: u32,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self { snapshot_hz: 20, max_prediction_ticks: 8 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientInputCommand {
    pub client_tick: u64,
    pub tank_id: TankId,
    pub command: TankCommand,
}

/// The crew's garage pick, sent during the lobby (v49: carries the session like every other
/// client message — as untagged legacy traffic the server rightly refused it, and the seat
/// always spawned the benchmark). Idempotent and unreliable-friendly: the client repeats it
/// until seated, the server keeps newest-wins, and a pick after the battle starts is a no-op
/// (the roster is spawned; a late joiner takes over whatever hull is free).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClientVehicleSelection {
    pub session_id: u64,
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
    /// Persistent per-instance perforations — **client-local presentation state, never on the
    /// wire** (protocol v39).
    ///
    /// `serde(skip)`, not `serde(default)`: this used to be replicated in full for every tank in
    /// every snapshot, which grew a battle's payload monotonically with the shooting until the
    /// snapshot no longer fit one transport message. Perforations now arrive as a stream of
    /// additions on the reliable lane (`CombatEvent::ArmorBreach`), and a client fills this field
    /// from the set it accumulates (`engine::ArmorBreachStore`) before handing the snapshot to
    /// the render path. Writing to it does not replicate anything.
    #[serde(skip)]
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
    /// v43: seconds left on a lit ammunition rack's fuze, `None` when the rack is not cooking.
    ///
    /// The countdown IS the mechanic — ten seconds the crew can win — and until this field a
    /// cook-off was sim-only: the player whose rack was burning learned about it from the
    /// damage ticks. PRIVATE state: the owner and their team read the fuze (the radio net
    /// would carry nothing else first); an enemy sees a tank's interior only when it resolves
    /// (`snapshot_filter::conceal_enemy_rack_fuze`).
    #[serde(default)]
    pub rack_fire_remaining_s: Option<f32>,
    /// v46: crewmen currently DOWN (bit `i` in `CrewRole::ALL` order). PRIVATE state like the
    /// rack fuze: the owner and their team read it; enemies see a whole crew (the shooter's
    /// knowledge is the one-shot `DamageEvent::crew_hits_mask` callout, not an ongoing readout).
    #[serde(default)]
    pub crew_unconscious_mask: u8,
    /// v46: crewmen back from first aid but scarred for the battle. Same privacy as above.
    #[serde(default)]
    pub crew_weakened_mask: u8,
    /// v46: seconds of first aid left per role (`CrewRole::ALL` order), `None` when not down —
    /// the HUD countdown, and the predictor's seed for ticking the bandage between snapshots.
    #[serde(default)]
    pub crew_down_remaining_s: [Option<f32>; game_core::CREW_ROLE_COUNT],
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
            // Deliberately NOT `tank.armor_breaches`: the field is client-local (see its doc).
            // Filling it here would put the authoritative sets into a struct whose whole point is
            // that they no longer travel in it, and the client overwrites it from its own store.
            armor_breaches: ArmorBreachSet::default(),
            track_break_t: tank.track_break_t,
            engine_fire: tank.engine_fire,
            fuel_fire: tank.fuel_fire,
            rack_fire_remaining_s: tank
                .rack_fire
                .then(|| (sim::RACK_COOKOFF_S - tank.rack_fire_s).max(0.0)),
            crew_unconscious_mask: tank.crew.unconscious_mask(),
            crew_weakened_mask: tank.crew.weakened_mask(),
            crew_down_remaining_s: tank.crew.down_remaining_s(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct ShellSnapshot {
    pub shell_id: ShellId,
    /// Who fired the shell — but ONLY when the viewer may know it (protocol v44). A tracer is a
    /// world event everyone standing there sees, so the shell always replicates; its OWNER is
    /// intel, so it is `None` for a shell whose firing tank the viewer has not spotted. The
    /// client keys presentation on `shell_id`, never on this. (Residual: `shell_id` is a hash of
    /// the owner id — a determined packet reader could brute-force it over the ~14 tank ids; a
    /// per-viewer shell-id remap is future work, tracked in the multiplayer program.)
    pub owner: Option<TankId>,
    pub position: [f32; 3],
    pub velocity_mps: [f32; 3],
    pub shell_type: ShellType,
    pub caliber_mm: f32,
    pub drag_per_s: f32,
    pub age_seconds: f32,
    /// The concrete round in flight (protocol v47) — identity for presentation only; flight
    /// stays keyed on the explicit `drag_per_s`/`caliber_mm` above, never on a catalog lookup.
    #[serde(default)]
    pub round: Option<game_core::RoundId>,
}

impl From<&sim::ShellState> for ShellSnapshot {
    fn from(shell: &sim::ShellState) -> Self {
        Self {
            shell_id: shell.id,
            // The sim always knows the owner; the per-viewer filter decides whether the viewer
            // may. `Some` here, possibly anonymized to `None` in `filtered_for_viewer_gated`.
            owner: Some(shell.owner),
            position: shell.position.to_array(),
            velocity_mps: shell.velocity_mps.to_array(),
            shell_type: shell.shell.shell_type,
            caliber_mm: shell.shell.caliber_mm,
            drag_per_s: shell.shell.drag_per_s(),
            age_seconds: shell.age_seconds,
            round: shell.shell.round,
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
    /// The battle's crater ledger (protocol v31), quantized and re-sent whole every snapshot so
    /// a late joiner converges on the same deformed ground. `serde(default)` keeps pre-v31
    /// fixtures loading with virgin terrain.
    #[serde(default)]
    pub craters: Vec<terrain::CraterRecord>,
    /// Shell wounds on static-cover faces (protocol v32): purely visual, re-sent whole every
    /// snapshot (a late joiner converges). `serde(default)` keeps pre-v32 fixtures unwounded.
    #[serde(default)]
    pub cover_scars: Vec<terrain::CoverScar>,
    /// Guns that fired this tick (protocol v41) — the shot as an EVENT rather than as a jump in
    /// somebody's reload clock. `serde(default)` keeps pre-v41 fixtures loading silent.
    #[serde(default)]
    pub shots_fired: Vec<game_core::ShotFired>,
}

impl From<&SimulationState> for Snapshot {
    fn from(state: &SimulationState) -> Self {
        Self {
            server_tick: state.tick(),
            tanks: state.tanks().iter().map(TankSnapshot::from).collect(),
            shells: state.shells().iter().map(ShellSnapshot::from).collect(),
            damage_events: state.damage_events().to_vec(),
            shell_impacts: state.shell_impacts().to_vec(),
            shots_fired: state.shots_fired().to_vec(),
            detached_turrets: state
                .tanks()
                .iter()
                .filter(|tank| tank.turret_detached)
                .map(|tank| tank.id)
                .collect(),
            cover_states: state.cover_states().iter().map(|state| state.phase.to_wire()).collect(),
            craters: state.craters().to_vec(),
            cover_scars: state.cover_scars().to_vec(),
        }
    }
}

/// Authoritative motion omitted from the neutral world snapshot but required when the owning
/// client rewinds its predictor and replays inputs that the server has not acknowledged yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AuthoritativeMotion {
    pub velocity_mps: [f32; 3],
    pub hull_yaw_velocity_rad_s: f32,
}

/// Per-recipient delivery metadata. ACK state belongs to a connection, not to the battle world,
/// so it deliberately wraps rather than pollutes [`Snapshot`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDelivery {
    pub session_id: u64,
    pub snapshot: Snapshot,
    pub last_processed_input_seq: Option<u64>,
    pub local_motion: AuthoritativeMotion,
}

/// One perforation carved into one vehicle (protocol v39).
///
/// The breach is the exact value the authoritative `ArmorBreachSet::add` was given, so a client
/// replaying it through the same call reproduces the same set — the merge and the capacity
/// decisions both depend on the ORDER breaches arrive in, which the lane preserves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmorBreachDelta {
    pub tank: TankId,
    pub breach: ArmorBreach,
}

/// The reliable combat-feedback payloads. World state remains newest-wins snapshots; this lane
/// exists for consequences whose meaning cannot be reconstructed after a lost packet — either
/// because they are one-shot (a hit, a kill, a shell's terminal) or because they are PERMANENT
/// and a snapshot cannot afford to keep repeating them (a perforation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CombatEvent {
    Damage(DamageEvent),
    ShellImpact(ShellImpact),
    /// v39. Unlike the two above, this is not personal: it goes to every crew, because a tank
    /// that is invisible now may be visible later and a viewer who missed its perforations could
    /// never dress it correctly again.
    ArmorBreach(ArmorBreachDelta),
}

/// One per-recipient stream item. `delivery_seq` is continuous for this session even though the
/// underlying battle event ids may have gaps after audience filtering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequencedCombatEvent {
    pub delivery_seq: u64,
    pub event: CombatEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtocolMessage {
    Input(ClientInputCommand),
    VehicleSelection(ClientVehicleSelection),
    Snapshot(Snapshot),
    Ping {
        session_id: u64,
        client_time_us: u64,
    },
    Pong {
        session_id: u64,
        client_time_us: u64,
        server_time_us: u64,
    },
    ClientHello {
        session_id: u64,
        protocol_version: u16,
    },
    /// The server's opening word (protocol v18): which map to generate locally and which
    /// weather to dress it in. Both sides run the same deterministic map generator, so these
    /// two ids are all it takes to agree on a world.
    ServerHello {
        session_id: u64,
        protocol_version: u16,
        map_id: MapId,
        weather: MatchWeather,
        /// v35: the golden compile hash of the map the server is PLAYING
        /// (`map_forge::battlefield_hash`). The client compiles the same document and
        /// compares - a mismatched world fails LOUD at the door instead of desyncing a
        /// battle (the map never crosses the wire; this hash is the pairing's proof).
        map_content_hash: u64,
    },
    /// The client's per-tick send over a LOSSY wire. v37 carries the oldest unacknowledged
    /// window until the lightweight `InputAck` advances it; the server consumes each sequence
    /// once and holds only continuous axes across a gap.
    InputBatch {
        session_id: u64,
        commands: Vec<ClientInputCommand>,
    },
    /// v28: an orderly goodbye, so a leaving client frees its lobby slot immediately instead
    /// of aging out through the heartbeat timeout.
    Disconnect {
        session_id: u64,
        reason: DisconnectReason,
    },
    /// v29: the waiting room — who is here, how many the battle wants, when it starts anyway.
    LobbyState {
        session_id: u64,
        players: u8,
        needed: u8,
        countdown_ticks: u64,
    },
    /// v29: the battle begins; this client drives `assigned_tank`.
    StartBattle {
        session_id: u64,
        assigned_tank: TankId,
        server_tick: u64,
        /// v45: the authoritative sim tick at which the clock expires, or `None` for an untimed
        /// battle. Constant for the whole battle, so it rides the seat word once instead of
        /// every snapshot; the client counts down locally against `server_tick`. Without it a
        /// remote HUD had to HIDE the timer (it knew the current tick but not the deadline).
        time_limit_tick: Option<u64>,
    },
    /// v29: the battle is over. `winning_team` is `None` for a draw.
    BattleEnded {
        session_id: u64,
        winning_team: Option<u16>,
    },
    /// v36: newest-wins world state plus this recipient's input ACK/reconciliation motion.
    SnapshotDelivery(SnapshotDelivery),
    /// v37: lightweight progress between snapshot deliveries. Snapshot delivery keeps carrying
    /// the reconciliation-aligned ACK as well.
    InputAck {
        session_id: u64,
        last_processed_input_seq: u64,
    },
    /// v38: oldest-unacknowledged personal combat events, sized by the sender to one datagram.
    CombatEventBatch {
        session_id: u64,
        events: Vec<SequencedCombatEvent>,
    },
    /// v38: highest contiguous combat-event delivery sequence accepted by the client.
    CombatEventAck {
        session_id: u64,
        last_received_seq: u64,
    },
}

impl ProtocolMessage {
    /// The remote connection incarnation this message belongs to. The three legacy/local-play
    /// payloads intentionally remain untagged and must not be accepted as remote-session traffic.
    pub fn session_id(&self) -> Option<u64> {
        match self {
            ProtocolMessage::Ping { session_id, .. }
            | ProtocolMessage::Pong { session_id, .. }
            | ProtocolMessage::ClientHello { session_id, .. }
            | ProtocolMessage::ServerHello { session_id, .. }
            | ProtocolMessage::InputBatch { session_id, .. }
            | ProtocolMessage::Disconnect { session_id, .. }
            | ProtocolMessage::LobbyState { session_id, .. }
            | ProtocolMessage::StartBattle { session_id, .. }
            | ProtocolMessage::BattleEnded { session_id, .. }
            | ProtocolMessage::InputAck { session_id, .. }
            | ProtocolMessage::CombatEventBatch { session_id, .. }
            | ProtocolMessage::CombatEventAck { session_id, .. } => Some(*session_id),
            ProtocolMessage::SnapshotDelivery(delivery) => Some(delivery.session_id),
            ProtocolMessage::VehicleSelection(selection) => Some(selection.session_id),
            ProtocolMessage::Input(_) | ProtocolMessage::Snapshot(_) => None,
        }
    }
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
    /// The client stopped acknowledging its reliable personal combat stream. Dropping events
    /// silently would lie about hits/kills, so the session fails loud instead.
    CombatEventOverflow,
}

/// Wire codec for all protocol messages. Byte-compatible with bincode's standalone
/// `serialize`/`deserialize` (little-endian, fixed-int) but additionally **rejects
/// trailing bytes**, so a prefix of a longer/newer message can no longer be silently
/// mis-decoded as a valid shorter one.
fn wire_codec() -> impl Options {
    // Cap deserialization allocation at the largest frame the reassembler can ever hand us
    // (`MAX_FRAGMENTS * MAX_DATAGRAM_PAYLOAD`). Today every wire `Vec` rides the safe seq path and
    // the 32 KB reassembler window already bounds input, so this changes no accepted message — but
    // it makes the bound EXPLICIT rather than emergent, so the day a `String`/`ByteBuf` field is
    // added (bincode pre-allocates those from the declared length, before reading) it cannot become
    // an allocation bomb. The limit only rejects; it never alters the byte encoding, so wire goldens
    // are untouched.
    const WIRE_LIMIT: u64 =
        transport::MAX_FRAGMENTS as u64 * transport::MAX_DATAGRAM_PAYLOAD as u64;
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .with_limit(WIRE_LIMIT)
}

pub fn encode_message(message: &ProtocolMessage) -> Result<Vec<u8>, NetError> {
    Ok(wire_codec().serialize(message)?)
}

pub fn decode_message(bytes: &[u8]) -> Result<ProtocolMessage, NetError> {
    Ok(wire_codec().deserialize(bytes)?)
}
