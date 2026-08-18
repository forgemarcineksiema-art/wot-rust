//! The tuning sweep — the wide arm of the balance instrument (`battle_host::stats`).
//!
//! Run it BY HAND around any damage-frequency constant change and quote the table in the commit
//! that moves the number:
//!
//! ```text
//! cargo run -p battle_host --example battle_sweep
//! ```
//!
//! An example rather than an `#[ignore]`d test on purpose: the quality gate forbids tests that
//! can decline to run, and this is an instrument (like `probe`), not a promise. The locking
//! promise — the frequency bands — lives in `tests/battle_statistics.rs`.

use battle_host::stats::run_measured_battle;
use game_core::VehicleKind;
use terrain::MapId;

fn main() {
    for seed in [3, 7, 11, 21, 34, 55, 89, 144] {
        let stats =
            run_measured_battle(seed, MapId::ProkhorovkaHill252_2, VehicleKind::T54_1951, 7.0);
        println!("{}", stats.table_row(seed));
    }
}
