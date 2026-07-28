use net::{CombatEvent, SequencedCombatEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteCombatEventError {
    Gap { expected: u64, received: u64 },
}

#[derive(Debug, Default)]
pub(super) struct RemoteCombatEventInbox {
    last_received_seq: Option<u64>,
}

impl RemoteCombatEventInbox {
    /// Accepts one retransmitted event window atomically.
    ///
    /// Old and overlapping items are already acknowledged, so they are discarded without a
    /// growing deduplication set. Every unseen item must extend the per-session sequence by one;
    /// a gap rejects the complete batch and leaves the acknowledgement cursor unchanged.
    pub(super) fn accept_batch(
        &mut self,
        batch: &[SequencedCombatEvent],
    ) -> Result<(Vec<CombatEvent>, Option<u64>), RemoteCombatEventError> {
        let mut candidate_last = self.last_received_seq;
        let mut accepted = Vec::new();

        for sequenced in batch {
            if candidate_last.is_some_and(|last| sequenced.delivery_seq <= last) {
                continue;
            }

            let expected = candidate_last.map_or(0, |last| last.saturating_add(1));
            if sequenced.delivery_seq != expected {
                return Err(RemoteCombatEventError::Gap {
                    expected,
                    received: sequenced.delivery_seq,
                });
            }

            accepted.push(sequenced.event.clone());
            candidate_last = Some(sequenced.delivery_seq);
        }

        self.last_received_seq = candidate_last;
        Ok((accepted, candidate_last))
    }

    #[cfg(test)]
    pub(super) fn last_received_seq(&self) -> Option<u64> {
        self.last_received_seq
    }
}

#[cfg(test)]
mod tests {
    use game_core::DamageEvent;

    use super::*;

    fn event(delivery_seq: u64) -> SequencedCombatEvent {
        SequencedCombatEvent {
            delivery_seq,
            event: CombatEvent::Damage(DamageEvent {
                damage_hp: delivery_seq as u32 + 1,
                ..DamageEvent::default()
            }),
        }
    }

    fn damage_values(events: &[CombatEvent]) -> Vec<u32> {
        events
            .iter()
            .map(|event| match event {
                CombatEvent::Damage(damage) => damage.damage_hp,
                other => panic!("test fixture is a damage event, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn duplicate_batch_returns_no_duplicate_events() {
        let mut inbox = RemoteCombatEventInbox::default();
        let batch = vec![event(0), event(1)];

        let (first, first_ack) = inbox.accept_batch(&batch).expect("initial batch");
        let (duplicate, duplicate_ack) = inbox.accept_batch(&batch).expect("duplicate batch");

        assert_eq!(damage_values(&first), vec![1, 2]);
        assert!(duplicate.is_empty());
        assert_eq!(first_ack, Some(1));
        assert_eq!(duplicate_ack, Some(1));
    }

    #[test]
    fn reordered_old_batch_cannot_move_the_cursor_backwards() {
        let mut inbox = RemoteCombatEventInbox::default();
        inbox.accept_batch(&[event(0), event(1), event(2)]).expect("initial batch");

        let (events, ack) = inbox.accept_batch(&[event(1), event(0)]).expect("old reordered batch");

        assert!(events.is_empty());
        assert_eq!(ack, Some(2));
        assert_eq!(inbox.last_received_seq(), Some(2));
    }

    #[test]
    fn overlapping_batch_returns_only_its_unseen_tail() {
        let mut inbox = RemoteCombatEventInbox::default();
        inbox.accept_batch(&[event(0), event(1)]).expect("initial batch");

        let (events, ack) =
            inbox.accept_batch(&[event(1), event(2), event(3)]).expect("overlapping batch");

        assert_eq!(damage_values(&events), vec![3, 4]);
        assert_eq!(ack, Some(3));
    }

    #[test]
    fn gap_rejects_the_entire_batch_without_advancing() {
        let mut inbox = RemoteCombatEventInbox::default();
        inbox.accept_batch(&[event(0)]).expect("initial batch");

        let error = inbox.accept_batch(&[event(1), event(3)]).expect_err("sequence gap");

        assert_eq!(error, RemoteCombatEventError::Gap { expected: 2, received: 3 });
        assert_eq!(inbox.last_received_seq(), Some(0));

        let (events, ack) = inbox.accept_batch(&[event(1), event(2)]).expect("complete retry");
        assert_eq!(damage_values(&events), vec![2, 3]);
        assert_eq!(ack, Some(2));
    }

    #[test]
    fn empty_batch_preserves_the_current_ack() {
        let mut inbox = RemoteCombatEventInbox::default();

        let (fresh_events, fresh_ack) = inbox.accept_batch(&[]).expect("fresh empty batch");
        assert!(fresh_events.is_empty());
        assert_eq!(fresh_ack, None);

        inbox.accept_batch(&[event(0)]).expect("initial event");
        let (events, ack) = inbox.accept_batch(&[]).expect("later empty batch");
        assert!(events.is_empty());
        assert_eq!(ack, Some(0));
        assert_eq!(inbox.last_received_seq(), Some(0));
    }
}
