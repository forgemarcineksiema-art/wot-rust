use std::collections::VecDeque;

use game_core::TankId;
use net::ClientInputCommand;
use sim::TankCommand;

const INPUT_WIRE_WINDOW: usize = 12;
const MAX_UNACKED_INPUTS: usize = 1_024;

#[derive(Debug, Default)]
pub(super) struct RemoteInputHistory {
    next_sequence: u64,
    last_wire_ack: Option<u64>,
    wire_unacked: VecDeque<ClientInputCommand>,
    prediction_history: VecDeque<ClientInputCommand>,
}

impl RemoteInputHistory {
    pub(super) fn enqueue(&mut self, tank_id: TankId, command: TankCommand) -> bool {
        if self.wire_unacked.len() == MAX_UNACKED_INPUTS {
            return false;
        }
        let input = ClientInputCommand { client_tick: self.next_sequence, tank_id, command };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.wire_unacked.push_back(input.clone());
        self.prediction_history.push_back(input);
        if self.prediction_history.len() > MAX_UNACKED_INPUTS {
            self.prediction_history.pop_front();
        }
        true
    }

    /// Advances only the retransmission window. Snapshot replay has a different clock: an
    /// `InputAck` can overtake an older `SnapshotDelivery`, whose state still needs commands
    /// between its aligned ACK and the locally predicted head.
    pub(super) fn acknowledge_wire(&mut self, sequence: Option<u64>) {
        let Some(sequence) = sequence else { return };
        if sequence >= self.next_sequence || self.last_wire_ack.is_some_and(|ack| sequence <= ack) {
            return;
        }
        self.last_wire_ack = Some(sequence);
        while self.wire_unacked.front().is_some_and(|input| input.client_tick <= sequence) {
            self.wire_unacked.pop_front();
        }
    }

    pub(super) fn wire_batch(&self) -> Vec<ClientInputCommand> {
        self.wire_unacked.iter().take(INPUT_WIRE_WINDOW).cloned().collect()
    }

    /// A terminal connection owns no commands anymore. Preserve the monotonically increasing
    /// sequence (useful for diagnostics), but release both retransmission and reconciliation
    /// tails so a dead session cannot sit on a 1,024-command zombie backlog.
    pub(super) fn clear_pending(&mut self) {
        self.wire_unacked.clear();
        self.prediction_history.clear();
    }

    /// Returns the exact contiguous tail after the ACK embedded in one accepted snapshot.
    /// A missing/truncated or over-budget tail forces a hard snap; partial replay would be a
    /// plausible-looking but incorrect pose.
    pub(super) fn prediction_replay_after(
        &mut self,
        sequence: Option<u64>,
        max_ticks: usize,
    ) -> Option<Vec<ClientInputCommand>> {
        let first_unprocessed = match sequence {
            Some(sequence) if sequence < self.next_sequence => sequence + 1,
            Some(_) => return None,
            None => 0,
        };
        while self
            .prediction_history
            .front()
            .is_some_and(|input| input.client_tick < first_unprocessed)
        {
            self.prediction_history.pop_front();
        }
        let tail_len = self.next_sequence.saturating_sub(first_unprocessed) as usize;
        if tail_len > max_ticks || self.prediction_history.len() < tail_len {
            return None;
        }
        let tail = self
            .prediction_history
            .iter()
            .rev()
            .take(tail_len)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let contiguous = tail
            .iter()
            .enumerate()
            .all(|(offset, input)| input.client_tick == first_unprocessed + offset as u64);
        contiguous.then_some(tail)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.wire_unacked.len()
    }

    #[cfg(test)]
    pub(super) fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_prunes_only_confirmed_inputs_and_sequence_never_restarts() {
        let mut history = RemoteInputHistory::default();
        for _ in 0..5 {
            assert!(history.enqueue(TankId(7), TankCommand::idle()));
        }
        history.acknowledge_wire(Some(2));
        assert_eq!(history.len(), 2);
        assert_eq!(history.wire_batch()[0].client_tick, 3);
        history.acknowledge_wire(Some(1));
        assert_eq!(history.len(), 2, "a reordered ACK cannot move history backward");
        assert!(history.enqueue(TankId(7), TankCommand::idle()));
        assert_eq!(history.next_sequence(), 6);
    }

    #[test]
    fn wire_ack_does_not_prune_snapshot_replay_history() {
        let mut history = RemoteInputHistory::default();
        for _ in 0..25 {
            assert!(history.enqueue(TankId(1), TankCommand::drive(1.0, 0.0)));
        }
        history.acknowledge_wire(Some(20));
        assert_eq!(history.len(), 4);
        let replay = history.prediction_replay_after(Some(17), 8).expect("exact tail");
        assert_eq!(
            replay.iter().map(|input| input.client_tick).collect::<Vec<_>>(),
            (18..25).collect::<Vec<_>>()
        );
    }

    #[test]
    fn over_budget_replay_snaps_without_pruning_the_network_tail() {
        let mut history = RemoteInputHistory::default();
        for _ in 0..9 {
            assert!(history.enqueue(TankId(1), TankCommand::drive(1.0, 0.0)));
        }
        assert!(history.prediction_replay_after(None, 8).is_none());
        assert_eq!(history.len(), 9);
        assert_eq!(history.wire_batch().len(), 9);
    }

    #[test]
    fn terminal_clear_releases_both_histories_without_reusing_sequence_numbers() {
        let mut history = RemoteInputHistory::default();
        for _ in 0..5 {
            assert!(history.enqueue(TankId(1), TankCommand::idle()));
        }
        history.clear_pending();
        assert!(history.wire_batch().is_empty());
        assert_eq!(history.len(), 0);
        assert_eq!(history.next_sequence(), 5);
        assert!(history.enqueue(TankId(1), TankCommand::idle()));
        assert_eq!(history.wire_batch()[0].client_tick, 5);
    }
}
