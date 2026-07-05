use std::time::{SystemTime, UNIX_EPOCH};

use game_core::{TeamId, VehicleKind};
use sim::TankState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMode {
    PracticeDuel,
    Random7v7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleSeed(u64);

impl BattleSeed {
    pub fn fixed(value: u64) -> Self {
        Self(value)
    }

    pub fn runtime() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos() as u64);
        Self(nanos ^ (std::process::id() as u64).rotate_left(17))
    }

    fn random_battle_mix(self, salt: u64) -> u64 {
        let mut value = self.0.wrapping_add(salt).wrapping_add(0x9E37_79B9_7F4A_7C15);
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub(crate) fn random_battle_index(self, salt: u64, len: usize) -> usize {
        (self.random_battle_mix(salt) as usize) % len.max(1)
    }

    pub(crate) fn random_battle_unit(self, salt: u64) -> f32 {
        let bits = self.random_battle_mix(salt) >> 40;
        bits as f32 / ((1u64 << 24) - 1) as f32
    }

    pub(crate) fn route_seed(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomBattleConfig {
    pub seed: BattleSeed,
    pub player_vehicle: VehicleKind,
}

impl RandomBattleConfig {
    pub fn new(seed: BattleSeed, player_vehicle: VehicleKind) -> Self {
        Self { seed, player_vehicle }
    }

    pub fn runtime(player_vehicle: VehicleKind) -> Self {
        Self::new(BattleSeed::runtime(), player_vehicle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleOutcome {
    TeamEliminated { winning_team: TeamId },
}

impl BattleOutcome {
    pub fn from_team_alive_counts<I>(counts: I) -> Option<Self>
    where
        I: IntoIterator<Item = (TeamId, usize)>,
    {
        let mut alive = Vec::new();
        let mut eliminated = 0usize;
        for (team, count) in counts {
            if count > 0 {
                alive.push(team);
            } else {
                eliminated += 1;
            }
        }
        if alive.len() == 1 && eliminated > 0 {
            Some(Self::TeamEliminated { winning_team: alive[0] })
        } else {
            None
        }
    }

    pub(crate) fn from_tanks(tanks: &[TankState]) -> Option<Self> {
        let mut counts: Vec<(TeamId, usize)> = Vec::new();
        for tank in tanks {
            if let Some((_, count)) = counts.iter_mut().find(|(team, _)| *team == tank.team) {
                if tank.hit_points > 0 {
                    *count += 1;
                }
            } else {
                counts.push((tank.team, usize::from(tank.hit_points > 0)));
            }
        }
        Self::from_team_alive_counts(counts)
    }

    pub fn winning_team(self) -> TeamId {
        match self {
            Self::TeamEliminated { winning_team } => winning_team,
        }
    }
}
