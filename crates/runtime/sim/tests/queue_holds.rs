//! A QUEUE HOLDS ITSELF UP — P1.3 of `docs/contact-and-tracks-program.md`.
//!
//! A Jacobi pass propagates a constraint one hull per iteration, so four iterations reach four
//! hulls deep and no further. Measured before the solver had a memory, that showed up exactly where
//! the arithmetic says it should: a pair pressed together settled at the 0.020 m of overlap it is
//! allowed, while seven settled at 0.037 m — the extra centimetre and a half is the solve running
//! out of iterations, not a rule anybody wrote.
//!
//! Worth stating what was NOT wrong, because the program predicted the wrong symptom and the
//! measurement corrected it: none of these queues ever jittered. Drift was 0.00000 m/tick at every
//! length, before the fix and after. A queue does not shake; it sinks. Warm starting is the cure
//! for the second thing.

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

/// The overlap the contact solver is allowed to leave alone (`POSITION_SLOP_M`). A queue may rest
/// inside it; sinking past it means the solve ran out of reach.
const ALLOWED_OVERLAP_M: f32 = 0.02;

/// Recorded before the solver had a memory, so a regression reads as a return to a known place
/// rather than as an unfamiliar number: 2 hulls 0.0202 m, 3 hulls 0.0215, 5 hulls 0.0277, 7 hulls
/// 0.0368. After: 0.0006, 0.0014, 0.0137, 0.0197.
#[test]
fn a_queue_of_hulls_neither_shakes_nor_sinks() {
    for count in [2usize, 3, 5, 7] {
        let (worst_step, worst_overlap) = press_a_queue(count);
        println!(
            "queue of {count}: worst step {worst_step:.5} m/tick, deepest overlap \
             {worst_overlap:.4} m"
        );
        assert!(
            worst_step <= 0.001,
            "a queue of {count} shifted {worst_step:.5} m in a tick while standing still"
        );
        assert!(
            worst_overlap <= ALLOWED_OVERLAP_M,
            "a queue of {count} sank {worst_overlap:.4} m into itself, past the {ALLOWED_OVERLAP_M} m \
             it is allowed — the solve is not reaching the far end of the chain"
        );
    }
}

/// Drive `count` hulls nose-to-tail into a wreck and report, over three seconds after they have
/// settled, the worst movement any of them makes in a tick and the deepest any pair overlaps.
fn press_a_queue(count: usize) -> (f32, f32) {
    let spec = TankSpec::t54_1951();
    let half_len = spec.hull_plan().half_length_m;
    let pitch = 2.0 * half_len + 1.0;
    let mut state = SimulationState::new();
    let ids: Vec<TankId> = (0..count)
        .map(|index| {
            let z = -(index as f32) * pitch;
            state.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::new(0.0, 0.0, z), 0.0)
        })
        .collect();
    // The hull at the head of the queue is a wreck: it blocks and never gives ground, so the whole
    // queue has something to pile up against.
    let wall = state.spawn_tank_with_yaw(TeamId(2), spec.clone(), Vec3::new(0.0, 0.0, pitch), 0.0);
    state.tank_mut(wall).expect("wall").hit_points = 0;

    let go: Vec<_> = ids.iter().map(|&id| (id, TankCommand::drive(1.0, 0.0))).collect();
    for _ in 0..1_800 {
        state.apply_commands(&go, step());
    }

    let (mut worst_step, mut worst_overlap) = (0.0_f32, 0.0_f32);
    let mut previous: Vec<Vec3> =
        ids.iter().map(|&id| state.tank(id).expect("tank").position).collect();
    for _ in 0..180 {
        state.apply_commands(&go, step());
        for (slot, &id) in ids.iter().enumerate() {
            let now = state.tank(id).expect("tank").position;
            worst_step = worst_step.max((now - previous[slot]).length());
            previous[slot] = now;
        }
        let mut zs: Vec<f32> =
            ids.iter().map(|&id| state.tank(id).expect("tank").position.z).collect();
        zs.push(state.tank(wall).expect("wall").position.z);
        zs.sort_by(|left, right| left.partial_cmp(right).expect("finite"));
        for pair in zs.windows(2) {
            worst_overlap = worst_overlap.max(2.0 * half_len - (pair[1] - pair[0]));
        }
    }
    (worst_step, worst_overlap)
}
