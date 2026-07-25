use net::TankSnapshot;

use super::ClientApp;
use super::session::RemoteReconciliation;

impl ClientApp {
    pub(super) fn reconcile_remote_prediction(
        &mut self,
        authoritative: &TankSnapshot,
        reconciliation: RemoteReconciliation,
    ) {
        let Some(inputs) = reconciliation.replay_inputs else {
            self.predictor.snap_to_with_motion(authoritative, reconciliation.motion);
            return;
        };
        self.predictor.sync_to_with_motion(authoritative, reconciliation.motion);
        for input in inputs {
            if let Some(slot) = input.command.select_ammo {
                self.predictor.set_selected_ammo(slot);
            }
            self.step_prediction(&input.command);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use game_core::TankId;
    use net::ClientInputCommand;

    use super::*;

    fn delayed_command(sequence: u64) -> sim::TankCommand {
        let steer = if (sequence / 12).is_multiple_of(2) { 0.18 } else { -0.12 };
        sim::TankCommand::drive(0.82, steer)
    }

    #[test]
    fn delayed_jittered_non_idle_replay_has_no_periodic_backward_sawtooth() {
        let mut authority = ClientApp::new_seeded(73);
        let mut predicted = ClientApp::new_seeded(73);
        authority.seed_prediction();
        predicted.seed_prediction();

        // Begin six fixed ticks ahead of authority (100 ms at 60 Hz), as a remote client does.
        let mut sequence = 0_u64;
        let mut pending = VecDeque::new();
        for _ in 0..6 {
            let input = ClientInputCommand {
                client_tick: sequence,
                tank_id: TankId(1),
                command: delayed_command(sequence),
            };
            predicted.step_prediction(&input.command);
            pending.push_back(input);
            sequence += 1;
        }

        let mut worst_correction = 0.0_f32;
        let mut worst_velocity_error = 0.0_f32;
        for ack_advance in [2_usize, 4, 3, 2, 3, 4, 2, 3, 4, 3].into_iter().cycle().take(40) {
            // Jitter changes how many old commands the authoritative side catches up this
            // delivery. The player meanwhile produces the same number of new unacked commands.
            for _ in 0..ack_advance {
                let acknowledged = pending.pop_front().expect("six-tick pipeline");
                authority.step_prediction(&acknowledged.command);
                let input = ClientInputCommand {
                    client_tick: sequence,
                    tank_id: TankId(1),
                    command: delayed_command(sequence),
                };
                predicted.step_prediction(&input.command);
                pending.push_back(input);
                sequence += 1;
            }

            let before = predicted.predictor.position();
            let expected_motion = predicted.predictor.authoritative_motion();
            let authoritative = authority.local_render_tank().expect("authoritative tank");
            predicted.reconcile_remote_prediction(
                &authoritative,
                RemoteReconciliation {
                    motion: authority.predictor.authoritative_motion(),
                    replay_inputs: Some(pending.iter().cloned().collect()),
                },
            );
            worst_correction =
                worst_correction.max(predicted.predictor.position().distance(before));
            let actual_motion = predicted.predictor.authoritative_motion();
            worst_velocity_error = worst_velocity_error.max(
                glam::Vec3::from_array(actual_motion.velocity_mps)
                    .distance(glam::Vec3::from_array(expected_motion.velocity_mps))
                    .max(
                        (actual_motion.hull_yaw_velocity_rad_s
                            - expected_motion.hull_yaw_velocity_rad_s)
                            .abs(),
                    ),
            );
        }

        assert!(
            worst_correction < 1.0e-3,
            "replaying the live six-tick tail must preserve the predicted pose; \
             periodic correction was {worst_correction} m"
        );
        assert!(
            worst_velocity_error < 1.0e-4,
            "replay must restore velocity too; worst error was {worst_velocity_error}"
        );
    }
}
