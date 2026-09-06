use battle_host::{
    BattleMode, BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig,
};
use game_core::{BattleFormat, VehicleKind};

#[test]
fn the_7v7_clock_is_seven_minutes_and_the_15v15_clock_fifteen() {
    for (format, seconds) in
        [(BattleFormat::SevenVsSeven, 420), (BattleFormat::FifteenVsFifteen, 900)]
    {
        for hz in [30, 60, 120] {
            let server = LocalAuthoritativeServer::new_random(
                ServerTickConfig::new(hz, 10),
                RandomBattleConfig::new(BattleSeed::fixed(42), VehicleKind::TigerII)
                    .with_format(format),
            );
            assert_eq!(server.time_limit_tick(), Some(u64::from(seconds * hz)));
            assert_eq!(server.battle_time_remaining_s(), Some(seconds as f32));
            assert_eq!(server.battle_format(), Some(format));
        }
    }
}

#[test]
fn the_7v7_format_is_todays_battle_byte_for_byte() {
    for &map in terrain::MapId::SHIPPED {
        let server = LocalAuthoritativeServer::new_random_7v7(
            ServerTickConfig::new(60, 20),
            RandomBattleConfig::new(BattleSeed::fixed(42), VehicleKind::TigerII).on_map(map),
        );
        let bytes =
            net::encode_message(&net::ProtocolMessage::Snapshot(server.latest_snapshot().clone()))
                .expect("snapshot");
        let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
        // Recorded from master 12bf9f48 before adding formats, seed 42, Tiger II.
        let expected = match map {
            terrain::MapId::ProkhorovkaHill252_2 => 0x542a8116342b8f03,
            terrain::MapId::BystraValley => 0xdb41e8284b556181,
            terrain::MapId::OrlinyPereval => 0x55a76543d993f0bc,
            terrain::MapId::Ostrogorsk => 0x54fc565dadf8db5b,
            terrain::MapId::MazurskiPrzesmyk => 0xd8823ba214633e65,
            _ => panic!("record a baseline for a newly shipped map"),
        };
        assert_eq!(hash, expected, "{map:?}: existing 7v7 deployment moved");
    }
}

#[test]
fn a_15v15_setup_spawns_thirty_tanks_with_the_player_on_team_one() {
    let server = LocalAuthoritativeServer::new_ai_battle(
        ServerTickConfig::default(),
        RandomBattleConfig::new(BattleSeed::fixed(42), VehicleKind::TigerII),
    );
    assert_eq!(server.battle_mode(), BattleMode::AiBattle);
    assert_eq!(server.battle_format(), Some(BattleFormat::FifteenVsFifteen));
    let roster = server.roster();
    assert_eq!(roster.len(), 30);
    let humans: Vec<_> =
        roster.iter().filter(|seat| seat.crew_kind == net::CrewKind::Human).collect();
    assert_eq!(humans.len(), 1);
    assert_eq!(humans[0].tank_id, server.player_tank());
    assert_eq!(humans[0].team, game_core::TeamId(1));
    assert_eq!(humans[0].vehicle, VehicleKind::TigerII);
    for team in [game_core::TeamId(1), game_core::TeamId(2)] {
        let seats: Vec<_> = roster.iter().filter(|seat| seat.team == team).collect();
        assert_eq!(seats.len(), 15);
        assert_eq!(seats.last().expect("last seat").seat_letter(), 'O');
    }
    assert!(roster.iter().all(|seat| VehicleKind::TigerII.in_matchmaking_bracket(seat.vehicle)));
}

/// M6 (`docs/game-modes.md` R4): the crews are dealt across BOTH teams in a snake, so the sides
/// differ by at most one human, every crew keeps its pick, and one crew is still the desktop
/// battle on team one.
#[test]
fn humans_are_dealt_across_both_teams_in_a_snake() {
    use game_core::{TeamId, VehicleKind};
    let wishes = [
        Some(VehicleKind::IS3),
        Some(VehicleKind::TigerI),
        None,
        Some(VehicleKind::Centurion),
        Some(VehicleKind::T34_85),
    ];
    for format in BattleFormat::ALL {
        let (server, human_tanks) = LocalAuthoritativeServer::new_random_for_humans(
            ServerTickConfig::default(),
            RandomBattleConfig::new(BattleSeed::fixed(42), VehicleKind::TigerII)
                .with_format(format),
            &wishes,
        );
        let roster = server.roster();
        let mut per_team = [0_usize; 2];
        for (crew, tank) in human_tanks.iter().enumerate() {
            let entry = roster.iter().find(|e| e.tank_id == *tank).expect("every crew is seated");
            assert_eq!(entry.crew_kind, net::CrewKind::Human);
            let team = usize::from(entry.team.0 - 1);
            assert_eq!(
                team,
                battle_host::human_team(crew),
                "crew {crew} sits where the snake says"
            );
            per_team[team] += 1;
            let wish = wishes[crew].unwrap_or(VehicleKind::BENCHMARK);
            assert_eq!(entry.vehicle, wish, "crew {crew} drives its garage pick");
        }
        assert_eq!(per_team, [3, 2], "{format:?}: five crews split three and two");
        assert_eq!(
            roster.iter().filter(|e| e.crew_kind == net::CrewKind::Human).count(),
            wishes.len(),
            "no other hull is human"
        );
        assert_eq!(roster.len(), format.total_seats(), "every other seat is a bot");
        assert_eq!(server.player_tank(), human_tanks[0]);
    }
    // One crew is the desktop battle: team one, seat A.
    let (server, human_tanks) = LocalAuthoritativeServer::new_random_for_humans(
        ServerTickConfig::default(),
        RandomBattleConfig::new(BattleSeed::fixed(42), VehicleKind::TigerII),
        &[None],
    );
    let roster = server.roster();
    let seat = roster.iter().find(|e| e.tank_id == human_tanks[0]).expect("seated");
    assert_eq!((seat.team, seat.seat, seat.vehicle), (TeamId(1), 0, VehicleKind::TigerII));
}

#[test]
fn every_seat_of_every_format_lands_inside_its_zone() {
    for &map_id in terrain::MapId::SHIPPED {
        let map = map_forge::battlefield(map_id);
        for format in BattleFormat::ALL {
            for seed in [0, 1, 42, u64::MAX] {
                let server = LocalAuthoritativeServer::new_random(
                    ServerTickConfig::default(),
                    RandomBattleConfig::new(BattleSeed::fixed(seed), VehicleKind::TigerII)
                        .on_map(map_id)
                        .with_format(format),
                );
                let tanks = &server.latest_snapshot().tanks;
                assert_eq!(tanks.len(), format.total_seats());
                for (index, tank) in tanks.iter().enumerate() {
                    let zone = map
                        .spawn_zones
                        .iter()
                        .find(|zone| zone.team == tank.team.0)
                        .expect("team zone");
                    let [x, y, z] = tank.position;
                    assert!(
                        (x - zone.center[0]).hypot(z - zone.center[2]) <= zone.radius_m,
                        "{map_id:?}/{format:?}/{seed}: {:?} outside zone",
                        tank.tank_id
                    );
                    assert_eq!(map.heightmap.sample_height(x, z), Some(y));
                    assert!(
                        map.water_field().depth_at(y, x, z) <= 0.0,
                        "{map_id:?}/{format:?}/{seed}: {:?} spawned in water",
                        tank.tank_id
                    );
                    for other in &tanks[index + 1..] {
                        assert!(
                            (x - other.position[0]).hypot(z - other.position[2]) > 10.0,
                            "{map_id:?}/{format:?}/{seed}: overlapping deployment"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn the_ai_battle_advances_thirty_hulls_without_a_socket_or_spawn_damage() {
    for &map in terrain::MapId::SHIPPED {
        let mut server = LocalAuthoritativeServer::new_ai_battle(
            ServerTickConfig::default(),
            RandomBattleConfig::new(BattleSeed::fixed(42), VehicleKind::TigerII).on_map(map),
        );
        let before = server.latest_snapshot().clone();
        for client_tick in 0..60 {
            let tick = server.tick_with_input(net::ClientInputCommand {
                client_tick,
                tank_id: server.player_tank(),
                command: sim::TankCommand::idle(),
            });
            assert!(tick.damage_events.is_empty(), "{map:?}: spawn caused damage");
        }
        let after = server.latest_snapshot();
        assert_eq!(after.tanks.len(), 30);
        let moved = before
            .tanks
            .iter()
            .zip(&after.tanks)
            .filter(|(a, b)| {
                a.tank_id != server.player_tank()
                    && (a.position[0] - b.position[0]).hypot(a.position[2] - b.position[2]) > 0.1
            })
            .count();
        assert!(moved >= 20, "{map:?}: only {moved} bots moved from deployment");
        assert_eq!(server.battle_time_remaining_s(), Some(899.0));
        assert_eq!(server.battle_outcome(), None);
    }
}
