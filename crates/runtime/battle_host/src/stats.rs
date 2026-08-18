//! The balance instrument: a seeded bot 7v7, counted.
//!
//! Every damage-frequency promise this game makes ("a fire is an event, not weather") lives or
//! dies on NUMBERS PER BATTLE, and before this module those numbers were measured ad hoc and
//! pinned nowhere — `FIRE_ENERGY_MM`'s original doc comment cites a measurement no test could
//! re-run. This is that measurement made repeatable; the locking bands live in
//! `tests/battle_statistics.rs`, and the wide tuning sweep is `examples/battle_sweep.rs` (an
//! instrument like `probe`, not an `#[ignore]`d test — the gate has no silent skips).

use game_core::{DamageCause, MODULE_SLOT_COUNT, TankId, VehicleKind};
use net::ClientInputCommand;
use sim::TankCommand;
use terrain::MapId;

use crate::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};

/// What one measured battle produced, battle-wide (both teams — halve for per-team reading).
#[derive(Debug, Default, Clone)]
pub struct BattleStats {
    /// Shell damage events that penetrated.
    pub penetrations: u32,
    /// Modules wounded summed over penetrating events (popcount of each event's mask).
    pub module_wounds: u32,
    /// Modules taken to zero, summed over events per slot (`ModuleSlot::ALL` order). A module
    /// destroyed, field-patched and destroyed again counts twice — this measures FREQUENCY.
    pub module_destructions: [u32; MODULE_SLOT_COUNT],
    /// Hits that took a track pool to zero (a fresh throw, not further damage on a broken band).
    pub track_throws: u32,
    /// Engine/fuel fires STARTED (false→true transitions of the burning state per tank).
    pub fires: u32,
    /// Ammunition-rack fuzes STARTED (rack cook-off ignitions).
    pub rack_ignitions: u32,
    /// Rack cook-offs that resolved as detonations.
    pub cookoff_detonations: u32,
    /// Crewmen knocked out (popcount of each event's `crew_hits_mask`, v46). Includes the
    /// back-face spall knocks below — this is the TOTAL the crew-frequency band polices.
    pub crew_hits: u32,
    /// Crewmen knocked out by back-face spall alone: crew hits on NON-penetrating shell events
    /// (the only way a non-penetration wounds a man). Spall must stay a minority wound source —
    /// its own band lives beside the total's in `tests/battle_statistics.rs`.
    pub spall_crew_hits: u32,
    /// Modules scratched by back-face spall: module masks on non-penetrating shell events.
    pub spall_module_wounds: u32,
    /// Ticks actually simulated (a battle can end before the budget).
    pub ticks: u64,
}

impl BattleStats {
    pub fn module_destructions_total(&self) -> u32 {
        self.module_destructions.iter().sum()
    }

    /// Mean modules wounded per penetration — the "one shell, one wound" reading.
    pub fn wounds_per_penetration(&self) -> f32 {
        self.module_wounds as f32 / (self.penetrations.max(1)) as f32
    }

    /// One row of the sweep table, for the instrument's output and for commit messages.
    pub fn table_row(&self, seed: u64) -> String {
        format!(
            "seed {seed}: pens {} wounds {} (per-pen {:.2}) destr {:?} (total {}) throws {} \
             fires {} rack-fuzes {} cookoffs {} crew-hits {} (spall {}) spall-wounds {} ticks {}",
            self.penetrations,
            self.module_wounds,
            self.wounds_per_penetration(),
            self.module_destructions,
            self.module_destructions_total(),
            self.track_throws,
            self.fires,
            self.rack_ignitions,
            self.cookoff_detonations,
            self.crew_hits,
            self.spall_crew_hits,
            self.spall_module_wounds,
            self.ticks,
        )
    }
}

/// Run one seeded bot battle for up to `minutes` and tally it. The player slot idles (bots fight
/// the whole battle; `player_vehicle` only anchors the roster seed); the tally reads the
/// authoritative per-tick damage events, plus a quarter-second poll of the burning flags — fires
/// last twelve seconds and a fuze ten, so a 15-tick poll cannot miss an ignition, and polling
/// beats paying for a full snapshot every tick.
pub fn run_measured_battle(
    seed: u64,
    map: MapId,
    player_vehicle: VehicleKind,
    minutes: f32,
) -> BattleStats {
    let config = RandomBattleConfig::new(BattleSeed::fixed(seed), player_vehicle).on_map(map);
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
                stats.module_wounds += event.damaged_modules_mask.count_ones();
            } else if event.cause == DamageCause::Shell {
                // A non-penetrating shell wounds a man only through back-face spall, so the
                // crew mask here IS the spall count. Modules need the suspension bit masked
                // off first: the exposed running gear takes non-penetrating pokes on its own
                // (`requires_penetration: false`), spall or no spall.
                stats.spall_crew_hits += event.crew_hits_mask.count_ones();
                let interior_mask = event.damaged_modules_mask
                    & !game_core::ModuleSlot::Suspension.destroyed_mask_bit();
                stats.spall_module_wounds += interior_mask.count_ones();
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
            stats.crew_hits += event.crew_hits_mask.count_ones();
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
