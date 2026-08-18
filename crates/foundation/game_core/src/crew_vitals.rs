//! Battle wounds for the five-man roster — the half of the crew model that [`crate::crew`]
//! deliberately is not.
//!
//! `Crew` is progression state and stays PINNED at 1.0 ("progression is proof, never power").
//! `CrewVitals` is battle state: a penetration crossing a crew STATION (the seated-man capsules
//! in the damage layouts) knocks that man out. The rest of the crew covers his station at a
//! penalty while they give first aid; after [`CREW_FIRST_AID_S`] he comes back — WEAKENED, for
//! the rest of the battle. Deterministic end to end: no roll decides who is hit (the shell's
//! geometry does) and none decides recovery (the clock does).
//!
//! Crew loss NEVER kills the tank and never zeroes a function (user decision 2026-08-18): every
//! effect is a multiplier the driver plays through, except the radio net, which is binary by
//! nature — an unconscious operator keeps no net (see `net::snapshot_filter`).

use serde::{Deserialize, Serialize};

use crate::crew::CrewRole;

pub const CREW_ROLE_COUNT: usize = 5;

/// Seconds the rest of the crew needs to bring a knocked-out man back to his station. Sized
/// like the field repairs it mirrors (`sim::repair`): long enough that losing your loader is a
/// decision about the next half minute, short enough that it is a wound and not a sentence.
pub const CREW_FIRST_AID_S: f32 = 22.0;

/// A crewman back from first aid works at this fraction for the REST of the battle — the
/// permanent scar that makes a crew hit matter after the bandage.
pub const CREW_WEAKENED_EFFECTIVENESS: f32 = 0.85;

/// While a man is down, whoever covers his station works it at this fraction — the crew is one
/// pair of hands short and doing two jobs.
pub const CREW_COVERED_EFFECTIVENESS: f32 = 0.5;

/// The one crew→spec bridge for time-like values (reload, aim settle): a station at 50%
/// effectiveness takes twice as long, never infinitely long. Shared by the server and the
/// client predictor so both read the same battle.
pub fn crew_time_multiplier(effectiveness: f32) -> f32 {
    1.0 / effectiveness.max(0.05)
}

impl CrewRole {
    /// Stable order for wire bitsets and the vitals array — [`CrewRole::ALL`] order.
    pub fn wire_index(self) -> usize {
        CrewRole::ALL.iter().position(|role| *role == self).expect("role listed in ALL")
    }

    pub fn mask_bit(self) -> u8 {
        1 << self.wire_index()
    }
}

/// One crewman's battle state. `Unconscious` carries how long he has been down; the first-aid
/// clock lives HERE (not in a parallel struct) so serializing a tank serializes the whole wound.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum CrewMemberState {
    #[default]
    Active,
    Unconscious {
        down_s: f32,
    },
    Weakened,
}

/// The five stations' battle states, in [`CrewRole::ALL`] order.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CrewVitals {
    states: [CrewMemberState; CREW_ROLE_COUNT],
}

impl CrewVitals {
    pub fn state(&self, role: CrewRole) -> CrewMemberState {
        self.states[role.wire_index()]
    }

    /// Knock a crewman out. A re-hit while he is down restarts the clock (the wound is fresh);
    /// hitting a weakened man puts him down again — there is no third state below Weakened, so
    /// wounds past the first cost the same bandage time and the same scar.
    pub fn knock(&mut self, role: CrewRole) {
        self.states[role.wire_index()] = CrewMemberState::Unconscious { down_s: 0.0 };
    }

    /// One tick of first aid across the roster: every downed man's clock runs, and at
    /// [`CREW_FIRST_AID_S`] he returns weakened. Deterministic, server-authoritative; called in
    /// the same per-tank pass as module field repair.
    pub fn step_first_aid(&mut self, dt: f32) {
        for state in &mut self.states {
            if let CrewMemberState::Unconscious { down_s } = state {
                *down_s += dt;
                if *down_s >= CREW_FIRST_AID_S {
                    *state = CrewMemberState::Weakened;
                }
            }
        }
    }

    /// How well this role's station works right now: 1.0 whole, covered while the man is down,
    /// scarred once he is back.
    pub fn effectiveness(&self, role: CrewRole) -> f32 {
        match self.state(role) {
            CrewMemberState::Active => 1.0,
            CrewMemberState::Unconscious { .. } => CREW_COVERED_EFFECTIVENESS,
            CrewMemberState::Weakened => CREW_WEAKENED_EFFECTIVENESS,
        }
    }

    /// Bit `i` ([`CrewRole::ALL`] order) set while that man is down.
    pub fn unconscious_mask(&self) -> u8 {
        let mut mask = 0;
        for role in CrewRole::ALL {
            if matches!(self.state(role), CrewMemberState::Unconscious { .. }) {
                mask |= role.mask_bit();
            }
        }
        mask
    }

    /// Bit `i` set once that man is back but scarred.
    pub fn weakened_mask(&self) -> u8 {
        let mut mask = 0;
        for role in CrewRole::ALL {
            if self.state(role) == CrewMemberState::Weakened {
                mask |= role.mask_bit();
            }
        }
        mask
    }

    /// Seconds of first aid left per role, `None` for a man not down — the HUD countdown.
    pub fn down_remaining_s(&self) -> [Option<f32>; CREW_ROLE_COUNT] {
        self.states.map(|state| match state {
            CrewMemberState::Unconscious { down_s } => Some((CREW_FIRST_AID_S - down_s).max(0.0)),
            _ => None,
        })
    }

    /// Rebuild vitals from the replicated form — the client-side mirror of the three snapshot
    /// fields. A remaining countdown maps back to elapsed time so prediction keeps ticking it.
    pub fn from_wire(
        unconscious_mask: u8,
        weakened_mask: u8,
        down_remaining_s: [Option<f32>; CREW_ROLE_COUNT],
    ) -> Self {
        let mut vitals = Self::default();
        for role in CrewRole::ALL {
            let index = role.wire_index();
            vitals.states[index] = if unconscious_mask & role.mask_bit() != 0 {
                let remaining = down_remaining_s[index].unwrap_or(CREW_FIRST_AID_S);
                CrewMemberState::Unconscious { down_s: (CREW_FIRST_AID_S - remaining).max(0.0) }
            } else if weakened_mask & role.mask_bit() != 0 {
                CrewMemberState::Weakened
            } else {
                CrewMemberState::Active
            };
        }
        vitals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_knock_runs_the_first_aid_clock_and_leaves_a_scar() {
        let mut vitals = CrewVitals::default();
        assert_eq!(vitals.effectiveness(CrewRole::Loader), 1.0);

        vitals.knock(CrewRole::Loader);
        assert_eq!(vitals.effectiveness(CrewRole::Loader), CREW_COVERED_EFFECTIVENESS);
        assert_eq!(vitals.unconscious_mask(), CrewRole::Loader.mask_bit());

        // The bandage takes the full window...
        vitals.step_first_aid(CREW_FIRST_AID_S - 0.5);
        assert_eq!(vitals.unconscious_mask(), CrewRole::Loader.mask_bit(), "still down");
        // ...and then the man is back, scarred for the rest of the battle.
        vitals.step_first_aid(1.0);
        assert_eq!(vitals.unconscious_mask(), 0);
        assert_eq!(vitals.weakened_mask(), CrewRole::Loader.mask_bit());
        assert_eq!(vitals.effectiveness(CrewRole::Loader), CREW_WEAKENED_EFFECTIVENESS);
    }

    #[test]
    fn a_rehit_restarts_the_clock_and_a_weakened_man_goes_down_again() {
        let mut vitals = CrewVitals::default();
        vitals.knock(CrewRole::Gunner);
        vitals.step_first_aid(20.0);
        // Hit again two seconds before the bandage: the wound is fresh, the clock restarts.
        vitals.knock(CrewRole::Gunner);
        vitals.step_first_aid(20.0);
        assert_eq!(vitals.unconscious_mask(), CrewRole::Gunner.mask_bit(), "clock restarted");
        vitals.step_first_aid(3.0);
        assert_eq!(vitals.weakened_mask(), CrewRole::Gunner.mask_bit());

        // A weakened man is not immune — he goes down again, and comes back weakened again.
        vitals.knock(CrewRole::Gunner);
        assert_eq!(vitals.effectiveness(CrewRole::Gunner), CREW_COVERED_EFFECTIVENESS);
        vitals.step_first_aid(CREW_FIRST_AID_S);
        assert_eq!(vitals.effectiveness(CrewRole::Gunner), CREW_WEAKENED_EFFECTIVENESS);
    }

    #[test]
    fn the_wire_round_trip_preserves_every_state_and_the_countdown() {
        let mut vitals = CrewVitals::default();
        vitals.knock(CrewRole::Driver);
        vitals.step_first_aid(8.0);
        vitals.knock(CrewRole::Commander);
        vitals.step_first_aid(0.0);
        // Weakened radio operator via a full recovery.
        vitals.knock(CrewRole::RadioOperator);
        {
            // Only the radio operator may finish: rebuild others' wounds after stepping.
            let mut solo = CrewVitals::default();
            solo.knock(CrewRole::RadioOperator);
            solo.step_first_aid(CREW_FIRST_AID_S);
            vitals.states[CrewRole::RadioOperator.wire_index()] =
                solo.state(CrewRole::RadioOperator);
        }

        let rebuilt = CrewVitals::from_wire(
            vitals.unconscious_mask(),
            vitals.weakened_mask(),
            vitals.down_remaining_s(),
        );
        for role in CrewRole::ALL {
            assert_eq!(
                rebuilt.effectiveness(role),
                vitals.effectiveness(role),
                "{role:?} effectiveness survives the wire"
            );
        }
        let down = rebuilt.down_remaining_s()[CrewRole::Driver.wire_index()]
            .expect("driver still bandaging");
        assert!((down - (CREW_FIRST_AID_S - 8.0)).abs() < 1.0e-3);
    }

    #[test]
    fn time_multiplier_doubles_at_half_effectiveness_and_never_diverges() {
        assert_eq!(crew_time_multiplier(1.0), 1.0);
        assert_eq!(crew_time_multiplier(0.5), 2.0);
        assert!(crew_time_multiplier(0.0) <= 20.0, "floor keeps the value finite");
    }
}
