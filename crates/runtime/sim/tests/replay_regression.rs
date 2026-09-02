use game_core::TankId;
use sim::{Replay, run_replay};

#[test]
fn drive_forward_replay_is_a_regression_test() {
    let replay: Replay =
        serde_json::from_str(include_str!("replays/drive_forward_v1.json")).expect("valid replay");

    let report = run_replay(&replay);
    let tank =
        report.tank(TankId(replay.expected.tank_id)).expect("expected tank in replay report");

    assert_eq!(report.final_tick, replay.expected.tick);
    // Straight drive: lateral/vertical state stays exactly zero — locks steering,
    // command ordering, and the flat-ground path that the old `>=` floors ignored.
    assert_eq!(tank.position.x, 0.0);
    assert_eq!(tank.position.y, 0.0);
    assert_eq!(tank.yaw_rad, 0.0);
    // Forward trajectory and turret are pinned tightly (the old floors passed under huge
    // regressions). Re-pinned 2026-08-23: `medium_test_tank()` is the T-54, not the deleted
    // prototype — the traverse and the first metres of launch are the benchmark hull's.
    // Re-pinned 2026-09-02 (Inny Poziom A11): the T-54's traverse went from 0.42 to 0.84 rad/s
    // (24 → 48 deg/s, the genre's value), so the same nine ticks of quarter-rate command turn
    // the turret exactly twice as far — 0.01575 → 0.0315 rad. The hull's launch is untouched.
    assert!((tank.position.z - 0.101224).abs() < 1e-4, "position.z drifted: {}", tank.position.z);
    assert!(
        (tank.turret_yaw_rad - 0.0315).abs() < 1e-4,
        "turret_yaw drifted: {}",
        tank.turret_yaw_rad
    );
    // Determinism: identical input reproduces identical state.
    assert_eq!(run_replay(&replay).tanks, report.tanks);
}
