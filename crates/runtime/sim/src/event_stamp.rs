use game_core::{BattleEventId, DamageEvent, ShellImpact};

/// Per-tick view over the battle's one monotonic event-id space.
///
/// The durable counter lives in `SimulationState`; this short-lived sequencer makes producers
/// stamp events at emission time, preserving causal ordering across the separate damage and
/// impact vectors.
pub(crate) struct BattleEventStamp {
    last_event_id: BattleEventId,
    tick: u64,
}

impl BattleEventStamp {
    pub(crate) fn new(last_event_id: BattleEventId, tick: u64) -> Self {
        Self { last_event_id, tick }
    }

    pub(crate) fn last_event_id(&self) -> BattleEventId {
        self.last_event_id
    }

    pub(crate) fn push_damage(&mut self, events: &mut Vec<DamageEvent>, mut event: DamageEvent) {
        event.event_id = self.next_id();
        event.occurred_tick = self.tick;
        events.push(event);
    }

    pub(crate) fn push_impact(&mut self, impacts: &mut Vec<ShellImpact>, mut impact: ShellImpact) {
        impact.event_id = self.next_id();
        impact.occurred_tick = self.tick;
        impacts.push(impact);
    }

    fn next_id(&mut self) -> BattleEventId {
        let next = self
            .last_event_id
            .0
            .checked_add(1)
            .expect("authoritative battle event id space exhausted");
        self.last_event_id = BattleEventId(next);
        self.last_event_id
    }
}

/// The shell pipeline's two replicated output vectors plus their shared sequencer.
pub(crate) struct BattleEventOutput<'a> {
    damage_events: &'a mut Vec<DamageEvent>,
    shell_impacts: &'a mut Vec<ShellImpact>,
    stamp: &'a mut BattleEventStamp,
}

impl<'a> BattleEventOutput<'a> {
    pub(crate) fn new(
        damage_events: &'a mut Vec<DamageEvent>,
        shell_impacts: &'a mut Vec<ShellImpact>,
        stamp: &'a mut BattleEventStamp,
    ) -> Self {
        Self { damage_events, shell_impacts, stamp }
    }

    pub(crate) fn push_damage(&mut self, event: DamageEvent) {
        self.stamp.push_damage(self.damage_events, event);
    }

    pub(crate) fn push_impact(&mut self, impact: ShellImpact) {
        self.stamp.push_impact(self.shell_impacts, impact);
    }
}
