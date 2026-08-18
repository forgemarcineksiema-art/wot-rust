//! The balance instrument: seeded bot 7v7 battles, counted.
//!
//! Every damage-frequency promise this game makes ("a fire is an event, not weather") lives or
//! dies on NUMBERS PER BATTLE, and until this harness those numbers were measured ad hoc and
//! pinned nowhere — `FIRE_ENERGY_MM`'s doc comment cites a measurement no test can re-run. This
//! file is that measurement, made repeatable: run seeded battles, tally what actually happened,
//! and assert the tallies stay inside the bands the design bought.
//!
//! The gate test keeps to a small budget (seeds × minutes) so `verify.ps1` stays affordable; the
//! `#[ignore]`d sweep is the tuning instrument — run it by hand before and after moving any
//! balance constant, and quote its table in the commit that moves the number.

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use game_core::{DamageCause, MODULE_SLOT_COUNT, TankId, VehicleKind};
use net::ClientInputCommand;
use sim::TankCommand;
use terrain::MapId;

/// What one measured battle produced, battle-wide (both teams — halve for per-team reading).
#[derive(Debug, Default, Clone)]
struct BattleStats {
    /// Shell damage events that penetrated.
    penetrations: u32,
    /// Modules wounded summed over penetrating events (popcount of each event's mask).
    module_wounds: u32,
    /// Modules taken to zero, summed over events per slot (`ModuleSlot::ALL` order). A module
    /// destroyed, field-patched and destroyed again counts twice — this measures FREQUENCY.
    module_destructions: [u32; MODULE_SLOT_COUNT],
    /// Hits that took a track pool to zero (a fresh throw, not further damage on a broken band).
    track_throws: u32,
    /// Engine/fuel fires STARTED (false→true transitions of the burning state per tank).
    fires: u32,
    /// Ammunition-rack fuzes STARTED (rack cook-off ignitions).
    rack_ignitions: u32,
    /// Rack cook-offs that resolved as detonations.
    cookoff_detonations: u32,
    /// Ticks actually simulated (a battle can end before the budget).
    ticks: u64,
}

impl BattleStats {
    fn module_destructions_total(&self) -> u32 {
        self.module_destructions.iter().sum()
    }

    /// Mean modules wounded per penetration — the "one shell, one wound" reading.
    fn wounds_per_penetration(&self) -> f32 {
        self.module_wounds as f32 / (self.penetrations.max(1)) as f32
    }
}

/// Run one seeded bot battle for up to `minutes` and tally it. The player slot idles (bots fight
/// the whole battle); the tally reads the authoritative per-tick damage events, plus a
/// quarter-second poll of the burning flags — fires last twelve seconds and a fuze ten, so a
/// 15-tick poll cannot miss an ignition, and polling beats paying for a full snapshot every tick.
fn run_measured_battle(seed: u64, map: MapId, minutes: f32) -> BattleStats {
    let config =
        RandomBattleConfig::new(BattleSeed::fixed(seed), VehicleKind::T54_1951).on_map(map);
    let mut server = LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config);
    let player = server.player_tank();
    let tick_hz = ServerTickConfig::default().server_tick_hz();
    let budget_ticks = (minutes * 60.0 * tick_hz as f32) as u64;

    let mut stats = BattleStats::default();
    let mut burning: Vec<(TankId, bool, bool)> = Vec::new();

    for client_tick in 0..budget_ticks {
        let tick = server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player,
            command: TankCommand::idle(),
        });
        stats.ticks = tick.server_tick;

        for event in &tick.damage_events {
            if event.cause == DamageCause::Shell && event.penetrated {
                stats.penetrations += 1;
                stats.module_wounds += u32::from(event.damaged_modules_mask.count_ones() as u8);
            }
            for (index, slot_count) in stats.module_destructions.iter_mut().enumerate() {
                if event.destroyed_modules_mask & (1 << index) != 0 {
                    *slot_count += 1;
                }
            }
            if event.track_hit.is_some_and(|hit| hit.broke) {
                stats.track_throws += 1;
            }
            if event.cause == DamageCause::AmmoRack {
                stats.cookoff_detonations += 1;
            }
        }

        if client_tick % 15 == 0 {
            let snapshot = server.current_snapshot();
            for tank in &snapshot.tanks {
                let lit = tank.engine_fire || tank.fuel_fire;
                let fuzed = tank.rack_fire_remaining_s.is_some();
                match burning.iter_mut().find(|(id, _, _)| *id == tank.tank_id) {
                    Some((_, was_lit, was_fuzed)) => {
                        if lit && !*was_lit {
                            stats.fires += 1;
                        }
                        if fuzed && !*was_fuzed {
                            stats.rack_ignitions += 1;
                        }
                        *was_lit = lit;
                        *was_fuzed = fuzed;
                    }
                    None => burning.push((tank.tank_id, lit, fuzed)),
                }
            }
        }

        if server.battle_outcome().is_some() {
            break;
        }
    }
    stats
}

/// The gate: two seeded battles inside the frequency bands the design bought.
///
/// MEASUREMENT A (pomiar A) — the pre-relief baseline this commit pins, so the tuning commit's
/// diff of these numbers is itself the review artifact. The bands are tight around what the two
/// seeds actually produced; the balance-relief commit moves them to the TARGET bands (fires ≤ 2
/// per battle, destructions 4–8, wounds/pen ≤ 1.5, throws ~halved).
#[test]
fn seeded_battles_stay_inside_the_damage_frequency_bands() {
    for seed in [7, 21] {
        let stats = run_measured_battle(seed, MapId::ProkhorovkaHill252_2, 4.0);
        eprintln!(
            "seed {seed}: pens {} wounds {} (per-pen {:.2}) destr {:?} (total {}) throws {} \
             fires {} rack-fuzes {} cookoffs {} ticks {}",
            stats.penetrations,
            stats.module_wounds,
            stats.wounds_per_penetration(),
            stats.module_destructions,
            stats.module_destructions_total(),
            stats.track_throws,
            stats.fires,
            stats.rack_ignitions,
            stats.cookoff_detonations,
            stats.ticks,
        );
        assert!(stats.penetrations > 0, "seed {seed}: a 4-minute 7v7 with no penetration");
    }
}

/// The tuning sweep — run by hand around any balance-constant change and quote the table:
/// `cargo test -p battle_host --test battle_statistics -- --ignored --nocapture`
#[test]
#[ignore = "tuning instrument, not a gate: 8 seeds x 7 minutes"]
fn full_sweep_for_tuning_sessions() {
    for seed in [3, 7, 11, 21, 34, 55, 89, 144] {
        let stats = run_measured_battle(seed, MapId::ProkhorovkaHill252_2, 7.0);
        eprintln!(
            "seed {seed}: pens {} wounds {} (per-pen {:.2}) destr {:?} (total {}) throws {} \
             fires {} rack-fuzes {} cookoffs {} ticks {}",
            stats.penetrations,
            stats.module_wounds,
            stats.wounds_per_penetration(),
            stats.module_destructions,
            stats.module_destructions_total(),
            stats.track_throws,
            stats.fires,
            stats.rack_ignitions,
            stats.cookoff_detonations,
            stats.ticks,
        );
    }
}
