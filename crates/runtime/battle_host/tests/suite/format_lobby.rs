use battle_host::remote::RemoteBattleServer;
use battle_host::{BattleFormat, BattleSeed, RandomBattleConfig, ServerTickConfig};
use net::ProtocolMessage;
use net::session::ClientSession;
use net::transport::MemoryHub;

#[test]
fn the_lobby_waits_for_its_format_and_sends_its_clock_to_every_crew() {
    for format in BattleFormat::ALL {
        let hub = MemoryHub::new();
        let address = "10.0.0.1:40000".parse().expect("host address");
        let mut host_port = hub.port(address);
        let config =
            RandomBattleConfig::new(BattleSeed::fixed(42), game_core::VehicleKind::TigerII)
                .with_format(format);
        let mut host = RemoteBattleServer::new(ServerTickConfig::default(), config, 60_000, 0);
        let seats = format.total_seats();
        let mut clients: Vec<_> = (0..seats)
            .map(|seat| {
                let port = hub.port(format!("10.0.0.{}:5000", seat + 2).parse().expect("client"));
                (ClientSession::connect(address, 0), port)
            })
            .collect();
        let mut saw_lobby = false;
        let mut started = vec![false; seats];
        for step in 0..200_u64 {
            let now = step * 16;
            // Keep the last seat empty long enough to prove twenty-nine (or thirteen) cannot
            // fill the selected lobby: since M6 "full" is BOTH teams' seats, so 15v15 never
            // starts at fifteen crews and 7v7 never at seven.
            let active = if step < 100 { seats - 1 } else { seats };
            for (index, (client, port)) in clients.iter_mut().take(active).enumerate() {
                for message in client.tick(now, port).expect("client tick") {
                    match message {
                        ProtocolMessage::LobbyState { needed, .. } => {
                            saw_lobby = true;
                            assert_eq!(usize::from(needed), seats);
                        }
                        ProtocolMessage::StartBattle { time_limit_tick, .. } => {
                            assert!(step >= 100, "{format:?} started with an empty seat");
                            assert_eq!(
                                time_limit_tick,
                                Some(u64::from(format.time_limit_s()) * 60)
                            );
                            started[index] = true;
                        }
                        _ => {}
                    }
                }
            }
            host.pump(now, &mut host_port);
            host.tick(now, &mut host_port);
            if step < 100 {
                assert!(!host.is_running(), "{format:?} started short of its format");
            }
            if started.iter().all(|started| *started) {
                break;
            }
        }
        assert!(saw_lobby);
        assert!(host.is_running());
        assert!(started.iter().all(|started| *started), "every crew receives the battle clock");
    }
}
