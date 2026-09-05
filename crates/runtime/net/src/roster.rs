//! The battle roster (protocol v51, interface program W-1): every hull in the battle named
//! WITHOUT a position. The snapshot filter strips unseen enemies, so a team list or a frag
//! counter built from snapshots alone would show a team of three; the roster is the manifest —
//! vehicle, team, seat, crew kind — and it never locates anyone.

use game_core::{TankId, TeamId, VehicleKind};
use serde::{Deserialize, Serialize};
use sim::TankState;

/// Who sits in a hull. The wire carries no names (nothing identifies a player on the
/// protocol yet); the seat letter is the hull's handle until Steam identity lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CrewKind {
    #[default]
    Bot,
    Human,
}

/// One roster line. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterEntry {
    pub tank_id: TankId,
    pub team: TeamId,
    pub vehicle: VehicleKind,
    /// The seat within the team, in spawn order: `0` is seat A.
    pub seat: u8,
    pub crew_kind: CrewKind,
}

impl RosterEntry {
    /// The seat as the HUD prints it: `A` for seat 0, `B` for 1, … — „T-54 · C".
    pub fn seat_letter(&self) -> char {
        char::from(b'A' + self.seat.min(25))
    }
}

/// The roster of a live board: every hull in `tanks` order, seated within its team in that
/// order, humans where `human_tanks` says.
pub fn roster_from_tanks(tanks: &[TankState], human_tanks: &[TankId]) -> Vec<RosterEntry> {
    let mut seats_by_team: Vec<(TeamId, u8)> = Vec::new();
    tanks
        .iter()
        .map(|tank| {
            let seat = match seats_by_team.iter_mut().find(|(team, _)| *team == tank.team) {
                Some((_, next)) => {
                    let seat = *next;
                    *next += 1;
                    seat
                }
                None => {
                    seats_by_team.push((tank.team, 1));
                    0
                }
            };
            RosterEntry {
                tank_id: tank.id,
                team: tank.team,
                vehicle: tank.spec.kind,
                seat,
                crew_kind: if human_tanks.contains(&tank.id) {
                    CrewKind::Human
                } else {
                    CrewKind::Bot
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seats_count_within_a_team_and_print_as_letters() {
        let mut state = sim::SimulationState::new();
        let a = state.spawn_tank(TeamId(1), game_core::TankSpec::t54_1951(), glam::Vec3::ZERO);
        let b = state.spawn_tank(TeamId(2), game_core::TankSpec::t54_1951(), glam::Vec3::X);
        let c = state.spawn_tank(TeamId(1), game_core::TankSpec::t54_1951(), glam::Vec3::Z);
        let roster = roster_from_tanks(state.tanks(), &[a]);
        let seat = |id: TankId| roster.iter().find(|entry| entry.tank_id == id).expect("entry");
        assert_eq!(
            (seat(a).seat, seat(a).seat_letter(), seat(a).crew_kind),
            (0, 'A', CrewKind::Human)
        );
        assert_eq!(
            (seat(b).seat, seat(b).seat_letter(), seat(b).crew_kind),
            (0, 'A', CrewKind::Bot)
        );
        assert_eq!((seat(c).seat, seat(c).seat_letter()), (1, 'B'));
        assert_eq!(roster.len(), 3);
    }
}
