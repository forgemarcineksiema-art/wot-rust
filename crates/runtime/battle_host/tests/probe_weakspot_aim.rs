//! TEMPORARY measurement probe (untracked): the #428 harness, re-created for the bot
//! weakspot-aiming before/after. Run explicitly:
//! `cargo test -p battle_host --test probe_weakspot_aim -- --ignored --nocapture`

use std::collections::HashSet;

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use game_core::ArmorZone;
use net::ClientInputCommand;
use sim::TankCommand;

#[test]
#[ignore = "measurement probe, not a lock"]
fn weakspot_aim_probe() {
    let mut total = (0u32, 0u32, 0u32, 0u32);
    for seed in [11u64, 21, 33] {
        let config =
            RandomBattleConfig::new(BattleSeed::fixed(seed), game_core::VehicleKind::T54_1951);
        let mut server =
            LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);
        let player = server.player_tank();
        let mut seen = HashSet::new();
        let mut pens = 0u32;
        let mut non_pens = 0u32;
        let mut ricochets = 0u32;
        let mut zone_hits: Vec<(ArmorZone, u32)> = Vec::new();
        let ticks = 600u64 * 60;
        for client_tick in 0..ticks {
            let out = server.tick_with_input(ClientInputCommand {
                client_tick,
                tank_id: player,
                command: TankCommand::idle(),
            });
            for event in &out.damage_events {
                if !seen.insert(event.event_id) {
                    continue;
                }
                if event.ricocheted {
                    ricochets += 1;
                }
                if event.penetrated {
                    pens += 1;
                } else {
                    non_pens += 1;
                }
                match zone_hits.iter_mut().find(|(zone, _)| *zone == event.armor_zone) {
                    Some((_, count)) => *count += 1,
                    None => zone_hits.push((event.armor_zone, 1)),
                }
            }
            if server.battle_outcome().is_some() {
                break;
            }
        }
        let snapshot = server.latest_snapshot();
        let kills = snapshot.tanks.iter().filter(|tank| tank.hit_points == 0).count() as u32;
        zone_hits.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        println!(
            "seed {seed}: pens {pens} non_pens {non_pens} ricochets {ricochets} kills {kills}"
        );
        println!("  zones: {zone_hits:?}");
        total.0 += pens;
        total.1 += non_pens;
        total.2 += ricochets;
        total.3 += kills;
    }
    println!(
        "TOTAL: pens {} non_pens {} ricochets {} kills {}",
        total.0, total.1, total.2, total.3
    );
}
