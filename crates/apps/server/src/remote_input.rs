use std::collections::BTreeMap;

use game_core::TankId;
use net::ClientInputCommand;
use sim::TankCommand;

pub(crate) const LAST_INPUT_HOLD_TICKS: u64 = 30;
const MAX_BUFFERED_INPUTS: usize = 64;
const MAX_INPUT_AHEAD: u64 = 64;

#[derive(Debug)]
pub(crate) struct RemoteInputQueue {
    next_sequence: u64,
    pending: BTreeMap<u64, TankCommand>,
    continuous: TankCommand,
    last_processed: Option<u64>,
    last_received_server_tick: Option<u64>,
}

impl Default for RemoteInputQueue {
    fn default() -> Self {
        Self {
            next_sequence: 0,
            pending: BTreeMap::new(),
            continuous: TankCommand::idle(),
            last_processed: None,
            last_received_server_tick: None,
        }
    }
}

impl RemoteInputQueue {
    pub(crate) fn ingest_batch(
        &mut self,
        assigned_tank: TankId,
        commands: &[ClientInputCommand],
        server_tick: u64,
    ) -> bool {
        let mut valid = false;
        for input in commands {
            if !self.accepts(assigned_tank, input) {
                continue;
            }
            valid = true;
            self.last_received_server_tick = Some(server_tick);
            if input.client_tick >= self.next_sequence {
                self.pending.entry(input.client_tick).or_insert(input.command);
            }
        }
        valid
    }

    pub(crate) fn command_for_tick(&mut self, server_tick: u64) -> TankCommand {
        if let Some(command) = self.pending.remove(&self.next_sequence) {
            self.last_processed = Some(self.next_sequence);
            self.next_sequence = self.next_sequence.saturating_add(1);
            self.continuous = continuous_only(command);
            return command;
        }
        if self
            .last_received_server_tick
            .is_some_and(|heard| server_tick.saturating_sub(heard) <= LAST_INPUT_HOLD_TICKS)
        {
            self.continuous
        } else {
            TankCommand::idle()
        }
    }

    pub(crate) fn last_processed(&self) -> Option<u64> {
        self.last_processed
    }

    fn accepts(&self, assigned_tank: TankId, input: &ClientInputCommand) -> bool {
        if input.tank_id != assigned_tank || !command_is_finite(input.command) {
            return false;
        }
        if input.client_tick < self.next_sequence {
            return true;
        }
        let ahead = input.client_tick.saturating_sub(self.next_sequence);
        if ahead > MAX_INPUT_AHEAD {
            return false;
        }
        self.pending.contains_key(&input.client_tick) || self.pending.len() < MAX_BUFFERED_INPUTS
    }
}

fn continuous_only(mut command: TankCommand) -> TankCommand {
    command.fire = false;
    command.select_ammo = None;
    command
}

fn command_is_finite(command: TankCommand) -> bool {
    let signed_axis = |value: f32| value.is_finite() && (-1.0..=1.0).contains(&value);
    signed_axis(command.throttle)
        && signed_axis(command.steer)
        && command.brake.is_finite()
        && (0.0..=1.0).contains(&command.brake)
        && signed_axis(command.turret_yaw_delta)
        && signed_axis(command.gun_pitch_delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequenced_input(sequence: u64, tank_id: u64, command: TankCommand) -> ClientInputCommand {
        ClientInputCommand { client_tick: sequence, tank_id: TankId(tank_id), command }
    }

    #[test]
    fn ordered_duplicates_apply_one_shots_exactly_once() {
        let mut queue = RemoteInputQueue::default();
        let fire = TankCommand { fire: true, ..TankCommand::drive(1.0, 0.2) };
        let switch = TankCommand { select_ammo: Some(1), ..TankCommand::drive(0.5, 0.0) };
        assert!(queue.ingest_batch(
            TankId(7),
            &[
                sequenced_input(1, 7, switch),
                sequenced_input(0, 7, fire),
                sequenced_input(0, 7, fire),
            ],
            90,
        ));

        assert!(queue.command_for_tick(90).fire);
        let second = queue.command_for_tick(91);
        assert_eq!(second.select_ammo, Some(1));
        let held = queue.command_for_tick(92);
        assert!(!held.fire);
        assert_eq!(held.select_ammo, None);
        assert_eq!(queue.last_processed(), Some(1));
    }

    #[test]
    fn invalid_tank_non_finite_and_future_inputs_are_rejected() {
        let mut queue = RemoteInputQueue::default();
        let nan = TankCommand { throttle: f32::NAN, ..TankCommand::idle() };
        let huge = TankCommand { steer: 1.0e30, ..TankCommand::idle() };
        assert!(!queue.ingest_batch(TankId(7), &[sequenced_input(0, 8, TankCommand::idle())], 0));
        assert!(!queue.ingest_batch(TankId(7), &[sequenced_input(0, 7, nan)], 0));
        assert!(!queue.ingest_batch(TankId(7), &[sequenced_input(0, 7, huge)], 0));
        assert!(!queue.ingest_batch(
            TankId(7),
            &[sequenced_input(MAX_INPUT_AHEAD + 1, 7, TankCommand::idle())],
            0,
        ));
        assert_eq!(queue.command_for_tick(0), TankCommand::idle());
        assert_eq!(queue.last_processed(), None);
    }

    #[test]
    fn a_gap_holds_continuous_axes_without_repeating_edges() {
        let mut queue = RemoteInputQueue::default();
        let command = TankCommand {
            throttle: 0.8,
            steer: -0.3,
            fire: true,
            select_ammo: Some(2),
            ..TankCommand::idle()
        };
        assert!(queue.ingest_batch(TankId(3), &[sequenced_input(0, 3, command)], 10));
        assert_eq!(queue.command_for_tick(10), command);
        let held = queue.command_for_tick(11);
        assert_eq!(held.throttle, 0.8);
        assert_eq!(held.steer, -0.3);
        assert!(!held.fire);
        assert_eq!(held.select_ammo, None);
        assert_eq!(queue.command_for_tick(10 + LAST_INPUT_HOLD_TICKS + 1), TankCommand::idle());
    }
}
