//! The battle session seam (N3): the app drives ONE surface whether the authoritative battle
//! lives in-process (practice, the desktop game so far) or across a wire. `Local` wraps the
//! same `LocalAuthoritativeServer` as always; `Remote` speaks the v38 protocol to a dedicated
//! host — epoch-tagged hello/heartbeat, acknowledged `InputBatch` every tick, snapshots
//! arriving ALREADY filtered per viewer by the server. An enum, not a trait object: the two
//! variants are the whole design space, and practice-only operations (changing vehicles in the
//! garage) stay honest by matching instead of pretending the server would allow them.

use std::net::SocketAddr;
use std::time::Instant;

use battle_host::{BattleMode, LocalAuthoritativeServer};
use game_core::{TankId, TankSpec, VehicleKind};
use net::session::{ClientSession, SessionFailure, SessionState, TIMEOUT_MS};
use net::transport::Transport;
use net::{AuthoritativeMotion, ClientInputCommand, ProtocolMessage, ReplicationConfig, Snapshot};
use terrain::MapId;

use super::remote_events::RemoteCombatEventInbox;
use super::remote_input::RemoteInputHistory;

const MAX_BUFFERED_COMBAT_EVENTS: usize = 256;

pub(super) struct RemoteReconciliation {
    pub(super) motion: AuthoritativeMotion,
    pub(super) replay_inputs: Option<Vec<ClientInputCommand>>,
}

#[derive(Default)]
pub(super) struct BattleSessionTick {
    pub(super) snapshot: Option<Snapshot>,
    pub(super) reconciliation: Option<RemoteReconciliation>,
    /// Perforations carved since the last tick (protocol v39). Permanent per-hull state, so it
    /// arrives as a stream of additions on the reliable lane rather than in every snapshot;
    /// the app folds them into its own ArmorBreachSet per tank.
    pub(super) armor_breaches: Vec<net::ArmorBreachDelta>,
}

/// The minimal per-neighbour snapshot the contact predictor reads each tick — see
/// [`BattleSessionKind::neighbour_poses`]. Deliberately tiny: everything the local solver needs to
/// place a neighbour hull and nothing the full `Snapshot` carries for other consumers.
pub(crate) struct NeighbourPose {
    pub(crate) tank_id: TankId,
    pub(crate) position: [f32; 3],
    pub(crate) yaw_rad: f32,
    pub(crate) vehicle: VehicleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteTerminalReason {
    TimedOut,
    Refused,
    BattleOver,
    CombatEventOverflow,
    CombatEventGap,
    Transport,
    SnapshotStalled,
    InputBacklog,
    /// The server compiled a different map document than this client (v35 hash mismatch:
    /// a stale build or an edited blueprint). The world never crosses the wire, so agreement
    /// is the only safety — but a mismatch must END the session with a message, never crash
    /// the process. A hostile or out-of-date server cannot take the client down with it.
    MapMismatch,
}

pub(crate) enum BattleSessionKind {
    Local(Box<LocalAuthoritativeServer>),
    Remote(Box<RemoteSession>),
}

impl BattleSessionKind {
    /// Pump remote liveness before local prediction is allowed to advance. This is the gameplay
    /// gate: an ended local battle, an ended remote battle, or a terminal connection freezes the
    /// owned hull while the app keeps rendering and listening for late lifecycle words.
    pub(super) fn prepare_player_tick(&mut self) -> bool {
        match self {
            Self::Local(server) => server.battle_outcome().is_none(),
            Self::Remote(session) => {
                session.pump();
                session.accepts_player_prediction()
            }
        }
    }

    /// Drain a world delivery already accepted by `prepare_player_tick`. Terminal remote sessions
    /// still receive final state/lifecycle traffic; they simply may not predict or send input.
    pub(super) fn take_pending_remote_tick(&mut self) -> BattleSessionTick {
        match self {
            Self::Local(_) => BattleSessionTick::default(),
            Self::Remote(session) => session.take_pending_tick(),
        }
    }

    pub(super) fn remote_terminal_reason(&self) -> Option<RemoteTerminalReason> {
        match self {
            Self::Local(_) => None,
            Self::Remote(session) => session.terminal_reason,
        }
    }

    pub(super) fn tick_with_player_input(
        &mut self,
        input: ClientInputCommand,
    ) -> BattleSessionTick {
        match self {
            Self::Local(server) => {
                let player = server.player_tank();
                let tick = server.tick_with_player_input(input);
                BattleSessionTick {
                    snapshot: tick.snapshot,
                    // Local play has no wire and nothing to replay, but it DOES need the
                    // authoritative motion. Without it the predictor is reconciled by position
                    // only, and any velocity the hull did not give itself is silently dropped —
                    // which was invisible while the drive was the only thing that could set one,
                    // and is not any more: a contact now shoves. A shove reaching the predictor as
                    // a position correction reads as a rubber-band, not as being pushed.
                    reconciliation: server.authoritative_motion(player).map(|motion| {
                        RemoteReconciliation { motion, replay_inputs: Some(Vec::new()) }
                    }),
                    // Local play has no wire, so the sim's per-tick stream IS the delivery.
                    armor_breaches: tick
                        .armor_breaches
                        .into_iter()
                        .map(|record| net::ArmorBreachDelta {
                            tank: record.tank,
                            breach: record.breach,
                        })
                        .collect(),
                }
            }
            Self::Remote(session) => session.tick_with_player_input(input),
        }
    }

    pub fn player_tank(&self) -> TankId {
        match self {
            Self::Local(server) => server.player_tank(),
            Self::Remote(session) => session.assigned_tank.unwrap_or(TankId(0)),
        }
    }

    pub fn map_id(&self) -> MapId {
        match self {
            Self::Local(server) => server.map_id(),
            Self::Remote(session) => session.map_id,
        }
    }

    pub fn weather(&self) -> game_core::MatchWeather {
        match self {
            Self::Local(server) => server.weather(),
            Self::Remote(session) => session.weather,
        }
    }

    pub fn battle_mode(&self) -> BattleMode {
        match self {
            Self::Local(server) => server.battle_mode(),
            Self::Remote(_) => BattleMode::Random7v7,
        }
    }

    pub fn battle_outcome(&self) -> Option<battle_host::BattleOutcome> {
        match self {
            Self::Local(server) => server.battle_outcome(),
            Self::Remote(session) => session.outcome,
        }
    }

    pub fn battle_time_remaining_s(&self) -> Option<f32> {
        match self {
            Self::Local(server) => server.battle_time_remaining_s(),
            // v45: the deadline arrived once in StartBattle; the countdown runs locally against
            // the server tick the client already tracks.
            Self::Remote(session) => session.battle_time_remaining_s(),
        }
    }

    /// Test-facing observability (battle-flow tests read the clock and the FULL roster);
    /// live code consumes only the per-viewer cut.
    pub fn authoritative_tick(&self) -> u64 {
        match self {
            Self::Local(server) => server.authoritative_tick(),
            Self::Remote(session) => session.latest_server_tick,
        }
    }

    #[cfg(test)]
    pub fn latest_snapshot(&self) -> Snapshot {
        match self {
            Self::Local(server) => server.latest_snapshot(),
            Self::Remote(session) => session.latest_snapshot.clone().unwrap_or_default(),
        }
    }

    /// Test-only now that live prediction reads `neighbour_poses`: the tests still inspect the whole
    /// snapshot (full roster, every field), which is exactly what a full build is for.
    #[cfg(test)]
    pub fn current_snapshot(&self) -> Snapshot {
        match self {
            Self::Local(server) => server.current_snapshot(),
            // The wire IS the current truth for a remote battle.
            Self::Remote(session) => session.latest_snapshot.clone().unwrap_or_default(),
        }
    }

    /// The minimal per-neighbour data the client's contact predictor needs each tick — id, pose, and
    /// which vehicle (for its footprint and mass) — WITHOUT building or cloning a whole `Snapshot`
    /// (256 craters, ~800 cover scars, every hull's breach set) just to read 13 positions. Local
    /// borrows the sim's tanks; remote borrows the last snapshot's. The vehicle kind matches what
    /// `Snapshot` carries (`tank.spec.kind`), so the predictor resolves the same spec as before.
    pub fn neighbour_poses(&self) -> Vec<NeighbourPose> {
        match self {
            Self::Local(server) => server
                .tanks()
                .iter()
                .map(|tank| NeighbourPose {
                    tank_id: tank.id,
                    position: tank.position.to_array(),
                    yaw_rad: tank.yaw_rad,
                    vehicle: tank.spec.kind,
                })
                .collect(),
            Self::Remote(session) => session
                .latest_snapshot
                .as_ref()
                .map(|snapshot| {
                    snapshot
                        .tanks
                        .iter()
                        .map(|tank| NeighbourPose {
                            tank_id: tank.tank_id,
                            position: tank.position,
                            yaw_rad: tank.yaw_rad,
                            vehicle: tank.vehicle,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn latest_snapshot_for_player(&self) -> Snapshot {
        match self {
            Self::Local(server) => server.latest_snapshot_for_player(),
            // Remote snapshots arrive ALREADY filtered per viewer by the host.
            Self::Remote(session) => session.latest_snapshot.clone().unwrap_or_default(),
        }
    }

    /// Practice-only: the garage swaps vehicles by rebuilding the local battle. A remote battle
    /// refuses honestly — the server owns the roster.
    pub fn change_player_vehicle_with_spec_for_player(&mut self, spec: TankSpec) -> Snapshot {
        match self {
            Self::Local(server) => server.change_player_vehicle_with_spec_for_player(spec),
            Self::Remote(session) => session.latest_snapshot.clone().unwrap_or_default(),
        }
    }
}

/// The remote half: one dedicated host across a [`Transport`], driven once per fixed tick.
pub struct RemoteSession {
    session: ClientSession,
    transport: Box<dyn Transport>,
    started: Instant,
    pub assigned_tank: Option<TankId>,
    pub map_id: MapId,
    weather: game_core::MatchWeather,
    latest_snapshot: Option<Snapshot>,
    latest_server_tick: u64,
    /// The sim tick the clock expires on (v45, from `StartBattle`); `None` for an untimed
    /// battle or before the seat word arrives. The countdown runs locally against
    /// `latest_server_tick` so it costs no per-snapshot bytes.
    time_limit_tick: Option<u64>,
    outcome: Option<battle_host::BattleOutcome>,
    inputs: RemoteInputHistory,
    combat_events: RemoteCombatEventInbox,
    pending_combat_events: Vec<net::CombatEvent>,
    pending_reconciliation: Option<RemoteReconciliation>,
    delivery_ready: bool,
    terminal_reason: Option<RemoteTerminalReason>,
    seat_started_ms: Option<u64>,
    rtt_ms: Option<u32>,
    last_snapshot_ms: u64,
    last_stats_log_ms: u64,
    recorder: Option<net::recording::FrameRecorder<std::io::BufWriter<std::fs::File>>>,
}

impl RemoteSession {
    pub fn connect(server_addr: SocketAddr, transport: Box<dyn Transport>) -> Self {
        Self::connect_with_vehicle(server_addr, transport, None)
    }

    /// Connect and ask the lobby to seat this crew in `vehicle` (seat=vehicle, v49). The
    /// pick repeats on the handshake cadence until `StartBattle` settles the seat; `None`
    /// keeps the host's default hull.
    pub fn connect_with_vehicle(
        server_addr: SocketAddr,
        transport: Box<dyn Transport>,
        vehicle: Option<game_core::VehicleKind>,
    ) -> Self {
        let mut session = ClientSession::connect(server_addr, 0);
        if let Some(kind) = vehicle {
            session.request_vehicle(kind);
        }
        Self {
            session,
            transport,
            started: Instant::now(),
            assigned_tank: None,
            map_id: MapId::default(),
            weather: game_core::MatchWeather::default(),
            latest_snapshot: None,
            latest_server_tick: 0,
            time_limit_tick: None,
            outcome: None,
            inputs: RemoteInputHistory::default(),
            combat_events: RemoteCombatEventInbox::default(),
            pending_combat_events: Vec::new(),
            pending_reconciliation: None,
            delivery_ready: false,
            terminal_reason: None,
            seat_started_ms: None,
            rtt_ms: None,
            last_snapshot_ms: 0,
            last_stats_log_ms: 0,
            // WOT_RECORD=path appends every accepted frame: the debug recorder today, the
            // player replay's substrate tomorrow (W3/S7).
            recorder: std::env::var("WOT_RECORD").ok().and_then(|path| {
                std::fs::File::create(&path)
                    .map(|file| net::recording::FrameRecorder::new(std::io::BufWriter::new(file)))
                    .ok()
            }),
        }
    }

    /// The connection at a glance: round trip and how stale the newest state is.
    pub fn net_stats(&self, now_ms: u64) -> (Option<u32>, u64) {
        (self.rtt_ms, now_ms.saturating_sub(self.last_snapshot_ms))
    }

    pub fn state(&self) -> &SessionState {
        self.session.state()
    }

    pub fn is_seated(&self) -> bool {
        self.assigned_tank.is_some()
    }

    pub(super) fn is_terminal(&self) -> bool {
        self.terminal_reason.is_some()
    }

    fn accepts_player_prediction(&self) -> bool {
        self.assigned_tank.is_some()
            && self.outcome.is_none()
            && self.terminal_reason.is_none()
            && *self.session.state() == SessionState::Connected
    }

    /// Seconds left on the battle clock, counted locally against the last server tick (v45);
    /// `None` for an untimed battle or before the seat word carrying the deadline arrives. The
    /// client already assumes the fixed authoritative rate everywhere it converts ticks to time.
    fn battle_time_remaining_s(&self) -> Option<f32> {
        let limit = self.time_limit_tick?;
        let remaining_ticks = limit.saturating_sub(self.latest_server_tick);
        Some(remaining_ticks as f32 / sim::DEFAULT_SERVER_TICK_HZ as f32)
    }

    /// Drive the wire until the lobby seats us (called from the connecting screen).
    pub fn pump(&mut self) {
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.pump_at(now_ms);
    }

    fn pump_at(&mut self, now_ms: u64) {
        let inbox = match self.session.tick(now_ms, self.transport.as_mut()) {
            Ok(inbox) => inbox,
            Err(error) => {
                tracing::error!(%error, "remote transport failed");
                self.enter_terminal(RemoteTerminalReason::Transport);
                return;
            }
        };
        for message in inbox {
            if let ProtocolMessage::SnapshotDelivery(delivery) = &message
                && self
                    .latest_snapshot
                    .as_ref()
                    .is_some_and(|latest| delivery.snapshot.server_tick <= latest.server_tick)
            {
                continue;
            }
            if let ProtocolMessage::CombatEventBatch { events, .. } = &message {
                let (accepted, ack) = match self.combat_events.accept_batch(events) {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::error!(?error, "remote combat event stream has a sequence gap");
                        self.enter_terminal(RemoteTerminalReason::CombatEventGap);
                        continue;
                    }
                };
                if self.pending_combat_events.len().saturating_add(accepted.len())
                    > MAX_BUFFERED_COMBAT_EVENTS
                {
                    tracing::error!("remote combat event presentation buffer reached its limit");
                    self.enter_terminal(RemoteTerminalReason::CombatEventOverflow);
                    continue;
                }
                if let Some(last_received_seq) = ack {
                    let ack = ProtocolMessage::CombatEventAck {
                        session_id: self.session.session_id(),
                        last_received_seq,
                    };
                    if let Err(error) = self.session.endpoint.send(self.transport.as_mut(), &ack) {
                        tracing::error!(%error, "remote combat event ACK failed");
                        self.enter_terminal(RemoteTerminalReason::Transport);
                        continue;
                    }
                }
                // A retransmit is acknowledged again but never reaches recording or presentation.
                if accepted.is_empty() {
                    continue;
                }
                self.pending_combat_events.extend(accepted);
            }
            if let Some(recorder) = &mut self.recorder {
                let _ = recorder.record(&message);
            }
            match message {
                ProtocolMessage::Pong { client_time_us, .. } => {
                    self.rtt_ms = Some((now_ms.saturating_sub(client_time_us / 1_000)) as u32);
                }
                ProtocolMessage::StartBattle {
                    assigned_tank,
                    server_tick,
                    time_limit_tick,
                    ..
                } => {
                    match self.assigned_tank {
                        None => self.assigned_tank = Some(assigned_tank),
                        Some(current) if current == assigned_tank => {}
                        Some(_) => continue,
                    }
                    // Seated: the garage pick is settled, stop repeating it.
                    self.session.clear_vehicle_selection();
                    self.seat_started_ms.get_or_insert(now_ms);
                    self.latest_server_tick = self.latest_server_tick.max(server_tick);
                    self.time_limit_tick = time_limit_tick;
                }
                ProtocolMessage::SnapshotDelivery(delivery) => {
                    self.inputs.acknowledge_wire(delivery.last_processed_input_seq);
                    self.latest_server_tick =
                        self.latest_server_tick.max(delivery.snapshot.server_tick);
                    self.pending_reconciliation = Some(RemoteReconciliation {
                        motion: delivery.local_motion,
                        replay_inputs: self.inputs.prediction_replay_after(
                            delivery.last_processed_input_seq,
                            ReplicationConfig::default().max_prediction_ticks as usize,
                        ),
                    });
                    self.latest_snapshot = Some(delivery.snapshot);
                    self.delivery_ready = true;
                    self.last_snapshot_ms = now_ms;
                }
                ProtocolMessage::InputAck { last_processed_input_seq, .. } => {
                    self.inputs.acknowledge_wire(Some(last_processed_input_seq));
                }
                ProtocolMessage::BattleEnded { winning_team, .. } => {
                    // The wire carries WHO won, not HOW — so a remote client cannot tell an
                    // elimination from a decision on the clock, and the variant below is a stand-in
                    // for "somebody won", not a claim about the manner of it. That is honest enough
                    // today because every consumer reads this through `winning_team()` and the
                    // outcome screen has exactly three faces (Victory / Defeat / Draw). If anything
                    // ever needs the manner, it belongs on the wire rather than guessed at here.
                    self.outcome = Some(match winning_team {
                        Some(team) => battle_host::BattleOutcome::TeamAhead {
                            winning_team: game_core::TeamId(team),
                        },
                        None => battle_host::BattleOutcome::Draw {
                            reason: battle_host::DrawReason::TimeExpired,
                        },
                    });
                }
                ProtocolMessage::ServerHello { map_id, weather, map_content_hash, .. } => {
                    // The pairing's proof (v35): both ends compile the SAME document or
                    // nobody plays. A mismatch means diverged content (a stale build, an edited
                    // blueprint) - failing loud at the door beats a silent desync twenty ticks
                    // into a battle. But "loud" is a TERMINAL SESSION with a message, never a
                    // panic: a public server's hello reaches an untrusted client, and a wrong
                    // hash must not be a remote crash. The world never crosses the wire, so
                    // agreement is the only safety - and disagreement simply ends the session.
                    let ours = map_forge::battlefield_hash(&map_forge::battlefield(map_id));
                    if ours != map_content_hash {
                        tracing::warn!(
                            ?map_id,
                            server = format!("0x{map_content_hash:016x}"),
                            client = format!("0x{ours:016x}"),
                            "map content mismatch - update your build; ending the session"
                        );
                        self.enter_terminal(RemoteTerminalReason::MapMismatch);
                        continue;
                    }
                    self.map_id = map_id;
                    self.weather = weather;
                }
                _ => {}
            }
        }
        if self.terminal_reason.is_none() {
            let session_terminal = match self.session.state() {
                SessionState::Failed(SessionFailure::TimedOut) => {
                    Some(RemoteTerminalReason::TimedOut)
                }
                SessionState::Failed(SessionFailure::Refused) => {
                    Some(RemoteTerminalReason::Refused)
                }
                SessionState::Failed(SessionFailure::BattleOver) => {
                    Some(RemoteTerminalReason::BattleOver)
                }
                SessionState::Failed(SessionFailure::CombatEventOverflow) => {
                    Some(RemoteTerminalReason::CombatEventOverflow)
                }
                SessionState::Connecting | SessionState::Connected => None,
            };
            if let Some(reason) = session_terminal {
                self.enter_terminal(reason);
            }
        }
        // Heartbeats and tiny input ACKs can survive while every fragmented world snapshot is
        // being lost. They prove a socket exists, not that the battle is playable. Once seated,
        // ten seconds without fresh world truth is a terminal stalled control path.
        if self.terminal_reason.is_none()
            && self.outcome.is_none()
            && *self.session.state() == SessionState::Connected
            && let Some(seated_ms) = self.seat_started_ms
        {
            let newest_world_ms =
                if self.latest_snapshot.is_some() { self.last_snapshot_ms } else { seated_ms };
            if now_ms.saturating_sub(newest_world_ms) >= TIMEOUT_MS {
                self.enter_terminal(RemoteTerminalReason::SnapshotStalled);
            }
        }
        if self.outcome.is_some() {
            self.retire_control();
        }
        // One quiet line every 2 s: the console net-HUD until the drawn one lands.
        if now_ms.saturating_sub(self.last_stats_log_ms) >= 2_000 {
            self.last_stats_log_ms = now_ms;
            let (rtt_ms, snapshot_age_ms) = self.net_stats(now_ms);
            tracing::info!(rtt_ms, snapshot_age_ms, server_tick = self.latest_server_tick, "net");
        }
    }

    fn tick_with_player_input(&mut self, input: ClientInputCommand) -> BattleSessionTick {
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.tick_with_player_input_at(input, now_ms)
    }

    fn tick_with_player_input_at(
        &mut self,
        input: ClientInputCommand,
        now_ms: u64,
    ) -> BattleSessionTick {
        // Liveness is resolved BEFORE accepting another command. A peer that crossed its timeout
        // during this tick never grows the backlog or transmits a post-terminal edge.
        self.pump_at(now_ms);
        if self.accepts_player_prediction()
            && let Some(tank) = self.assigned_tank
        {
            if !self.inputs.enqueue(tank, input.command) {
                tracing::error!("remote input backlog reached its hard limit");
                self.enter_terminal(RemoteTerminalReason::InputBacklog);
            } else {
                let commands = self.inputs.wire_batch();
                if !commands.is_empty() {
                    let batch = ProtocolMessage::InputBatch {
                        session_id: self.session.session_id(),
                        commands,
                    };
                    let _ = self.session.endpoint.send(self.transport.as_mut(), &batch);
                }
            }
        }
        self.take_pending_tick()
    }

    fn enter_terminal(&mut self, reason: RemoteTerminalReason) {
        if self.terminal_reason.is_none() {
            self.terminal_reason = Some(reason);
            self.retire_control();
        }
    }

    fn retire_control(&mut self) {
        self.inputs.clear_pending();
        self.pending_reconciliation = None;
    }

    fn take_pending_tick(&mut self) -> BattleSessionTick {
        let mut snapshot = std::mem::take(&mut self.delivery_ready)
            .then(|| self.latest_snapshot.clone())
            .flatten();
        // Perforations leave the lane whether or not a snapshot is due this tick: they are
        // permanent state and hold no play-once cue, so gating them on snapshot cadence would
        // only delay the dressing. The personal events keep riding a snapshot, as they always
        // have — the HUD, the damage log and the FX all read them off one.
        let mut armor_breaches = Vec::new();
        let mut personal = Vec::new();
        for event in self.pending_combat_events.drain(..) {
            match event {
                net::CombatEvent::ArmorBreach(delta) => armor_breaches.push(delta),
                other => personal.push(other),
            }
        }
        match &mut snapshot {
            Some(snapshot) => {
                for event in personal {
                    match event {
                        net::CombatEvent::Damage(event) => snapshot.damage_events.push(event),
                        net::CombatEvent::ShellImpact(event) => snapshot.shell_impacts.push(event),
                        net::CombatEvent::ArmorBreach(_) => {
                            unreachable!("breaches were split out above")
                        }
                    }
                }
            }
            // Nothing to carry them on: hold the personal events for the next delivery rather
            // than dropping consequences the lane went to the trouble of making reliable.
            None => self.pending_combat_events = personal,
        }
        let reconciliation = snapshot.as_ref().and_then(|_| self.pending_reconciliation.take());
        BattleSessionTick { snapshot, reconciliation, armor_breaches }
    }
}

#[cfg(test)]
#[path = "session_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
mod tests {
    use battle_host::remote::RemoteBattleServer;
    use battle_host::{BattleSeed, RandomBattleConfig, ServerTickConfig};
    use net::transport::MemoryHub;

    use super::*;

    fn battle() -> RandomBattleConfig {
        RandomBattleConfig {
            seed: BattleSeed::fixed(33),
            player_vehicle: game_core::VehicleKind::T54_1951,
            map: MapId::default(),
        }
    }

    fn session_parity_input(
        tick: u64,
        tank: TankId,
        command: sim::TankCommand,
    ) -> ClientInputCommand {
        ClientInputCommand { client_tick: tick, tank_id: tank, command }
    }

    /// THE parity lock from the network plan: the same seed and non-idle steering, once through
    /// the in-process session and once across the wire — every snapshot the remote client
    /// receives must be BYTE-IDENTICAL to the local session's snapshot at the same server tick.
    /// Local and Remote are one game, not two.
    #[test]
    fn local_and_remote_sessions_agree_snapshot_for_snapshot() {
        // Local run: collect filtered snapshots by tick.
        let mut local = BattleSessionKind::Local(Box::new(
            LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), battle()),
        ));
        let player = local.player_tank();
        let mut local_by_tick = std::collections::HashMap::new();
        for step in 0..600_u64 {
            // The lobby transition delivers the assigned seat before the host's first running
            // tick, so sequence zero can drive authoritative tick one without a hidden idle step.
            let command = sim::TankCommand::drive(0.7, 0.2);
            let outcome = local.tick_with_player_input(session_parity_input(step, player, command));
            if let Some(snapshot) = outcome.snapshot {
                local_by_tick.insert(snapshot.server_tick, snapshot);
            }
        }

        // Remote run: same battle across the in-memory hub.
        let hub = MemoryHub::new();
        let server_addr: SocketAddr = "10.0.0.1:40000".parse().expect("addr");
        let mut server_port = hub.port(server_addr);
        let client_port = hub.port("10.0.0.9:5000".parse().expect("addr"));
        let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle(), 50, 0);
        let mut remote = BattleSessionKind::Remote(Box::new(RemoteSession::connect(
            server_addr,
            Box::new(client_port),
        )));

        let mut compared = 0_usize;
        for step in 0..700_u64 {
            let now_ms = step * 17;
            let tank = remote.player_tank();
            let outcome = remote.tick_with_player_input(session_parity_input(
                step,
                tank,
                sim::TankCommand::drive(0.7, 0.2),
            ));
            host.pump(now_ms, &mut server_port);
            host.tick(now_ms, &mut server_port);
            if let Some(snapshot) = outcome.snapshot
                && let Some(local_snapshot) = local_by_tick.get(&snapshot.server_tick)
            {
                assert_eq!(
                    &snapshot, local_snapshot,
                    "tick {}: the wire must carry the same battle",
                    snapshot.server_tick
                );
                compared += 1;
            }
        }
        assert!(compared >= 5, "the runs must actually overlap, compared {compared}");
        assert_eq!(remote.weather(), local.weather(), "wire and local match weather identity");
        let elapsed_s = remote.authoritative_tick() as f32 / sim::DEFAULT_SIMULATION_TICK_HZ as f32;
        let local_weather =
            scene_build::weather_timeline::WeatherTimeline::new(local.map_id(), local.weather());
        let remote_weather =
            scene_build::weather_timeline::WeatherTimeline::new(remote.map_id(), remote.weather());
        assert_eq!(local_weather.sample(elapsed_s), remote_weather.sample(elapsed_s));
        assert!(host.is_running());
    }
}
