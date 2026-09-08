//! Where one authoritative tick goes (Q8): the AI battle the garage's BATTLE deploys into,
//! 15v15 on the map `WOT_MAP` names (as in the game), warmed 1800 ticks, then 600 ticks summed
//! section by section. Read it against the client's frame log (`WOT_FRAME_LOG`), whose `host`
//! part is this tick inside the client's process.
//!
//! ```text
//! WOT_MAP=bystra-valley cargo run --release -p battle_host --example tick_sections
//! ```
//!
//! An instrument, not a promise: the numbers move with the machine and its heat.

use battle_host::{BattleSeed, LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use net::ClientInputCommand;
use sim::TankCommand;

const WARM_TICKS: u64 = 1800;
const MEASURED_TICKS: u64 = 600;

fn main() {
    let mut config = RandomBattleConfig::runtime_from_env(game_core::VehicleKind::T54_1951);
    config.seed = BattleSeed::fixed(42);
    let mut server = LocalAuthoritativeServer::new_ai_battle(ServerTickConfig::default(), config);
    let player = server.player_tank();
    let drive = |client_tick: u64| ClientInputCommand {
        client_tick,
        tank_id: player,
        command: TankCommand::drive(1.0, 0.0),
    };
    for tick in 0..WARM_TICKS {
        server.tick_with_player_input(drive(tick));
    }
    server.enable_tick_profile();
    for tick in WARM_TICKS..WARM_TICKS + MEASURED_TICKS {
        server.tick_with_player_input(drive(tick));
    }
    let sections = server.take_tick_profile().expect("armed");
    println!(
        "map {:?}, {} cover boxes, {} tanks",
        server.map_id(),
        server.cover_box_count(),
        server.tanks().len()
    );
    print!("{}", sections.table());
    println!(
        "observer masks computed {} times, handed back {} times",
        sections.mask_computations, sections.mask_reuses
    );
}
