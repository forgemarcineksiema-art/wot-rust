use net::session::{ClientSession, Endpoint};
use net::transport::MemoryHub;
use net::{ClientInputCommand, ProtocolMessage};
use server::remote::RemoteBattleServer;
use server::{BattleSeed, RandomBattleConfig, ServerTickConfig};

const OLD_SESSION: u64 = 0x1111_2222_3333_4444;
const NEW_SESSION: u64 = 0xAAAA_BBBB_CCCC_DDDD;

fn battle() -> RandomBattleConfig {
    RandomBattleConfig {
        seed: BattleSeed::fixed(117),
        player_vehicle: game_core::VehicleKind::T54_1951,
        map: terrain::MapId::default(),
    }
}

fn input_batch(session_id: u64, sequence: u64, tank_id: game_core::TankId) -> ProtocolMessage {
    ProtocolMessage::InputBatch {
        session_id,
        commands: vec![ClientInputCommand {
            client_tick: sequence,
            tank_id,
            command: sim::TankCommand::drive(1.0, 0.0),
        }],
    }
}

#[test]
fn same_address_reconnect_resets_sequence_and_rejects_the_retired_session() {
    let hub = MemoryHub::new();
    let server_addr = "10.30.0.1:40000".parse().expect("server address");
    let client_addr = "10.30.0.2:5000".parse().expect("client address");
    let mut server_port = hub.port(server_addr);
    let mut client_port = hub.port(client_addr);
    let mut host = RemoteBattleServer::new(ServerTickConfig::default(), battle(), 100, 0);
    let mut first = ClientSession::connect_with_session_id(server_addr, 0, OLD_SESSION);
    let mut first_tank = None;
    let mut first_sequence = 0_u64;

    for step in 0..120_u64 {
        let now_ms = step * 17;
        for message in first.tick(now_ms, &mut client_port).expect("first session tick") {
            if let ProtocolMessage::StartBattle { assigned_tank, .. } = message {
                first_tank = Some(assigned_tank);
            }
        }
        if let Some(tank) = first_tank {
            let batch = input_batch(OLD_SESSION, first_sequence, tank);
            first.endpoint.send(&mut client_port, &batch).expect("first input");
            first_sequence += 1;
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }
    let first_tank = first_tank.expect("first session seated");
    drop(first);
    drop(client_port);

    // Reuse the same UDP four-tuple before the old seat times out. Queued old server traffic may
    // still exist; the new ClientSession must reject it by session id.
    let mut client_port = hub.port(client_addr);
    let mut second = ClientSession::connect_with_session_id(server_addr, 120 * 17, NEW_SESSION);
    let mut retired_sender = Endpoint::new(server_addr);
    let mut second_tank = None;
    let mut second_sequence = 0_u64;
    let mut newest_ack = None;
    let mut injected_retired_session = false;
    let mut resent_current_hello = false;
    let mut start_position = None;
    let mut end_position = None;
    let slot = game_core::VehicleKind::T54_1951.spec().ammo.initial_selected as usize;
    let initial_rounds = game_core::VehicleKind::T54_1951.spec().ammo.counts[slot];
    let mut minimum_rounds = initial_rounds;

    for step in 120..420_u64 {
        let now_ms = step * 17;
        for message in second.tick(now_ms, &mut client_port).expect("second session tick") {
            match message {
                ProtocolMessage::StartBattle { assigned_tank, .. } => {
                    second_tank = Some(assigned_tank);
                }
                ProtocolMessage::InputAck { last_processed_input_seq, .. } => {
                    newest_ack = Some(newest_ack.map_or(last_processed_input_seq, |old: u64| {
                        old.max(last_processed_input_seq)
                    }));
                }
                ProtocolMessage::SnapshotDelivery(delivery) => {
                    if let Some(tank) = second_tank
                        && let Some(own) = delivery
                            .snapshot
                            .tanks
                            .iter()
                            .find(|candidate| candidate.tank_id == tank)
                    {
                        start_position.get_or_insert(own.position);
                        end_position = Some(own.position);
                        minimum_rounds = minimum_rounds.min(own.ammo_counts[slot]);
                    }
                }
                _ => {}
            }
        }
        if let Some(tank) = second_tank {
            if !injected_retired_session {
                let old_hello = ProtocolMessage::ClientHello {
                    session_id: OLD_SESSION,
                    protocol_version: net::PROTOCOL_VERSION,
                };
                retired_sender.send(&mut client_port, &old_hello).expect("delayed old hello");
                let old_fire = ProtocolMessage::InputBatch {
                    session_id: OLD_SESSION,
                    commands: vec![ClientInputCommand {
                        client_tick: 0,
                        tank_id: tank,
                        command: sim::TankCommand { fire: true, ..sim::TankCommand::idle() },
                    }],
                };
                retired_sender.send(&mut client_port, &old_fire).expect("delayed old fire");
                injected_retired_session = true;
            }
            if second_sequence == 10 && !resent_current_hello {
                let repeated = ProtocolMessage::ClientHello {
                    session_id: NEW_SESSION,
                    protocol_version: net::PROTOCOL_VERSION,
                };
                second.endpoint.send(&mut client_port, &repeated).expect("repeated current hello");
                resent_current_hello = true;
            }
            let batch = input_batch(NEW_SESSION, second_sequence, tank);
            second.endpoint.send(&mut client_port, &batch).expect("second input");
            second_sequence += 1;
        }
        host.pump(now_ms, &mut server_port);
        host.tick(now_ms, &mut server_port);
    }

    assert_eq!(second_tank, Some(first_tank), "a fast reconnect keeps its assigned hull");
    assert!(
        newest_ack.is_some_and(|ack| ack >= 100),
        "sequence zero and the continuing new epoch must advance, got {newest_ack:?}"
    );
    assert_eq!(
        minimum_rounds, initial_rounds,
        "the retired epoch's delayed fire must never reach the authoritative queue"
    );
    let start = start_position.expect("new session receives its hull");
    let end = end_position.expect("new session keeps receiving its hull");
    let travel = ((end[0] - start[0]).powi(2) + (end[2] - start[2]).powi(2)).sqrt();
    assert!(travel > 10.0, "the reset sequence must actually drive the tank, moved {travel} m");
}
