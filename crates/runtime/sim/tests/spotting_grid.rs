//! The sight line must sample the terrain at least as finely as the terrain is defined.
//!
//! `shell_trace` sweeps the ground at a fixed 1 m; the eye used to step a fixed 2 m. On a 5 m grid
//! that is 2.5 eye samples per cell and nothing can hide between them. On a finer grid it stops
//! being true — at 1.25 m cells a flat 2 m step is 0.6 samples per cell, so a one-cell ridge fits
//! between two steps. Then a bot fires at something it "sees" through a crest and the shell eats
//! the crest: the honesty doctrine breaking in the least visible place there is.
//!
//! This asserts the RULE rather than a scenario. The first version built a 12 m ridge and asserted
//! it blocked the line at every cell size — and it passed with the old flat step, because a ridge
//! that tall is caught by any sample near it. A test that cannot fail against the defect it was
//! written for is worse than no test.

/// `shell_trace::terrain::TERRAIN_SWEEP_STEP_M`, which is private and has no reason not to be.
const SHELL_TERRAIN_SWEEP_M: f32 = 1.0;

#[test]
fn the_eye_never_samples_more_coarsely_than_the_grid() {
    for cell_m in [10.0_f32, 5.0, 2.5, 2.0, 1.25, 0.5] {
        let step = sim::terrain_sight_step_m(cell_m);
        assert!(
            step <= cell_m,
            "cell {cell_m} m sampled every {step} m — a ridge one cell wide fits between steps"
        );
    }
}

#[test]
fn todays_grid_is_unchanged_and_a_denser_one_follows_it_down() {
    // 5 m is what every shipped map uses, and the step there must stay exactly what it always was:
    // densifying one map must not silently re-price spotting on the maps nobody touched.
    assert_eq!(sim::terrain_sight_step_m(5.0), 2.0);
    assert_eq!(sim::terrain_sight_step_m(2.5), 1.25);
    assert_eq!(sim::terrain_sight_step_m(1.25), 0.625);
}

#[test]
fn once_the_grid_is_fine_the_eye_keeps_up_with_the_shell() {
    // Below this cell size the shell's own 1 m sweep is the coarser instrument and the eye is
    // finer than it — which is the side of the disagreement that does not lie to a player.
    for cell_m in [2.0_f32, 1.25, 0.5] {
        assert!(
            sim::terrain_sight_step_m(cell_m) <= SHELL_TERRAIN_SWEEP_M,
            "at cell {cell_m} m the eye must not step past what the shell sweep resolves"
        );
    }
}
