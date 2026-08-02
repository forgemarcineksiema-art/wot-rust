use battle_host::{
    BattleMode, BattleOutcome, BattleSeed, DrawReason, LocalAuthoritativeServer,
    RandomBattleConfig, ServerTickConfig,
};
use net::ClientInputCommand;
use sim::TankCommand;

#[test]
fn random_7v7_spawns_fourteen_tanks_with_player_on_team_one() {
    let server = LocalAuthoritativeServer::new_random_7v7(
        ServerTickConfig::new(60, 20),
        RandomBattleConfig::new(BattleSeed::fixed(42), game_core::VehicleKind::TigerII),
    );

    let snapshot = server.latest_snapshot();
    let team_one = snapshot.tanks.iter().filter(|tank| tank.team == game_core::TeamId(1)).count();
    let team_two = snapshot.tanks.iter().filter(|tank| tank.team == game_core::TeamId(2)).count();
    let player = snapshot
        .tanks
        .iter()
        .find(|tank| tank.tank_id == server.player_tank())
        .expect("player tank");

    assert_eq!(server.battle_mode(), BattleMode::Random7v7);
    assert_eq!(snapshot.tanks.len(), 14);
    assert_eq!(team_one, 7);
    assert_eq!(team_two, 7);
    assert_eq!(player.team, game_core::TeamId(1));
    assert_eq!(player.vehicle, game_core::VehicleKind::TigerII);
}

/// A random battle is one technology bracket: every tank on the field shares the player's era.
/// This is the era system doing the matchmaker's job — no Tiger ever meets a T-54 (locked for
/// both directions and across seeds, so it cannot pass by luck of the draw).
#[test]
fn random_7v7_fields_a_single_era_battle() {
    for seed in [1u64, 2, 3, 4, 5] {
        for player_vehicle in [game_core::VehicleKind::TigerI, game_core::VehicleKind::T54_1951] {
            let server = LocalAuthoritativeServer::new_random_7v7(
                ServerTickConfig::new(60, 20),
                RandomBattleConfig::new(BattleSeed::fixed(seed), player_vehicle),
            );
            let snapshot = server.latest_snapshot();
            for tank in &snapshot.tanks {
                assert_eq!(
                    tank.vehicle.era(),
                    player_vehicle.era(),
                    "seed {seed}: {:?} broke into a {:?} battle",
                    tank.vehicle,
                    player_vehicle.era()
                );
            }
        }
    }
}

/// The era pool must not collapse into clone armies: across seeds, the enemy team of an Era II
/// battle draws more than one design (the pool has four).
#[test]
fn random_7v7_era_battles_keep_roster_variety() {
    let mut kinds = std::collections::HashSet::new();
    for seed in [11u64, 12, 13, 14] {
        let server = LocalAuthoritativeServer::new_random_7v7(
            ServerTickConfig::new(60, 20),
            RandomBattleConfig::new(BattleSeed::fixed(seed), game_core::VehicleKind::TigerII),
        );
        for tank in &server.latest_snapshot().tanks {
            kinds.insert(tank.vehicle);
        }
    }
    assert!(kinds.len() >= 3, "four seeds of Era II must field variety, got {kinds:?}");
}

#[test]
fn random_7v7_uses_map_spawn_zones_and_facing_yaw() {
    let server = LocalAuthoritativeServer::new_random_7v7(
        ServerTickConfig::new(60, 20),
        RandomBattleConfig::new(BattleSeed::fixed(7), game_core::VehicleKind::T54_1951),
    );
    // The zones must come from the map the server actually spawned on (MapId::default()),
    // not a hardcoded one — this assertion went red the day the default flipped to Bystra.
    let map = map_forge::battlefield(server.map_id());
    let snapshot = server.latest_snapshot();

    for tank in &snapshot.tanks {
        let zone =
            map.spawn_zones.iter().find(|zone| zone.team == tank.team.0).expect("team spawn zone");
        let dx = tank.position[0] - zone.center[0];
        let dz = tank.position[2] - zone.center[2];
        let distance = (dx * dx + dz * dz).sqrt();

        assert!(distance <= zone.radius_m, "tank {:?} spawned outside zone", tank.tank_id);
        assert!(
            (tank.yaw_rad - zone.facing_yaw_rad).abs() < 1.0e-6,
            "tank {:?} did not face out of spawn",
            tank.tank_id
        );
    }
}

#[test]
fn random_7v7_bot_commands_move_or_aim_without_player_input() {
    let mut server = LocalAuthoritativeServer::new_random_7v7(
        ServerTickConfig::new(60, 20),
        RandomBattleConfig::new(BattleSeed::fixed(9), game_core::VehicleKind::T54_1951),
    );
    let player_tank = server.player_tank();
    let bot_tank = server
        .latest_snapshot()
        .tanks
        .iter()
        .find(|tank| tank.team == game_core::TeamId(1) && tank.tank_id != player_tank)
        .map(|tank| tank.tank_id)
        .expect("allied bot");
    let before = server
        .latest_snapshot()
        .tanks
        .iter()
        .find(|tank| tank.tank_id == bot_tank)
        .cloned()
        .expect("bot before");

    for client_tick in 0..45 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
    }

    let after = server
        .latest_snapshot()
        .tanks
        .iter()
        .find(|tank| tank.tank_id == bot_tank)
        .cloned()
        .expect("bot after");
    let dx = after.position[0] - before.position[0];
    let dz = after.position[2] - before.position[2];
    let moved = (dx * dx + dz * dz).sqrt() > 0.05;
    let aimed = (after.turret_yaw_rad - before.turret_yaw_rad).abs() > 1.0e-4;

    assert!(moved || aimed, "bot should issue deterministic commands during server ticks");
}

#[test]
fn battle_outcome_reports_team_wipe_winner() {
    let outcome = BattleOutcome::from_team_alive_counts([
        (game_core::TeamId(1), 0),
        (game_core::TeamId(2), 3),
    ]);

    assert_eq!(outcome, Some(BattleOutcome::TeamEliminated { winning_team: game_core::TeamId(2) }));
}

/// The last tanks of both teams dying on the same tick used to leave the battle running forever —
/// no team "alive count == 1" ever happened again. A mutual wipe is a draw, and a draw has no
/// winning team.
#[test]
fn battle_outcome_reports_mutual_wipe_as_a_draw() {
    let outcome = BattleOutcome::from_team_alive_counts([
        (game_core::TeamId(1), 0),
        (game_core::TeamId(2), 0),
    ]);

    assert_eq!(outcome, Some(BattleOutcome::Draw { reason: DrawReason::MutualElimination }));
    assert_eq!(outcome.unwrap().winning_team(), None);
}

/// The clock is a safety net, NOT a verdict of "nobody won".
///
/// A measured 7v7 ended `Draw { TimeExpired }` with one team holding four hulls against the
/// other's one — a fourfold advantage called even, punishing the side that won the fight. The
/// timer now awards the battle to whoever is ahead on the board.
#[test]
fn the_clock_awards_the_battle_to_whoever_is_ahead_on_hulls() {
    let ahead =
        BattleOutcome::from_time_expiry([(game_core::TeamId(1), 4), (game_core::TeamId(2), 1)]);
    assert_eq!(ahead, BattleOutcome::TeamAhead { winning_team: game_core::TeamId(1) });
    assert_eq!(
        ahead.winning_team(),
        Some(game_core::TeamId(1)),
        "a win on the clock is a win, and reports its winner like any other"
    );

    // One hull of advantage is still an advantage — there is no "close enough to call it even".
    assert_eq!(
        BattleOutcome::from_time_expiry([(game_core::TeamId(1), 2), (game_core::TeamId(2), 3)]),
        BattleOutcome::TeamAhead { winning_team: game_core::TeamId(2) }
    );
}

/// Only a genuine tie is a draw — and an empty board is one too, not a win for nobody in
/// particular.
#[test]
fn the_clock_draws_only_on_a_genuine_tie() {
    assert_eq!(
        BattleOutcome::from_time_expiry([(game_core::TeamId(1), 3), (game_core::TeamId(2), 3)]),
        BattleOutcome::Draw { reason: DrawReason::TimeExpired }
    );
    assert_eq!(
        BattleOutcome::from_time_expiry([(game_core::TeamId(1), 0), (game_core::TeamId(2), 0)]),
        BattleOutcome::Draw { reason: DrawReason::TimeExpired },
        "a board with nothing left on it is a draw, not a win"
    );
    assert_eq!(
        BattleOutcome::from_time_expiry([(game_core::TeamId(1), 3), (game_core::TeamId(2), 3)])
            .winning_team(),
        None
    );
}

/// End to end through the real server: kill enough of one team, run the clock out, and the
/// survivors are told they won instead of being handed a draw.
#[test]
fn a_battle_that_runs_out_of_time_ahead_on_hulls_is_won_not_drawn() {
    let mut server = LocalAuthoritativeServer::new_random_7v7(
        ServerTickConfig::new(60, 20),
        RandomBattleConfig::new(BattleSeed::fixed(11), game_core::VehicleKind::T54_1951),
    );
    let player_tank = server.player_tank();
    let player_team = server
        .latest_snapshot()
        .tanks
        .iter()
        .find(|tank| tank.tank_id == player_tank)
        .expect("player")
        .team;

    // Wipe most of the OTHER team, leaving one alive so the elimination rule cannot fire and the
    // clock is what ends the battle.
    let doomed: Vec<_> = server
        .latest_snapshot()
        .tanks
        .iter()
        .filter(|tank| tank.team != player_team)
        .skip(1)
        .map(|tank| tank.tank_id)
        .collect();
    assert!(doomed.len() >= 2, "the other team should have hulls to lose");
    for id in doomed {
        server.knock_out_for_test(id);
    }

    server.override_battle_time_limit_ticks(Some(4));
    for client_tick in 0..4 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
    }

    assert_eq!(
        server.battle_outcome(),
        Some(BattleOutcome::TeamAhead { winning_team: player_team }),
        "the side still holding the board wins on the clock"
    );
}

/// The battle clock is the safety net that guarantees a random battle always ends: when it runs
/// out with both teams alive, the battle closes as a draw and stays closed.
#[test]
fn random_battle_clock_expiry_ends_the_battle_in_a_draw() {
    let mut server = LocalAuthoritativeServer::new_random_7v7(
        ServerTickConfig::new(60, 20),
        RandomBattleConfig::new(BattleSeed::fixed(11), game_core::VehicleKind::T54_1951),
    );
    let player_tank = server.player_tank();
    assert_eq!(
        server.battle_time_remaining_s(),
        Some(battle_host::RANDOM_BATTLE_TIME_LIMIT_S as f32),
        "the clock starts full"
    );

    // Driving 36 000 real ticks is not worth a test's time; shrink the clock instead.
    server.override_battle_time_limit_ticks(Some(4));
    for client_tick in 0..4 {
        assert_eq!(server.battle_outcome(), None, "battle runs while the clock has time");
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
    }

    assert_eq!(
        server.battle_outcome(),
        Some(BattleOutcome::Draw { reason: DrawReason::TimeExpired })
    );
    assert_eq!(server.battle_time_remaining_s(), Some(0.0), "the clock bottoms out at zero");
}

/// The practice duel is a sandbox: no clock, no timeout draw.
#[test]
fn practice_duel_runs_untimed() {
    let server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    assert_eq!(server.battle_time_remaining_s(), None);
}

#[test]
fn local_server_can_start_with_selected_player_vehicle() {
    let server = LocalAuthoritativeServer::new_with_player_vehicle(
        ServerTickConfig::new(60, 20),
        game_core::VehicleKind::TigerII,
    );
    let snapshot = server.latest_snapshot();
    let player = snapshot
        .tanks
        .iter()
        .find(|tank| tank.tank_id == server.player_tank())
        .expect("player tank");

    assert_eq!(player.vehicle, game_core::VehicleKind::TigerII);
    assert_eq!(player.hit_points, game_core::VehicleKind::TigerII.spec().hit_points);
}

#[test]
fn latest_player_snapshot_is_filtered_after_seeded_visibility() {
    let server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let snapshot = server.latest_snapshot_for_player();

    assert!(snapshot.tanks.iter().any(|tank| tank.tank_id == server.player_tank()));
    assert!(
        snapshot.tanks.iter().any(|tank| tank.tank_id == server.target_tank()),
        "the practice target starts in LOS and should survive player-view filtering"
    );
}

#[test]
fn local_server_runtime_vehicle_change_reassigns_player_tank_id() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let old_player = server.player_tank();

    let snapshot = server.change_player_vehicle(game_core::VehicleKind::Jagdtiger);
    let new_player = server.player_tank();

    assert_ne!(old_player, new_player);
    assert!(snapshot.tanks.iter().all(|tank| tank.tank_id != old_player));
    let player =
        snapshot.tanks.iter().find(|tank| tank.tank_id == new_player).expect("new player tank");
    assert_eq!(player.vehicle, game_core::VehicleKind::Jagdtiger);
    assert_eq!(player.reload_remaining_s, 0.0);
}

#[test]
fn filtered_vehicle_change_keeps_the_new_player_snapshot_visible() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));

    let snapshot =
        server.change_player_vehicle_with_spec_for_player(game_core::TankSpec::tiger_i_ausf_e());

    assert!(snapshot.tanks.iter().any(|tank| tank.tank_id == server.player_tank()));
    assert!(
        snapshot.tanks.iter().any(|tank| tank.tank_id == server.target_tank()),
        "post-garage visibility should be refreshed before filtering"
    );
}

#[test]
fn local_server_installs_a_custom_assembled_spec_not_the_stock_one() {
    // The garage builds a non-stock spec (e.g. a slower-reloading crew) and installs it here; the
    // respawned tank must fight with the custom stats, not the vehicle's default loadout.
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let mut spec = game_core::VehicleKind::TigerII.spec();
    spec.hit_points += 250;
    spec.gun.reload_seconds += 3.0;

    let snapshot = server.change_player_vehicle_with_spec(spec.clone());
    let player =
        snapshot.tanks.iter().find(|t| t.tank_id == server.player_tank()).expect("player tank");

    assert_eq!(player.vehicle, game_core::VehicleKind::TigerII);
    assert_eq!(player.hit_points, spec.hit_points);
    assert_ne!(player.hit_points, game_core::VehicleKind::TigerII.spec().hit_points);
}

#[test]
fn player_tick_emits_filtered_snapshots_with_own_feedback() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();

    server.tick_with_player_input(ClientInputCommand {
        client_tick: 0,
        tank_id: player_tank,
        command: TankCommand { fire: true, ..TankCommand::idle() },
    });

    let mut damage_snapshot = None;
    for client_tick in 1..=8 {
        let tick = server.tick_with_player_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
        if let Some(snapshot) = tick.snapshot
            && !snapshot.damage_events.is_empty()
        {
            damage_snapshot = Some(snapshot);
            break;
        }
    }

    let snapshot = damage_snapshot.expect("player shot feedback should survive filtering");
    assert!(snapshot.damage_events.iter().any(|event| event.source == player_tank));
}

#[test]
fn local_server_accepts_client_commands_and_emits_authoritative_snapshots() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();

    assert_eq!(server.authoritative_tick(), 0);
    assert_eq!(server.latest_snapshot().server_tick, 0);

    let first = server.tick_with_input(ClientInputCommand {
        client_tick: 0,
        tank_id: player_tank,
        command: TankCommand::drive(1.0, 0.0),
    });
    assert_eq!(first.server_tick, 1);
    assert!(first.snapshot.is_none());

    server.tick_with_input(ClientInputCommand {
        client_tick: 1,
        tank_id: player_tank,
        command: TankCommand::drive(1.0, 0.0),
    });
    let third = server.tick_with_input(ClientInputCommand {
        client_tick: 2,
        tank_id: player_tank,
        command: TankCommand::drive(1.0, 0.0),
    });

    let snapshot = third.snapshot.expect("20 Hz snapshots at 60 Hz server tick emit every 3 ticks");
    assert_eq!(third.server_tick, 3);
    assert_eq!(snapshot.server_tick, 3);
    assert_eq!(snapshot.tanks[0].tank_id, player_tank);
}

#[test]
fn local_server_replication_carries_damage_events_to_next_snapshot() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();
    let target_tank = server.target_tank();

    server.tick_with_input(ClientInputCommand {
        client_tick: 0,
        tank_id: player_tank,
        command: TankCommand { fire: true, ..TankCommand::idle() },
    });

    let mut damage_snapshot = None;
    for client_tick in 1..=8 {
        let tick = server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
        if let Some(snapshot) = tick.snapshot
            && !snapshot.damage_events.is_empty()
        {
            damage_snapshot = Some(snapshot);
            break;
        }
    }

    let snapshot = damage_snapshot.expect("damage event should be replicated on a snapshot");
    let event = &snapshot.damage_events[0];
    assert_eq!(event.source, player_tank);
    assert_eq!(event.target, target_tank);
    assert!(event.penetrated);
}

#[test]
fn local_server_replication_carries_absorbed_shell_impacts_to_next_snapshot() {
    let mut server = LocalAuthoritativeServer::new(ServerTickConfig::new(60, 20));
    let player_tank = server.player_tank();

    // Depress the gun fully, then fire into the dirt well short of the target tank.
    for client_tick in 0..30 {
        server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand { gun_pitch_delta: -1.0, ..TankCommand::idle() },
        });
    }
    server.tick_with_input(ClientInputCommand {
        client_tick: 30,
        tank_id: player_tank,
        command: TankCommand { fire: true, ..TankCommand::idle() },
    });

    let mut impact_snapshot = None;
    for client_tick in 31..=120 {
        let tick = server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::idle(),
        });
        if let Some(snapshot) = tick.snapshot
            && !snapshot.shell_impacts.is_empty()
        {
            impact_snapshot = Some(snapshot);
            break;
        }
    }

    // Snapshot cadence must not drop the impact: it is buffered to the next emitted snapshot,
    // so the firing client always learns where its shot died.
    let snapshot = impact_snapshot.expect("an absorbed shell must reach the client in a snapshot");
    let impact = &snapshot.shell_impacts[0];
    assert_eq!(impact.owner, player_tank);
    assert_eq!(impact.surface, game_core::ImpactSurface::Terrain);
}
