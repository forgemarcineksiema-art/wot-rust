use std::io::BufWriter;

use battle_host::remote::RemoteBattleServer;
use battle_host::{BattleSeed, RandomBattleConfig, ServerTickConfig};
use net::session::Endpoint;
use net::transport::MemoryHub;
use net::{AuthoritativeMotion, ProtocolMessage, Snapshot, SnapshotDelivery};

use super::*;

fn contract_battle() -> RandomBattleConfig {
    RandomBattleConfig {
        seed: BattleSeed::fixed(51),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: MapId::default(),
    }
}

fn contract_input(command: sim::TankCommand) -> ClientInputCommand {
    ClientInputCommand { client_tick: u64::MAX, tank_id: TankId(u64::MAX), command }
}

#[test]
fn seeded_loss_retries_one_fire_edge_until_it_is_applied_once() {
    let hub = MemoryHub::with_loss(12_345, 30);
    let server_addr: SocketAddr = "10.20.0.1:40000".parse().expect("server addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port("10.20.0.2:5000".parse().expect("client addr"));
    let mut host = RemoteBattleServer::new(ServerTickConfig::new(60, 1), contract_battle(), 100, 0);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    let spec = game_core::VehicleKind::T54_1951.spec();
    let slot = spec.ammo.initial_selected as usize;
    let initial_rounds = spec.ammo.counts[slot];
    let mut fire_queued = false;
    let mut minimum_rounds = initial_rounds;
    let mut own_snapshots = 0_usize;

    for step in 0..2_400_u64 {
        let now_ms = step * 17;
        let fire = remote.is_seated() && remote.inputs.next_sequence() >= 20 && !fire_queued;
        fire_queued |= fire;
        let command = sim::TankCommand { fire, ..sim::TankCommand::idle() };
        let outcome = remote.tick_with_player_input_at(contract_input(command), now_ms);
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
        if let (Some(tank), Some(snapshot)) = (remote.assigned_tank, outcome.snapshot)
            && let Some(own) = snapshot.tanks.iter().find(|candidate| candidate.tank_id == tank)
        {
            own_snapshots += 1;
            minimum_rounds = minimum_rounds.min(own.ammo_counts[slot]);
        }
    }

    assert!(fire_queued, "lossy handshake must still seat the remote player");
    assert!(own_snapshots >= 10, "the assertion needs real authoritative samples");
    assert_eq!(
        minimum_rounds,
        initial_rounds - 1,
        "the true edge is retried through loss, while all later false commands prevent repeats"
    );
    assert!(
        remote.inputs.len() <= ReplicationConfig::default().max_prediction_ticks as usize,
        "snapshot ACKs must keep prediction history bounded"
    );
}

#[test]
fn stale_delivery_is_rejected_before_ack_recording_and_snapshot_side_effects() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.21.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.21.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    assert!(remote.inputs.enqueue(TankId(7), sim::TankCommand::idle()));
    assert!(remote.inputs.enqueue(TankId(7), sim::TankCommand::idle()));

    let path = std::env::temp_dir().join(format!(
        "wot-stale-delivery-{}-{}.wotrec",
        std::process::id(),
        remote.started.elapsed().as_nanos()
    ));
    let file = std::fs::File::create(&path).expect("recording file");
    remote.recorder = Some(net::recording::FrameRecorder::new(BufWriter::new(file)));

    let accepted = ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
        session_id: remote.session.session_id(),
        snapshot: Snapshot { server_tick: 10, ..Snapshot::default() },
        last_processed_input_seq: Some(0),
        local_motion: AuthoritativeMotion::default(),
    });
    server.send(&mut server_port, &accepted).expect("accepted delivery");
    remote.pump_at(100);
    assert_eq!(remote.latest_snapshot.as_ref().map(|snapshot| snapshot.server_tick), Some(10));
    assert_eq!(remote.inputs.len(), 1);
    assert_eq!(remote.recorder.as_ref().map(net::recording::FrameRecorder::frames), Some(1));
    remote.delivery_ready = false;
    remote.pending_reconciliation = None;

    let stale = ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
        session_id: remote.session.session_id(),
        snapshot: Snapshot { server_tick: 9, ..Snapshot::default() },
        last_processed_input_seq: Some(1),
        local_motion: AuthoritativeMotion {
            velocity_mps: [99.0, 0.0, 0.0],
            hull_yaw_velocity_rad_s: 99.0,
        },
    });
    server.send(&mut server_port, &stale).expect("stale delivery");
    remote.pump_at(900);

    assert_eq!(remote.latest_snapshot.as_ref().map(|snapshot| snapshot.server_tick), Some(10));
    assert_eq!(remote.inputs.len(), 1, "a stale ACK cannot prune prediction history");
    assert_eq!(remote.last_snapshot_ms, 100, "stale state cannot renew snapshot freshness");
    assert!(!remote.delivery_ready, "stale state cannot become ingest-ready");
    assert!(remote.pending_reconciliation.is_none());
    assert_eq!(
        remote.recorder.as_ref().map(net::recording::FrameRecorder::frames),
        Some(1),
        "stale delivery must be dropped before replay recording"
    );
    drop(remote);
    std::fs::remove_file(path).expect("remove test recording");
}

#[test]
fn timeout_is_terminal_and_cannot_grow_the_input_history() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.22.0.1:40000".parse().expect("server addr");
    let client_port = hub.port("10.22.0.2:5000".parse().expect("client addr"));
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    remote.assigned_tank = Some(TankId(7));
    for _ in 0..12 {
        assert!(remote.inputs.enqueue(TankId(7), sim::TankCommand::drive(1.0, 0.0)));
    }
    let next_sequence = remote.inputs.next_sequence();

    remote.pump_at(net::session::TIMEOUT_MS);

    assert_eq!(remote.terminal_reason, Some(RemoteTerminalReason::TimedOut));
    assert!(!remote.accepts_player_prediction());
    assert_eq!(remote.inputs.len(), 0, "terminal transition releases every pending command");

    for step in 1..=30 {
        let outcome = remote.tick_with_player_input_at(
            contract_input(sim::TankCommand {
                fire: true,
                select_ammo: Some(2),
                ..sim::TankCommand::drive(1.0, 1.0)
            }),
            net::session::TIMEOUT_MS + step,
        );
        assert!(outcome.snapshot.is_none());
    }
    assert_eq!(
        remote.inputs.next_sequence(),
        next_sequence,
        "held controls and one-shot edges cannot enter a terminal session"
    );
    assert_eq!(remote.inputs.len(), 0);
}

#[test]
fn battle_result_survives_the_orderly_battle_over_disconnect() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.23.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.23.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    let session_id = remote.session.session_id();
    let map_id = MapId::default();

    server
        .send(
            &mut server_port,
            &ProtocolMessage::ServerHello {
                session_id,
                protocol_version: net::PROTOCOL_VERSION,
                map_id,
                weather: Default::default(),
                map_content_hash: map_forge::battlefield_hash(&map_forge::battlefield(map_id)),
            },
        )
        .expect("hello");
    server
        .send(
            &mut server_port,
            &ProtocolMessage::StartBattle {
                session_id,
                assigned_tank: TankId(7),
                server_tick: 10,
                time_limit_tick: None,
            },
        )
        .expect("start");
    server
        .send(&mut server_port, &ProtocolMessage::BattleEnded { session_id, winning_team: Some(1) })
        .expect("result");
    server
        .send(
            &mut server_port,
            &ProtocolMessage::Disconnect { session_id, reason: net::DisconnectReason::BattleOver },
        )
        .expect("goodbye");

    remote.pump_at(100);

    // What must survive the disconnect is WHO WON. The wire carries only that — not whether the
    // battle was won by elimination or on the clock — so asserting a particular variant here would
    // be pinning a detail the remote client cannot actually know.
    assert_eq!(
        remote.outcome.and_then(battle_host::BattleOutcome::winning_team),
        Some(game_core::TeamId(1)),
        "the battle result outlives the orderly close"
    );
    assert!(remote.outcome.is_some(), "and the outcome itself is retained, not just its winner");
    assert_eq!(remote.terminal_reason, Some(RemoteTerminalReason::BattleOver));
    assert!(
        !remote.accepts_player_prediction(),
        "an outcome and its orderly close both stop controls"
    );
}

#[test]
fn repeated_combat_batch_is_acked_repeatedly_but_presented_exactly_once() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.25.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.25.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    let session_id = remote.session.session_id();
    let event = net::SequencedCombatEvent {
        delivery_seq: 0,
        event: net::CombatEvent::Damage(game_core::DamageEvent {
            source: TankId(7),
            target: TankId(8),
            damage_hp: 377,
            ..Default::default()
        }),
    };
    let batch = ProtocolMessage::CombatEventBatch { session_id, events: vec![event.clone()] };
    server.send(&mut server_port, &batch).expect("first batch");
    server.send(&mut server_port, &batch).expect("retransmit");

    remote.pump_at(10);

    assert_eq!(remote.combat_events.last_received_seq(), Some(0));
    assert_eq!(
        remote.pending_combat_events.len(),
        1,
        "wire retransmit must enter presentation once"
    );
    let mut ack_count = 0;
    while let Some((_, datagram)) = server_port.recv().expect("server receive") {
        if let Ok(Some(ProtocolMessage::CombatEventAck { last_received_seq: 0, .. })) =
            server.accept(&datagram)
        {
            ack_count += 1;
        }
    }
    assert_eq!(ack_count, 2, "a lost ACK is repaired by acknowledging every retransmit");

    server
        .send(
            &mut server_port,
            &ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
                session_id,
                snapshot: Snapshot { server_tick: 10, ..Default::default() },
                ..Default::default()
            }),
        )
        .expect("world delivery");
    remote.pump_at(20);
    let first = remote.take_pending_tick().snapshot.expect("delivery");
    assert_eq!(first.damage_events.len(), 1);
    assert_eq!(first.damage_events[0].damage_hp, 377);

    server.send(&mut server_port, &batch).expect("late duplicate");
    server
        .send(
            &mut server_port,
            &ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
                session_id,
                snapshot: Snapshot { server_tick: 12, ..Default::default() },
                ..Default::default()
            }),
        )
        .expect("newer world delivery");
    remote.pump_at(30);
    let second = remote.take_pending_tick().snapshot.expect("newer delivery");
    assert!(
        second.damage_events.is_empty(),
        "a late batch duplicate cannot replay HUD, audio, scars or FX"
    );
}

#[test]
fn seeded_thirty_percent_loss_retries_combat_event_without_duplicate_presentation() {
    let hub = MemoryHub::with_loss(0xC0BA_7038, 30);
    let server_addr: SocketAddr = "10.27.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.27.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    let session_id = remote.session.session_id();
    let map_id = MapId::default();
    let hello = ProtocolMessage::ServerHello {
        session_id,
        protocol_version: net::PROTOCOL_VERSION,
        map_id,
        weather: Default::default(),
        map_content_hash: map_forge::battlefield_hash(&map_forge::battlefield(map_id)),
    };

    for step in 0..64_u64 {
        server.send(&mut server_port, &hello).expect("hello retry");
        remote.pump_at(step * 10);
        while let Some((_, datagram)) = server_port.recv().expect("server receive") {
            let _ = server.accept(&datagram);
        }
        if *remote.state() == net::session::SessionState::Connected {
            break;
        }
    }
    assert_eq!(*remote.state(), net::session::SessionState::Connected);

    let batch = ProtocolMessage::CombatEventBatch {
        session_id,
        events: vec![net::SequencedCombatEvent {
            delivery_seq: 0,
            event: net::CombatEvent::Damage(game_core::DamageEvent {
                source: TankId(7),
                target: TankId(8),
                damage_hp: 377,
                ..Default::default()
            }),
        }],
    };
    let mut attempts = 0_usize;
    let mut received_acks = 0_usize;
    let mut delivered_snapshots = 0_usize;
    let mut presentations = 0_usize;

    for attempt in 0..96_u64 {
        attempts += 1;
        server.send(&mut server_port, &batch).expect("combat event retry");
        server
            .send(
                &mut server_port,
                &ProtocolMessage::SnapshotDelivery(SnapshotDelivery {
                    session_id,
                    snapshot: Snapshot { server_tick: 100 + attempt, ..Default::default() },
                    ..Default::default()
                }),
            )
            .expect("world delivery");

        remote.pump_at(1_000 + attempt * 17);
        if let Some(snapshot) = remote.take_pending_tick().snapshot {
            delivered_snapshots += 1;
            presentations +=
                snapshot.damage_events.iter().filter(|event| event.damage_hp == 377).count();
        }
        while let Some((_, datagram)) = server_port.recv().expect("server receive") {
            if let Ok(Some(ProtocolMessage::CombatEventAck { last_received_seq: 0, .. })) =
                server.accept(&datagram)
            {
                received_acks += 1;
            }
        }
        if received_acks >= 4 && delivered_snapshots >= 4 {
            break;
        }
    }

    assert!(
        received_acks >= 4,
        "multiple retransmits and their ACKs must survive the seeded lossy route"
    );
    assert!(
        attempts > received_acks,
        "the deterministic 30% route must actually lose a combat batch or its ACK"
    );
    assert!(delivered_snapshots >= 4, "world delivery must also survive the same lossy route");
    assert_eq!(presentations, 1, "retries and repeated ACKs cannot replay HUD, audio, scars or FX");
    assert_eq!(remote.combat_events.last_received_seq(), Some(0));
    assert_eq!(remote.terminal_reason, None);
}

#[test]
fn combat_event_sequence_gap_is_terminal_without_partial_presentation() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.26.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.26.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    let batch = ProtocolMessage::CombatEventBatch {
        session_id: remote.session.session_id(),
        events: vec![net::SequencedCombatEvent {
            delivery_seq: 1,
            event: net::CombatEvent::Damage(Default::default()),
        }],
    };
    server.send(&mut server_port, &batch).expect("gapped batch");

    remote.pump_at(10);

    assert_eq!(remote.terminal_reason, Some(RemoteTerminalReason::CombatEventGap));
    assert!(remote.pending_combat_events.is_empty());
    assert_eq!(remote.combat_events.last_received_seq(), None);
}

/// v45: the battle clock is replicated. `StartBattle` carries the deadline tick; the client
/// counts down locally against the server tick it already tracks, so a remote HUD shows the
/// timer instead of hiding it. An untimed battle (`None`) still shows nothing.
#[test]
fn the_remote_battle_clock_counts_down_from_the_replicated_deadline() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.28.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.28.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    let session_id = remote.session.session_id();

    // A ten-minute battle at 60 Hz, already 60 s in: 540 s should remain.
    let limit = 600 * sim::DEFAULT_SERVER_TICK_HZ as u64;
    server
        .send(
            &mut server_port,
            &ProtocolMessage::StartBattle {
                session_id,
                assigned_tank: TankId(3),
                server_tick: 60 * sim::DEFAULT_SERVER_TICK_HZ as u64,
                time_limit_tick: Some(limit),
            },
        )
        .expect("start");
    remote.pump_at(10);

    let remaining = remote.battle_time_remaining_s().expect("a timed battle reports its clock");
    assert!(
        (remaining - 540.0).abs() < 0.5,
        "≈540 s remain 60 s into a 600 s battle, got {remaining}"
    );

    // Wrap it in the session kind the app actually reads through.
    let session = BattleSessionKind::Remote(Box::new(remote));
    assert!(session.battle_time_remaining_s().is_some(), "the HUD path sees the clock");
}

/// A `ServerHello` whose map hash disagrees ends the session with a message — it must NEVER
/// panic. A public server's hello reaches an untrusted client, so a wrong (or hostile) hash
/// cannot be a remote crash: the session goes terminal (`MapMismatch`) and the app keeps running.
#[test]
fn a_mismatched_map_hash_ends_the_session_instead_of_panicking() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.27.0.1:40000".parse().expect("server addr");
    let client_addr: SocketAddr = "10.27.0.2:5000".parse().expect("client addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port(client_addr);
    let mut server = Endpoint::new(client_addr);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));

    let map_id = MapId::default();
    let real_hash = map_forge::battlefield_hash(&map_forge::battlefield(map_id));
    let hello = ProtocolMessage::ServerHello {
        session_id: remote.session.session_id(),
        protocol_version: net::PROTOCOL_VERSION,
        map_id,
        weather: Default::default(),
        // A deliberately wrong hash: a stale build or a hostile server.
        map_content_hash: real_hash ^ 0xDEAD_BEEF,
    };
    server.send(&mut server_port, &hello).expect("mismatched hello");

    // The pump must return normally, not unwind.
    remote.pump_at(10);

    assert_eq!(remote.terminal_reason, Some(RemoteTerminalReason::MapMismatch));
}

/// A shove the player did not ask for has to reach the predictor as MOTION.
///
/// The predictor simulates the local hull with the same code the server runs and reconciles
/// against snapshots. That is exact for velocity the hull gave ITSELF — the predictor computed the
/// same number from the same command. It breaks the moment something EXTERNAL can set a velocity,
/// which is what hull contact carrying momentum introduced: the server shoves the hull, the
/// predictor knows nothing, and the next snapshot lands as a position the predictor disagrees
/// with. That reads as a rubber-band, not as being pushed.
///
/// `SnapshotDelivery::local_motion` already carried this for the networked path.
/// LOCAL play passed `reconciliation: None` and dropped it on the floor, which is the regression
/// locked here: a local session must hand up the authoritative motion of a moving hull.
#[test]
fn adopting_a_map_rebuilds_the_ground_rule() {
    // Before the fix this locks, `ClientApp.ground` was built once and never refreshed: after
    // a map swap the predictor kept gripping by the PREVIOUS map's roads/water/drainage. The
    // classifier must follow the battlefield it stands on, always.
    let mut app = crate::app::ClientApp::new_seeded(7);
    let previous = app.session.map_id();
    let next = if previous == MapId::ProkhorovkaHill252_2 {
        MapId::BystraValley
    } else {
        MapId::ProkhorovkaHill252_2
    };
    app.session = BattleSessionKind::Local(Box::new(LocalAuthoritativeServer::new_random_7v7(
        ServerTickConfig::default(),
        RandomBattleConfig {
            seed: BattleSeed::fixed(7),
            player_vehicle: game_core::VehicleKind::T54_1951,
            map: next,
        },
    )));
    app.adopt_session_map();
    assert_eq!(
        app.ground,
        terrain::GroundClassifier::new(&app.battlefield),
        "the predictor's ground rule must be rebuilt for the adopted map"
    );
}

#[test]
fn local_play_hands_the_predictor_the_authoritative_motion() {
    let server =
        LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), contract_battle());
    let player = server.player_tank();
    let mut session = BattleSessionKind::Local(Box::new(server));

    let mut moving_ticks = 0;
    for tick in 0..240 {
        let input = ClientInputCommand {
            client_tick: tick,
            tank_id: player,
            command: sim::TankCommand::drive(1.0, 0.0),
        };
        let result = session.tick_with_player_input(input);
        let Some(reconciliation) = result.reconciliation else {
            panic!("local play must reconcile every tick, or the motion never arrives");
        };
        let speed = glam::Vec3::from_array(reconciliation.motion.velocity_mps).length();
        if speed > 1.0 {
            moving_ticks += 1;
        }
    }

    assert!(
        moving_ticks > 60,
        "a hull under full throttle must report real authoritative motion, saw {moving_ticks} ticks"
    );
}

/// Netcode block 2, the honest-failure rule: an app built around a remote session that
/// never seated (dead wire, refused lobby, silence) says CONNECTION LOST from the first
/// frame — never a silent bot battle wearing multiplayer's name. The terminal reason on
/// the session is the whole mechanism; without it the outcome may not appear.
#[test]
fn an_unseated_remote_start_says_connection_lost_not_bots() {
    let hub = MemoryHub::new();
    // A port with NO server behind it: the connect can only die.
    let dead_addr: std::net::SocketAddr = "10.0.0.250:40999".parse().expect("addr");
    let client_port = hub.port("10.0.0.9:5999".parse().expect("addr"));
    let remote = RemoteSession::connect(dead_addr, Box::new(client_port));

    let mut app = crate::app::ClientApp::new_seeded(7);
    app.confirm_garage_selection(); // leave the garage - the tick loop is gated on it
    let mut session = BattleSessionKind::Remote(Box::new(remote));
    // FALSIFICATION ARM (kept commented): skip the abandon and the outcome stays None -
    // the terminal reason is the entire mechanism, proven red before this landed.
    session.abandon_remote(crate::app::session::RemoteTerminalReason::TimedOut);
    app.session = session;

    app.run_fixed_ticks(1);
    assert_eq!(
        app.battle_outcome,
        Some(crate::hud::BattleHudOutcome::ConnectionLost),
        "a failed remote start must be LOUD - the screen says the truth from frame one"
    );
}

/// N4, the reconnect: a NETWORK death re-dials on the same local port, the host re-keys
/// the seat for the known address, and the crew is back in ITS OWN tank — terminal
/// cleared, snapshots flowing — without the player doing anything. The screen said
/// CONNECTION LOST for exactly as long as the seat was actually gone.
#[test]
fn a_network_death_redials_back_into_the_same_tank() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.30.0.1:40000".parse().expect("server addr");
    let mut server_port = hub.port(server_addr);
    let client_port = hub.port("10.30.0.2:5000".parse().expect("client addr"));
    let mut host = RemoteBattleServer::new(ServerTickConfig::new(60, 1), contract_battle(), 100, 0);
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));

    let mut seated_tank = None;
    let mut cut_at_ms = None;
    let mut resumed_snapshots = 0_usize;
    for step in 0..4_000_u64 {
        let now_ms = step * 17;
        let outcome =
            remote.tick_with_player_input_at(contract_input(sim::TankCommand::idle()), now_ms);
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);

        if seated_tank.is_none() && remote.is_seated() {
            seated_tank = remote.assigned_tank;
        }
        // Once seated and flowing, cut the line ONCE: a transport death mid-battle.
        if cut_at_ms.is_none() && seated_tank.is_some() && step > 400 {
            remote.enter_terminal(RemoteTerminalReason::Transport);
            assert!(remote.is_terminal(), "the cut is real (test premise)");
            cut_at_ms = Some(now_ms);
        }
        if let Some(cut) = cut_at_ms
            && now_ms > cut
            && !remote.is_terminal()
            && outcome.snapshot.is_some()
        {
            resumed_snapshots += 1;
        }
    }

    let tank = seated_tank.expect("the crew was seated before the cut");
    assert!(cut_at_ms.is_some(), "the line was cut (test premise)");
    assert!(!remote.is_terminal(), "the re-dial must clear the terminal by winning a seat");
    assert_eq!(remote.assigned_tank, Some(tank), "the host re-keys the SAME tank by address");
    assert!(
        resumed_snapshots > 10,
        "the world must flow again after the reconnect, saw {resumed_snapshots}"
    );
}

/// The other half of the N4 rule: an ANSWER is not an outage. A refusal never re-dials —
/// the terminal stands, and the budget is never spent knocking on a door that said no.
#[test]
fn a_refused_session_never_redials() {
    let hub = MemoryHub::new();
    let server_addr: SocketAddr = "10.31.0.1:40000".parse().expect("server addr");
    let client_port = hub.port("10.31.0.2:5000".parse().expect("client addr"));
    let mut remote = RemoteSession::connect(server_addr, Box::new(client_port));
    remote.enter_terminal(RemoteTerminalReason::Refused);
    let first_session_id = remote.session_id_for_tests();
    for step in 0..600_u64 {
        let _ =
            remote.tick_with_player_input_at(contract_input(sim::TankCommand::idle()), step * 17);
    }
    assert!(remote.is_terminal(), "a refusal is final");
    assert_eq!(
        remote.session_id_for_tests(),
        first_session_id,
        "no fresh handshake may be spent on an answer"
    );
}
