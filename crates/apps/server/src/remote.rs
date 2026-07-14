//! The dedicated battle host (N2): one process, one transport, up to seven human crews and bots
//! for every empty seat. The lobby waits for players or a deadline, the battle runs the same
//! `LocalAuthoritativeServer` core the desktop game uses, and every snapshot leaves the building
//! ALREADY filtered per viewer — the anti-wallhack cut happens here, before the wire, unioning
//! team-shared intel (radio permitting) with each crew's own eyes.

use std::collections::HashMap;
use std::net::SocketAddr;

use game_core::TankId;
use net::session::Endpoint;
use net::transport::Transport;
use net::{ClientInputCommand, DisconnectReason, ProtocolMessage};

use crate::battle::BattleOutcome;
use crate::{LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};

/// How many humans the lobby wants before starting early.
pub const LOBBY_FULL_PLAYERS: usize = 7;
/// A joined client that stays silent this long (ms) is dropped.
const CLIENT_TIMEOUT_MS: u64 = 10_000;
/// A dead client's tank keeps repeating its last command for this many ticks, then idles.
const LAST_COMMAND_HOLD_TICKS: u64 = 30;
/// How many times the battle-over word is repeated (unreliable wire, no ack lane needed).
const BATTLE_ENDED_REPEATS: u32 = 20;

struct RemoteClient {
    endpoint: Endpoint,
    tank: Option<TankId>,
    last_command: sim::TankCommand,
    last_command_tick: u64,
    last_heard_ms: u64,
}

enum Phase {
    Lobby { deadline_ms: u64 },
    Running { core: Box<LocalAuthoritativeServer>, ended_repeats: u32 },
}

/// The whole host. Drive [`RemoteBattleServer::pump`] as often as you like (it drains the wire)
/// and [`RemoteBattleServer::tick`] at the simulation cadence.
pub struct RemoteBattleServer {
    config: ServerTickConfig,
    battle: RandomBattleConfig,
    clients: HashMap<SocketAddr, RemoteClient>,
    phase: Phase,
}

impl RemoteBattleServer {
    pub fn new(
        config: ServerTickConfig,
        battle: RandomBattleConfig,
        lobby_wait_ms: u64,
        now_ms: u64,
    ) -> Self {
        Self {
            config,
            battle,
            clients: HashMap::new(),
            phase: Phase::Lobby { deadline_ms: now_ms + lobby_wait_ms },
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Running { .. })
    }

    pub fn battle_outcome(&self) -> Option<BattleOutcome> {
        match &self.phase {
            Phase::Running { core, .. } => core.battle_outcome(),
            Phase::Lobby { .. } => None,
        }
    }

    /// Drain the wire: hellos join the lobby, input batches update each crew's latest command,
    /// pings echo, goodbyes free the seat immediately.
    pub fn pump(&mut self, now_ms: u64, transport: &mut dyn Transport) {
        while let Ok(Some((from, datagram))) = transport.recv() {
            let client = self.clients.entry(from).or_insert_with(|| RemoteClient {
                endpoint: Endpoint::new(from),
                tank: None,
                last_command: sim::TankCommand::idle(),
                last_command_tick: 0,
                last_heard_ms: now_ms,
            });
            let Ok(Some(message)) = client.endpoint.accept(&datagram) else {
                continue;
            };
            client.last_heard_ms = now_ms;
            match message {
                ProtocolMessage::ClientHello { .. } => {
                    // The version gate already ran in decode (a stale client cannot even parse).
                    let hello = ProtocolMessage::ServerHello {
                        protocol_version: net::PROTOCOL_VERSION,
                        map_id: self.battle.map,
                        weather_variant: crate::match_info::pick_weather(
                            self.battle.map,
                            self.battle.seed,
                        ),
                    };
                    let _ = client.endpoint.send(transport, &hello);
                }
                ProtocolMessage::InputBatch { commands } => {
                    apply_input_batch(client, &commands);
                }
                ProtocolMessage::Input(input) => {
                    apply_input_batch(client, std::slice::from_ref(&input));
                }
                ProtocolMessage::Ping { client_time_us } => {
                    let pong =
                        ProtocolMessage::Pong { client_time_us, server_time_us: now_ms * 1_000 };
                    let _ = client.endpoint.send(transport, &pong);
                }
                ProtocolMessage::Disconnect { .. } => {
                    self.clients.remove(&from);
                }
                _ => {}
            }
        }
        // Silent clients age out; their tank falls to the last-command hold, then idles.
        self.clients
            .retain(|_, client| now_ms.saturating_sub(client.last_heard_ms) < CLIENT_TIMEOUT_MS);
    }

    /// One authoritative step at the simulation cadence: run the lobby clock or the battle tick,
    /// and send every client ITS OWN filtered view.
    pub fn tick(&mut self, now_ms: u64, transport: &mut dyn Transport) {
        match &mut self.phase {
            Phase::Lobby { deadline_ms } => {
                let players = self.clients.len();
                let deadline = *deadline_ms;
                let countdown_ms = deadline.saturating_sub(now_ms);
                let lobby = ProtocolMessage::LobbyState {
                    players: players as u8,
                    needed: LOBBY_FULL_PLAYERS as u8,
                    countdown_ticks: countdown_ms / 50,
                };
                for client in self.clients.values_mut() {
                    let _ = client.endpoint.send(transport, &lobby);
                }
                if players == 0 {
                    return; // an empty lobby never starts; the deadline restarts on first join
                }
                if players >= LOBBY_FULL_PLAYERS || now_ms >= deadline {
                    self.start_battle(transport);
                }
            }
            Phase::Running { core, ended_repeats } => {
                let tick = core.authoritative_tick();
                let inputs: Vec<(TankId, sim::TankCommand)> = self
                    .clients
                    .values()
                    .filter_map(|client| {
                        let tank = client.tank?;
                        let stale =
                            tick.saturating_sub(client.last_command_tick) > LAST_COMMAND_HOLD_TICKS;
                        Some((
                            tank,
                            if stale { sim::TankCommand::idle() } else { client.last_command },
                        ))
                    })
                    .collect();
                let result = core.tick_with_inputs(&inputs);

                if let Some(snapshot) = result.snapshot {
                    let observers = core.observer_masks();
                    for client in self.clients.values_mut() {
                        let Some(tank) = client.tank else { continue };
                        let viewer_index = snapshot
                            .tanks
                            .iter()
                            .position(|t| t.tank_id == tank)
                            .unwrap_or(usize::MAX);
                        let cut = snapshot.filtered_for_viewer_with_observers(
                            tank,
                            &observers,
                            viewer_index,
                        );
                        let _ = client.endpoint.send(transport, &ProtocolMessage::Snapshot(cut));
                    }
                }

                if let Some(outcome) = core.battle_outcome()
                    && *ended_repeats < BATTLE_ENDED_REPEATS
                {
                    *ended_repeats += 1;
                    let winner = match outcome {
                        BattleOutcome::TeamEliminated { winning_team } => Some(winning_team.0),
                        BattleOutcome::Draw { .. } => None,
                    };
                    let word = ProtocolMessage::BattleEnded { winning_team: winner };
                    for client in self.clients.values_mut() {
                        let _ = client.endpoint.send(transport, &word);
                        if *ended_repeats == BATTLE_ENDED_REPEATS {
                            let bye = ProtocolMessage::Disconnect {
                                reason: DisconnectReason::BattleOver,
                            };
                            let _ = client.endpoint.send(transport, &bye);
                        }
                    }
                }
            }
        }
    }

    fn start_battle(&mut self, transport: &mut dyn Transport) {
        let humans = self.clients.len().clamp(1, LOBBY_FULL_PLAYERS);
        let (core, human_tanks) =
            LocalAuthoritativeServer::new_random_7v7_for_humans(self.config, self.battle, humans);
        let server_tick = core.authoritative_tick();
        for (client, tank) in self.clients.values_mut().zip(human_tanks) {
            client.tank = Some(tank);
            let start = ProtocolMessage::StartBattle { assigned_tank: tank, server_tick };
            let _ = client.endpoint.send(transport, &start);
        }
        self.phase = Phase::Running { core: Box::new(core), ended_repeats: 0 };
    }
}

fn apply_input_batch(client: &mut RemoteClient, commands: &[ClientInputCommand]) {
    for input in commands {
        if input.client_tick >= client.last_command_tick {
            client.last_command_tick = input.client_tick;
            client.last_command = input.command;
        }
    }
}
