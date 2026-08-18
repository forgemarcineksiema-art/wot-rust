//! The locking arm of the balance instrument (`battle_host::stats`): seeded battles must stay
//! inside the damage-frequency bands the design bought. The wide tuning sweep is
//! `examples/battle_sweep.rs` — run it by hand around any balance-constant change and quote its
//! table in the commit that moves the number.

use battle_host::stats::run_measured_battle;
use game_core::VehicleKind;
use terrain::MapId;

/// The gate: two seeded battles inside the frequency bands the design bought.
///
/// THE TARGET BANDS (frequency-relief pass, user decision 2026-08-18 "mocno: rzadkie
/// wydarzenia"), asserted battle-wide (both teams):
/// - fires ≤ 2 (≈ one per team at most),
/// - module destructions ≤ 8 (2–4 per team is the notable-event budget),
/// - modules wounded per penetration ≤ 1.5 (one shell, one wound — usually),
/// - track throws ≤ 3 (a tactic, not weather).
///
/// Against pomiar A (baseline, first harness commit) the 8-seed sweep moved: destructions
/// mean 7.5 → 5.5 per battle, fires mean 1.4 → 0.5, and a healthy module can no longer be
/// destroyed by a single touch (`MODULE_WOUND_SCALE`). Tolerances are bands, not pins, so the
/// gate survives unrelated sim drift; re-run the sweep before moving any constant.
#[test]
fn seeded_battles_stay_inside_the_damage_frequency_bands() {
    for seed in [7, 21] {
        let stats =
            run_measured_battle(seed, MapId::ProkhorovkaHill252_2, VehicleKind::T54_1951, 4.0);
        eprintln!("{}", stats.table_row(seed));
        assert!(stats.penetrations > 0, "seed {seed}: a 4-minute 7v7 with no penetration");
        assert!(stats.fires <= 2, "seed {seed}: {} fires — a fire must be an event", stats.fires);
        assert!(
            stats.module_destructions_total() <= 8,
            "seed {seed}: {} module destructions — destruction must be an event",
            stats.module_destructions_total(),
        );
        assert!(
            stats.wounds_per_penetration() <= 1.5,
            "seed {seed}: {:.2} modules wounded per penetration — one shell, one wound",
            stats.wounds_per_penetration(),
        );
        assert!(
            stats.track_throws <= 3,
            "seed {seed}: {} track throws — tracking is a tactic, not weather",
            stats.track_throws,
        );
        // Crew hits (v46): with the WHOLE fleet's stations authored the sweep measured 0-8 per
        // battle (mean 3.5) — a knocked-out crewman roughly twice a battle per team, and the
        // max landing on the one bloodbath seed. The dial is `CREW_KNOCK_ENERGY_MM`.
        assert!(
            stats.crew_hits <= 8,
            "seed {seed}: {} crew hits — a knocked-out crewman must stay an event",
            stats.crew_hits,
        );
    }
}
