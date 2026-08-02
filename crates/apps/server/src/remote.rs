//! The dedicated battle host (N2): one process, one transport, up to seven human crews and bots
//! for every empty seat. The lobby waits for players or a deadline, the battle runs the same
//! `LocalAuthoritativeServer` core the desktop game uses, and every snapshot leaves the building
//! ALREADY filtered per viewer — the anti-wallhack cut happens here, before the wire, unioning
//! team-shared intel (radio permitting) with each crew's own eyes.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;

use game_core::TankId;
use net::session::Endpoint;
use net::transport::Transport;
use net::{CombatEvent, DisconnectReason, ProtocolMessage, SnapshotDelivery};

use crate::battle::BattleOutcome;
use crate::remote_events::{RemoteEventQueue, RemoteEventQueueError};
use crate::remote_input::RemoteInputQueue;
use crate::{LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};

/// How many humans the lobby wants before starting early.
pub const LOBBY_FULL_PLAYERS: usize = 7;
/// A joined client that stays silent this long (ms) is dropped.
const CLIENT_TIMEOUT_MS: u64 = 10_000;
/// How many times the battle-over word is repeated (unreliable wire, no ack lane needed).
const BATTLE_ENDED_REPEATS: u32 = 20;
const RETIRED_SESSION_IDS: usize = 4;

struct RemoteClient {
    endpoint: Endpoint,
    session_id: u64,
    retired_session_ids: VecDeque<u64>,
    tank: Option<TankId>,
    /// The first InputBatch is the implicit ack that the crew knows its seat; until then the
    /// host re-sends StartBattle every tick (the wire is lossy and that one word must land).
    acked_start: bool,
    inputs: RemoteInputQueue,
    events: RemoteEventQueue,
    event_overflowed: bool,
    last_heard_ms: u64,
}

impl RemoteClient {
    fn new(peer: SocketAddr, now_ms: u64) -> Self {
        Self {
            endpoint: Endpoint::new(peer),
            session_id: 0,
            retired_session_ids: VecDeque::new(),
            tank: None,
            acked_start: false,
            inputs: RemoteInputQueue::default(),
            events: RemoteEventQueue::default(),
            event_overflowed: false,
            last_heard_ms: now_ms,
        }
    }

    fn begin_session(&mut self, peer: SocketAddr, session_id: u64, now_ms: u64) -> bool {
        if self.session_id == session_id {
            self.last_heard_ms = now_ms;
            return true;
        }
        if self.retired_session_ids.contains(&session_id) {
            return false;
        }
        if self.session_id != 0 {
            self.retired_session_ids.push_back(self.session_id);
            if self.retired_session_ids.len() > RETIRED_SESSION_IDS {
                self.retired_session_ids.pop_front();
            }
        }
        self.endpoint = Endpoint::new(peer);
        self.session_id = session_id;
        self.inputs = RemoteInputQueue::default();
        self.events = RemoteEventQueue::default();
        self.event_overflowed = false;
        self.acked_start = false;
        self.last_heard_ms = now_ms;
        true
    }

    fn belongs_to(&self, session_id: u64) -> bool {
        self.session_id == session_id
    }
}

enum Phase {
    Lobby { deadline_ms: u64 },
    Running { core: Box<LocalAuthoritativeServer>, ended_repeats: u32 },
}

/// Seats whose crew vanished mid-battle: a reconnecting (or fresh) client claims one and the
/// tank goes back under human control — the late joiner converges from the very first snapshot,
/// because a snapshot IS the full battle state (breaches, cover, turrets and all).
#[derive(Default)]
struct FreeSeats(Vec<TankId>);

/// The whole host. Drive [`RemoteBattleServer::pump`] as often as you like (it drains the wire)
/// and [`RemoteBattleServer::tick`] at the simulation cadence.
pub struct RemoteBattleServer {
    config: ServerTickConfig,
    battle: RandomBattleConfig,
    clients: HashMap<SocketAddr, RemoteClient>,
    free_seats: FreeSeats,
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
            free_seats: FreeSeats::default(),
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

    /// The final lifecycle word has had its full unreliable-wire repeat window. A one-battle
    /// dedicated process may now exit cleanly instead of simulating an ended match forever.
    pub fn is_finished(&self) -> bool {
        matches!(
            &self.phase,
            Phase::Running {
                core,
                ended_repeats
            } if core.battle_outcome().is_some() && *ended_repeats >= BATTLE_ENDED_REPEATS
        )
    }

    /// Drain the wire: hellos join the lobby, input batches fill each crew's ordered queue,
    /// pings echo, goodbyes free the seat immediately.
    pub fn pump(&mut self, now_ms: u64, transport: &mut dyn Transport) {
        let input_server_tick = match &self.phase {
            Phase::Lobby { .. } => 0,
            Phase::Running { core, .. } => core.authoritative_tick(),
        };
        while let Ok(Some((from, datagram))) = transport.recv() {
            let client =
                self.clients.entry(from).or_insert_with(|| RemoteClient::new(from, now_ms));
            let Ok(Some(message)) = client.endpoint.accept(&datagram) else {
                continue;
            };
            match message {
                ProtocolMessage::ClientHello { session_id, .. } => {
                    if !client.begin_session(from, session_id, now_ms) {
                        continue;
                    }
                    // The version gate already ran in decode (a stale client cannot even parse).
                    let hello = ProtocolMessage::ServerHello {
                        session_id,
                        protocol_version: net::PROTOCOL_VERSION,
                        map_id: self.battle.map,
                        weather: crate::match_info::pick_weather(self.battle.map, self.battle.seed),
                        map_content_hash: map_forge::battlefield_hash(&map_forge::battlefield(
                            self.battle.map,
                        )),
                    };
                    let _ = client.endpoint.send(transport, &hello);
                    // Mid-battle hello: a late joiner (or a reconnect) takes a freed seat, or is
                    // refused honestly — never left hanging in a lobby that will not come.
                    if let Phase::Running { core, .. } = &self.phase {
                        if client.tank.is_none() {
                            if let Some(tank) = self.free_seats.0.pop() {
                                client.tank = Some(tank);
                            } else {
                                let refuse = ProtocolMessage::Disconnect {
                                    session_id,
                                    reason: DisconnectReason::Refused,
                                };
                                let _ = client.endpoint.send(transport, &refuse);
                                continue;
                            }
                        }
                        if let Some(tank) = client.tank {
                            let start = ProtocolMessage::StartBattle {
                                session_id,
                                assigned_tank: tank,
                                server_tick: core.authoritative_tick(),
                            };
                            let _ = client.endpoint.send(transport, &start);
                            // v39: the world's EXISTING perforations, once, before the stream of
                            // additions. `begin_session` resets the lane, so a reconnect is
                            // seeded exactly like a fresh joiner — and without this baseline the
                            // additions have nothing to apply to and the crew would see
                            // undamaged steel for the rest of the battle.
                            queue_armor_breaches(client, &core.armor_breach_state());
                        }
                    }
                }
                ProtocolMessage::InputBatch { session_id, commands } => {
                    if client.belongs_to(session_id)
                        && let Some(tank) = client.tank
                        && client.inputs.ingest_batch(tank, &commands, input_server_tick)
                    {
                        client.acked_start = true;
                        client.last_heard_ms = now_ms;
                    }
                }
                ProtocolMessage::CombatEventAck { session_id, last_received_seq }
                    if client.belongs_to(session_id) =>
                {
                    client.events.acknowledge(last_received_seq);
                    client.last_heard_ms = now_ms;
                }
                // Untagged legacy traffic is never accepted by the remote host.
                ProtocolMessage::Input(_) => {}
                ProtocolMessage::Ping { session_id, client_time_us }
                    if client.belongs_to(session_id) =>
                {
                    client.last_heard_ms = now_ms;
                    let pong = ProtocolMessage::Pong {
                        session_id,
                        client_time_us,
                        server_time_us: now_ms * 1_000,
                    };
                    let _ = client.endpoint.send(transport, &pong);
                }
                ProtocolMessage::Disconnect { session_id, .. } if client.belongs_to(session_id) => {
                    if let Some(gone) = self.clients.remove(&from)
                        && let Some(tank) = gone.tank
                    {
                        self.free_seats.0.push(tank);
                    }
                }
                _ => {}
            }
        }
        // Silent clients age out; their tank falls to the last-command hold, then idles until a
        // late joiner claims the freed seat.
        let free_seats = &mut self.free_seats;
        self.clients.retain(|_, client| {
            let alive = now_ms.saturating_sub(client.last_heard_ms) < CLIENT_TIMEOUT_MS;
            if !alive && let Some(tank) = client.tank {
                free_seats.0.push(tank);
            }
            alive
        });
    }

    /// One authoritative step at the simulation cadence: run the lobby clock or the battle tick,
    /// and send every client ITS OWN filtered view.
    pub fn tick(&mut self, now_ms: u64, transport: &mut dyn Transport) {
        match &mut self.phase {
            Phase::Lobby { deadline_ms } => {
                let players = self.clients.len();
                let deadline = *deadline_ms;
                let countdown_ms = deadline.saturating_sub(now_ms);
                for client in self.clients.values_mut() {
                    let lobby = ProtocolMessage::LobbyState {
                        session_id: client.session_id,
                        players: players as u8,
                        needed: LOBBY_FULL_PLAYERS as u8,
                        countdown_ticks: countdown_ms / 50,
                    };
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
                    .values_mut()
                    .filter_map(|client| {
                        let tank = client.tank?;
                        Some((tank, client.inputs.command_for_tick(tick)))
                    })
                    .collect();
                let result = core.tick_with_inputs(&inputs);

                // Personal one-shot feedback leaves the snapshot cadence here. Audience is fixed
                // at the event's authoritative tick: the shooter and target get damage truth;
                // only the shooter gets its absorbed-shell terminal. Retransmission never
                // re-evaluates later spotting and therefore cannot leak changing visibility.
                for client in self.clients.values_mut() {
                    queue_personal_combat_events(
                        client,
                        &result.damage_events,
                        &result.shell_impacts,
                    );
                    // Perforations are NOT personal (v39): a hull invisible to this crew now may
                    // be visible later, and a viewer that missed its perforations could never
                    // dress it correctly again. Everyone gets every breach, once, in order.
                    queue_armor_breaches(client, &result.armor_breaches);
                }

                // A tiny independent ACK keeps input progress moving even when a fragmented
                // world snapshot is lost. Repeat it every tick; reordered duplicates are cheap.
                for client in self.clients.values_mut() {
                    if let Some(last_processed_input_seq) = client.inputs.last_processed() {
                        let ack = ProtocolMessage::InputAck {
                            session_id: client.session_id,
                            last_processed_input_seq,
                        };
                        let _ = client.endpoint.send(transport, &ack);
                    }
                }

                // The oldest unacknowledged combat prefix gets its own single-datagram lane.
                // A lost batch or ACK is answered by the next tick; no duplicate reaches FX.
                for client in self.clients.values_mut() {
                    if client.event_overflowed {
                        let overflow = ProtocolMessage::Disconnect {
                            session_id: client.session_id,
                            reason: DisconnectReason::CombatEventOverflow,
                        };
                        let _ = client.endpoint.send(transport, &overflow);
                        continue;
                    }
                    match client.events.batch(client.session_id) {
                        Ok(batch) => {
                            if let Err(error) = client.endpoint.send(transport, &batch) {
                                tracing::warn!(%error, "combat event batch send failed");
                            }
                        }
                        Err(RemoteEventQueueError::Empty) => {}
                        Err(error) => {
                            tracing::error!(%error, "combat event lane cannot make progress");
                            client.event_overflowed = true;
                            let overflow = ProtocolMessage::Disconnect {
                                session_id: client.session_id,
                                reason: DisconnectReason::CombatEventOverflow,
                            };
                            let _ = client.endpoint.send(transport, &overflow);
                        }
                    }
                }

                // Until the first InputBatch acks the seat, keep repeating the one word a
                // crew cannot play without.
                let acked_tick = core.authoritative_tick();
                for client in self.clients.values_mut() {
                    if let Some(tank) = client.tank
                        && !client.acked_start
                    {
                        let start = ProtocolMessage::StartBattle {
                            session_id: client.session_id,
                            assigned_tank: tank,
                            server_tick: acked_tick,
                        };
                        let _ = client.endpoint.send(transport, &start);
                    }
                }

                if let Some(snapshot) = result.snapshot {
                    let observers = core.observer_masks();
                    for client in self.clients.values_mut() {
                        let Some(tank) = client.tank else { continue };
                        let viewer_index = snapshot
                            .tanks
                            .iter()
                            .position(|t| t.tank_id == tank)
                            .unwrap_or(usize::MAX);
                        let mut cut = snapshot.filtered_for_viewer_with_observers(
                            tank,
                            &observers,
                            viewer_index,
                        );
                        // Personal events travel exactly once through the reliable lane above.
                        // Keep third-party visible/world feedback best-effort in snapshots for
                        // now, but never double-play the shooter's or target's own cues.
                        strip_reliable_personal_events(&mut cut, tank);
                        let delivery = SnapshotDelivery {
                            session_id: client.session_id,
                            snapshot: cut,
                            last_processed_input_seq: client.inputs.last_processed(),
                            local_motion: core.authoritative_motion(tank).unwrap_or_default(),
                        };
                        // NEVER `let _ =` here. A snapshot too large for the transport's
                        // fragment budget (see `net::transport::MAX_FRAGMENTS`) fails on send,
                        // and swallowing that error stops world state for this crew for the
                        // rest of the battle WITHOUT closing the session: input ACKs and the
                        // reliable combat lane keep flowing, so the client shows a frozen
                        // world rather than a lost connection. Loud is the minimum.
                        if let Err(error) = client
                            .endpoint
                            .send(transport, &ProtocolMessage::SnapshotDelivery(delivery))
                        {
                            tracing::error!(
                                %error,
                                tank = tank.0,
                                "snapshot delivery failed — this crew is receiving no world state"
                            );
                        }
                    }
                }

                if let Some(outcome) = core.battle_outcome()
                    && *ended_repeats < BATTLE_ENDED_REPEATS
                {
                    *ended_repeats += 1;
                    // `winning_team()` is the single answer to "who won?", so a new outcome
                    // variant cannot arrive here meaning one thing and be reported as another.
                    let winner = outcome.winning_team().map(|team| team.0);
                    for client in self.clients.values_mut() {
                        let word = ProtocolMessage::BattleEnded {
                            session_id: client.session_id,
                            winning_team: winner,
                        };
                        let _ = client.endpoint.send(transport, &word);
                        if *ended_repeats == BATTLE_ENDED_REPEATS {
                            let bye = ProtocolMessage::Disconnect {
                                session_id: client.session_id,
                                reason: DisconnectReason::BattleOver,
                            };
                            let _ = client.endpoint.send(transport, &bye);
                        }
                    }
                }

                // Overflow is a loud session failure, never a silent hit/kill drop. Free its seat
                // after the refusal word has had one send opportunity.
                let free_seats = &mut self.free_seats;
                self.clients.retain(|_, client| {
                    if client.event_overflowed
                        && let Some(tank) = client.tank
                    {
                        free_seats.0.push(tank);
                    }
                    !client.event_overflowed
                });
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
            let start = ProtocolMessage::StartBattle {
                session_id: client.session_id,
                assigned_tank: tank,
                server_tick,
            };
            let _ = client.endpoint.send(transport, &start);
        }
        self.phase = Phase::Running { core: Box::new(core), ended_repeats: 0 };
    }
}

/// Queue this tick's perforations for one crew. Unlike the personal events beside them these go
/// to everyone — see the call site.
fn queue_armor_breaches(
    client: &mut RemoteClient,
    breaches: &[sim::event_stamp::ArmorBreachRecord],
) {
    if client.tank.is_none() {
        return;
    }
    for record in breaches {
        if client
            .events
            .enqueue(CombatEvent::ArmorBreach(net::ArmorBreachDelta {
                tank: record.tank,
                breach: record.breach.clone(),
            }))
            .is_err()
        {
            client.event_overflowed = true;
            return;
        }
    }
}

fn queue_personal_combat_events(
    client: &mut RemoteClient,
    damage_events: &[game_core::DamageEvent],
    shell_impacts: &[game_core::ShellImpact],
) {
    let Some(tank) = client.tank else { return };
    for event in damage_events {
        if (event.source == tank || event.target == tank)
            && client.events.enqueue(CombatEvent::Damage(*event)).is_err()
        {
            client.event_overflowed = true;
            return;
        }
    }
    for impact in shell_impacts {
        if impact.owner == tank && client.events.enqueue(CombatEvent::ShellImpact(*impact)).is_err()
        {
            client.event_overflowed = true;
            return;
        }
    }
}

fn strip_reliable_personal_events(snapshot: &mut net::Snapshot, tank: TankId) {
    snapshot.damage_events.retain(|event| event.source != tank && event.target != tank);
    snapshot.shell_impacts.retain(|impact| impact.owner != tank);
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn ended_battle_finishes_after_the_full_repeat_window() {
        let config = ServerTickConfig::default();
        let battle = RandomBattleConfig {
            seed: crate::BattleSeed::fixed(0xE0_D0),
            player_vehicle: game_core::VehicleKind::T54_1951,
            map: terrain::MapId::default(),
        };
        let mut host = RemoteBattleServer::new(config, battle, 0, 0);
        let mut core = LocalAuthoritativeServer::new_random_7v7(config, battle);
        core.override_battle_time_limit_ticks(Some(1));
        host.phase = Phase::Running { core: Box::new(core), ended_repeats: 0 };
        let hub = net::transport::MemoryHub::new();
        let mut port = hub.port("10.24.0.1:40000".parse().expect("address"));

        for tick in 0..BATTLE_ENDED_REPEATS {
            host.tick(u64::from(tick) * 17, &mut port);
            assert_eq!(
                host.is_finished(),
                tick + 1 == BATTLE_ENDED_REPEATS,
                "the process exits only after every repeat opportunity"
            );
        }
    }

    #[test]
    fn personal_events_use_the_reliable_lane_and_leave_no_snapshot_duplicates() {
        let peer = "10.27.0.2:5000".parse().expect("address");
        let mut client = RemoteClient::new(peer, 0);
        client.begin_session(peer, 38, 0);
        client.tank = Some(TankId(7));
        let personal_damage = game_core::DamageEvent {
            source: TankId(7),
            target: TankId(8),
            damage_hp: 300,
            ..Default::default()
        };
        let incoming_damage = game_core::DamageEvent {
            source: TankId(9),
            target: TankId(7),
            damage_hp: 200,
            ..Default::default()
        };
        let third_party_damage = game_core::DamageEvent {
            source: TankId(9),
            target: TankId(10),
            damage_hp: 100,
            ..Default::default()
        };
        let own_impact = game_core::ShellImpact { owner: TankId(7), ..Default::default() };
        let foreign_impact = game_core::ShellImpact { owner: TankId(9), ..Default::default() };

        queue_personal_combat_events(
            &mut client,
            &[personal_damage, incoming_damage, third_party_damage],
            &[own_impact, foreign_impact],
        );
        let batch = client.events.batch(38).expect("personal batch");
        let ProtocolMessage::CombatEventBatch { events, .. } = batch else {
            panic!("expected reliable batch");
        };
        assert_eq!(events.len(), 3, "outgoing damage, incoming damage and own impact are personal");

        let mut snapshot = net::Snapshot {
            damage_events: vec![personal_damage, incoming_damage, third_party_damage],
            shell_impacts: vec![own_impact, foreign_impact],
            ..Default::default()
        };
        strip_reliable_personal_events(&mut snapshot, TankId(7));
        assert_eq!(snapshot.damage_events, vec![third_party_damage]);
        assert_eq!(snapshot.shell_impacts, vec![foreign_impact]);
    }
}
