//! The battle session seam (N3): the app drives ONE surface whether the authoritative battle
//! lives in-process (practice, the desktop game so far) or across a wire. `Local` wraps the
//! same `LocalAuthoritativeServer` as always; `Remote` speaks the v29 protocol to a dedicated
//! host — hello/heartbeat via `net::session`, redundant `InputBatch` every tick, snapshots
//! arriving ALREADY filtered per viewer by the server. An enum, not a trait object: the two
//! variants are the whole design space, and practice-only operations (changing vehicles in the
//! garage) stay honest by matching instead of pretending the server would allow them.

use std::net::SocketAddr;
use std::time::Instant;

use game_core::{TankId, TankSpec};
use net::session::{ClientSession, SessionState};
use net::transport::Transport;
use net::{ClientInputCommand, ProtocolMessage, Snapshot};
use server::{AuthoritativeTick, BattleMode, LocalAuthoritativeServer};
use terrain::MapId;

/// How many recent commands each InputBatch re-carries: a lost datagram costs nothing because
/// the next batch repeats the history the server has not yet consumed.
const INPUT_REDUNDANCY: usize = 3;

pub(crate) enum BattleSessionKind {
    Local(Box<LocalAuthoritativeServer>),
    Remote(Box<RemoteSession>),
}

impl BattleSessionKind {
    pub fn tick_with_player_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        match self {
            Self::Local(server) => server.tick_with_player_input(input),
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

    pub fn weather_variant(&self) -> game_core::WeatherVariant {
        match self {
            Self::Local(server) => server.weather_variant(),
            Self::Remote(session) => session.weather,
        }
    }

    pub fn battle_mode(&self) -> BattleMode {
        match self {
            Self::Local(server) => server.battle_mode(),
            Self::Remote(_) => BattleMode::Random7v7,
        }
    }

    pub fn battle_outcome(&self) -> Option<server::BattleOutcome> {
        match self {
            Self::Local(server) => server.battle_outcome(),
            Self::Remote(session) => session.outcome,
        }
    }

    pub fn battle_time_remaining_s(&self) -> Option<f32> {
        match self {
            Self::Local(server) => server.battle_time_remaining_s(),
            // The battle clock is not replicated yet (a v30 candidate); the HUD simply hides
            // the timer on a remote battle instead of showing a guess.
            Self::Remote(_) => None,
        }
    }

    /// Test-facing observability (battle-flow tests read the clock and the FULL roster);
    /// live code consumes only the per-viewer cut.
    #[cfg(test)]
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

    pub fn current_snapshot(&self) -> Snapshot {
        match self {
            Self::Local(server) => server.current_snapshot(),
            // The wire IS the current truth for a remote battle.
            Self::Remote(session) => session.latest_snapshot.clone().unwrap_or_default(),
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
    weather: game_core::WeatherVariant,
    latest_snapshot: Option<Snapshot>,
    latest_server_tick: u64,
    outcome: Option<server::BattleOutcome>,
    recent: std::collections::VecDeque<ClientInputCommand>,
}

impl RemoteSession {
    pub fn connect(server_addr: SocketAddr, transport: Box<dyn Transport>) -> Self {
        Self {
            session: ClientSession::connect(server_addr, 0),
            transport,
            started: Instant::now(),
            assigned_tank: None,
            map_id: MapId::default(),
            weather: game_core::WeatherVariant::default(),
            latest_snapshot: None,
            latest_server_tick: 0,
            outcome: None,
            recent: std::collections::VecDeque::with_capacity(INPUT_REDUNDANCY),
        }
    }

    pub fn state(&self) -> &SessionState {
        self.session.state()
    }

    pub fn is_seated(&self) -> bool {
        self.assigned_tank.is_some()
    }

    /// Drive the wire until the lobby seats us (called from the connecting screen).
    pub fn pump(&mut self) {
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.pump_at(now_ms);
    }

    fn pump_at(&mut self, now_ms: u64) {
        let Ok(inbox) = self.session.tick(now_ms, self.transport.as_mut()) else {
            return;
        };
        for message in inbox {
            match message {
                ProtocolMessage::StartBattle { assigned_tank, server_tick } => {
                    self.assigned_tank = Some(assigned_tank);
                    self.latest_server_tick = server_tick;
                }
                ProtocolMessage::Snapshot(snapshot) => {
                    self.latest_server_tick = snapshot.server_tick;
                    self.latest_snapshot = Some(snapshot);
                }
                ProtocolMessage::BattleEnded { winning_team } => {
                    self.outcome = Some(match winning_team {
                        Some(team) => server::BattleOutcome::TeamEliminated {
                            winning_team: game_core::TeamId(team),
                        },
                        None => {
                            server::BattleOutcome::Draw { reason: server::DrawReason::TimeExpired }
                        }
                    });
                }
                ProtocolMessage::ServerHello { map_id, weather_variant, .. } => {
                    self.map_id = map_id;
                    self.weather = weather_variant;
                }
                _ => {}
            }
        }
    }

    fn tick_with_player_input(&mut self, input: ClientInputCommand) -> AuthoritativeTick {
        if self.recent.len() == INPUT_REDUNDANCY {
            self.recent.pop_front();
        }
        self.recent.push_back(input);
        if self.assigned_tank.is_some() && *self.session.state() == SessionState::Connected {
            let batch =
                ProtocolMessage::InputBatch { commands: self.recent.iter().cloned().collect() };
            let _ = self.session.endpoint.send(self.transport.as_mut(), &batch);
        }
        let before = self.latest_server_tick;
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.pump_at(now_ms);
        AuthoritativeTick {
            server_tick: self.latest_server_tick,
            snapshot: (self.latest_server_tick != before)
                .then(|| self.latest_snapshot.clone())
                .flatten(),
        }
    }
}

#[cfg(test)]
mod tests {
    use net::transport::MemoryHub;
    use server::remote::RemoteBattleServer;
    use server::{BattleSeed, RandomBattleConfig, ServerTickConfig};

    use super::*;

    fn battle() -> RandomBattleConfig {
        RandomBattleConfig {
            seed: BattleSeed::fixed(33),
            player_vehicle: game_core::VehicleKind::T54_1951,
            map: MapId::default(),
        }
    }

    fn idle_input(tick: u64, tank: TankId) -> ClientInputCommand {
        ClientInputCommand { client_tick: tick, tank_id: tank, command: sim::TankCommand::idle() }
    }

    /// THE parity lock from the network plan: the same seed, the same idle player, once through
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
            let outcome = local.tick_with_player_input(idle_input(step, player));
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
            let outcome = remote.tick_with_player_input(idle_input(step, tank));
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
        assert!(host.is_running());
    }
}
