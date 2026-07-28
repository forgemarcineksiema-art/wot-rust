use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use net::{CombatEvent, ProtocolMessage, SequencedCombatEvent};

/// Deepest the per-recipient lane may get before the session fails loud.
///
/// Sized by the v39 JOIN SEED, which is the largest burst this queue ever sees: a crew arriving
/// mid-battle is handed every hull's existing perforations at once, bounded by
/// `MAX_ARMOR_BREACHES * MAX_BREACH_FRAGMENTS_PER_GROUP` per tank across a full 7v7 — 672. The
/// ordinary combat trickle is a handful of events a second, so the headroom above that costs
/// nothing in practice and refusing a legitimate late joiner would cost a player their battle.
pub const MAX_PENDING_COMBAT_EVENTS: usize = 1_024;

#[derive(Debug)]
pub(crate) enum RemoteEventQueueError {
    Overflow { capacity: usize },
    Empty,
    SequenceExhausted,
    Encode(net::NetError),
    EventTooLarge { delivery_seq: u64, encoded_len: usize, max: usize },
}

impl fmt::Display for RemoteEventQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow { capacity } => {
                write!(formatter, "combat event queue reached its {capacity}-event capacity")
            }
            Self::Empty => formatter.write_str("combat event queue is empty"),
            Self::SequenceExhausted => {
                formatter.write_str("combat event delivery sequence is exhausted")
            }
            Self::Encode(error) => write!(formatter, "combat event batch encode failed: {error}"),
            Self::EventTooLarge { delivery_seq, encoded_len, max } => write!(
                formatter,
                "combat event {delivery_seq} encodes to {encoded_len} bytes, above the {max}-byte datagram limit"
            ),
        }
    }
}

impl Error for RemoteEventQueueError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<net::NetError> for RemoteEventQueueError {
    fn from(error: net::NetError) -> Self {
        Self::Encode(error)
    }
}

#[derive(Debug, Default)]
pub(crate) struct RemoteEventQueue {
    pending: VecDeque<SequencedCombatEvent>,
    next_delivery_seq: u64,
    highest_sent: Option<u64>,
}

impl RemoteEventQueue {
    pub(crate) fn enqueue(&mut self, event: CombatEvent) -> Result<u64, RemoteEventQueueError> {
        if self.pending.len() >= MAX_PENDING_COMBAT_EVENTS {
            return Err(RemoteEventQueueError::Overflow { capacity: MAX_PENDING_COMBAT_EVENTS });
        }
        let delivery_seq = self.next_delivery_seq;
        let next_delivery_seq =
            delivery_seq.checked_add(1).ok_or(RemoteEventQueueError::SequenceExhausted)?;
        self.pending.push_back(SequencedCombatEvent { delivery_seq, event });
        self.next_delivery_seq = next_delivery_seq;
        Ok(delivery_seq)
    }

    pub(crate) fn acknowledge(&mut self, last_received_seq: u64) -> bool {
        let Some(oldest) = self.pending.front().map(|event| event.delivery_seq) else {
            return false;
        };
        let newest = self.pending.back().expect("non-empty queue").delivery_seq;
        let Some(highest_sent) = self.highest_sent else {
            return false;
        };
        if last_received_seq < oldest
            || last_received_seq > newest
            || last_received_seq > highest_sent
        {
            return false;
        }
        while self.pending.front().is_some_and(|event| event.delivery_seq <= last_received_seq) {
            self.pending.pop_front();
        }
        true
    }

    pub(crate) fn batch(
        &mut self,
        session_id: u64,
    ) -> Result<ProtocolMessage, RemoteEventQueueError> {
        if self.pending.is_empty() {
            return Err(RemoteEventQueueError::Empty);
        }

        let mut prefix = Vec::new();
        let mut selected = None;
        for event in &self.pending {
            prefix.push(event.clone());
            let candidate =
                ProtocolMessage::CombatEventBatch { session_id, events: prefix.clone() };
            let encoded_len = net::encode_frame(&candidate)?.len();
            if encoded_len > net::transport::MAX_DATAGRAM_PAYLOAD {
                prefix.pop();
                if selected.is_none() {
                    return Err(RemoteEventQueueError::EventTooLarge {
                        delivery_seq: event.delivery_seq,
                        encoded_len,
                        max: net::transport::MAX_DATAGRAM_PAYLOAD,
                    });
                }
                break;
            }
            selected = Some(candidate);
        }

        let message = selected.expect("a non-empty queue produced a valid prefix");
        let ProtocolMessage::CombatEventBatch { events, .. } = &message else {
            unreachable!("selected messages are always combat event batches");
        };
        let last_sent = events.last().expect("a batch always contains an event").delivery_seq;
        self.highest_sent =
            Some(self.highest_sent.map_or(last_sent, |previous| previous.max(last_sent)));
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_ID: u64 = 0x38_0001;

    fn damage(amount: u32) -> CombatEvent {
        CombatEvent::Damage(game_core::DamageEvent { damage_hp: amount, ..Default::default() })
    }

    fn batch_events(message: &ProtocolMessage) -> &[SequencedCombatEvent] {
        let ProtocolMessage::CombatEventBatch { events, .. } = message else {
            panic!("expected a combat event batch");
        };
        events
    }

    #[test]
    fn repeats_the_same_oldest_events_until_their_ack_arrives() {
        let mut queue = RemoteEventQueue::default();
        queue.enqueue(damage(10)).expect("first event");
        queue.enqueue(damage(20)).expect("second event");

        let first = queue.batch(SESSION_ID).expect("first send");
        let repeated = queue.batch(SESSION_ID).expect("lost send repeats");
        assert_eq!(repeated, first);

        assert!(queue.acknowledge(0));
        let remainder = queue.batch(SESSION_ID).expect("unacknowledged remainder");
        assert_eq!(batch_events(&remainder)[0].delivery_seq, 1);
        assert!(queue.acknowledge(1));
        assert!(queue.pending.is_empty());
        assert!(matches!(queue.batch(SESSION_ID), Err(RemoteEventQueueError::Empty)));
    }

    #[test]
    fn future_unsent_and_stale_acks_do_not_mutate_the_queue() {
        let mut queue = RemoteEventQueue::default();
        for amount in 0..64 {
            queue.enqueue(damage(amount)).expect("within capacity");
        }
        let sent = queue.batch(SESSION_ID).expect("batch");
        let sent_events = batch_events(&sent);
        let highest_sent = sent_events.last().expect("non-empty").delivery_seq;
        assert!(highest_sent < 63, "one datagram must leave an unsent suffix");

        let original_len = queue.pending.len();
        assert!(!queue.acknowledge(highest_sent + 1));
        assert!(!queue.acknowledge(u64::MAX));
        assert_eq!(queue.pending.len(), original_len);

        assert!(queue.acknowledge(0));
        let after_ack = queue.pending.len();
        assert!(!queue.acknowledge(0));
        assert_eq!(queue.pending.len(), after_ack);
    }

    #[test]
    fn acknowledgement_deduplicates_exactly_one_contiguous_prefix() {
        let mut queue = RemoteEventQueue::default();
        assert_eq!(queue.enqueue(damage(7)).expect("event"), 0);
        queue.batch(SESSION_ID).expect("send");
        assert!(queue.acknowledge(0));
        assert!(!queue.acknowledge(0), "a duplicate ack is stale");

        assert_eq!(queue.enqueue(damage(8)).expect("next event"), 1);
        let next = queue.batch(SESSION_ID).expect("next send");
        assert_eq!(batch_events(&next).len(), 1);
        assert_eq!(batch_events(&next)[0].delivery_seq, 1);
    }

    #[test]
    fn capacity_overflow_drops_nothing_and_consumes_no_sequence() {
        let mut queue = RemoteEventQueue::default();
        for sequence in 0..MAX_PENDING_COMBAT_EVENTS {
            assert_eq!(
                queue.enqueue(damage(sequence as u32)).expect("within capacity"),
                sequence as u64
            );
        }
        assert!(matches!(
            queue.enqueue(damage(999)),
            Err(RemoteEventQueueError::Overflow { capacity: MAX_PENDING_COMBAT_EVENTS })
        ));
        assert_eq!(queue.pending.len(), MAX_PENDING_COMBAT_EVENTS);

        let batch = queue.batch(SESSION_ID).expect("send prefix");
        let last_sent = batch_events(&batch).last().expect("non-empty").delivery_seq;
        assert!(queue.acknowledge(last_sent));
        assert_eq!(
            queue.enqueue(damage(1_000)).expect("room after ack"),
            MAX_PENDING_COMBAT_EVENTS as u64,
            "the sequence continues past the capacity it just cleared"
        );
    }

    #[test]
    fn batch_is_the_largest_oldest_prefix_that_fits_one_datagram() {
        let mut queue = RemoteEventQueue::default();
        for amount in 0..MAX_PENDING_COMBAT_EVENTS {
            queue.enqueue(damage(amount as u32)).expect("within capacity");
        }

        let batch = queue.batch(SESSION_ID).expect("batch");
        let encoded = net::encode_frame(&batch).expect("encode selected batch");
        let events = batch_events(&batch);
        assert!(!events.is_empty());
        assert!(encoded.len() <= net::transport::MAX_DATAGRAM_PAYLOAD);
        assert!(events.len() < queue.pending.len());

        let mut one_too_many = events.to_vec();
        one_too_many.push(queue.pending[events.len()].clone());
        let oversized =
            ProtocolMessage::CombatEventBatch { session_id: SESSION_ID, events: one_too_many };
        assert!(
            net::encode_frame(&oversized).expect("encode oversized candidate").len()
                > net::transport::MAX_DATAGRAM_PAYLOAD
        );
    }

    #[test]
    fn reset_restores_default_and_restarts_delivery_at_zero() {
        let mut queue = RemoteEventQueue::default();
        queue.enqueue(damage(1)).expect("event");
        queue.batch(SESSION_ID).expect("send");
        queue = RemoteEventQueue::default();

        assert!(queue.pending.is_empty());
        assert_eq!(queue.highest_sent, None);
        assert_eq!(queue.enqueue(damage(2)).expect("fresh event"), 0);
    }
}
