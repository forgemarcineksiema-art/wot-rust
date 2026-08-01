use std::time::{SystemTime, UNIX_EPOCH};

use game_core::{TeamId, VehicleKind};
use sim::TankState;
use terrain::MapId;

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

    pub(crate) fn random_battle_u64(self, salt: u64) -> u64 {
        self.random_battle_mix(salt)
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
    /// Which world this battle happens on. Server and client both regenerate the map from
    /// this id — see [`terrain::MapId`].
    pub map: MapId,
}

impl RandomBattleConfig {
    pub fn new(seed: BattleSeed, player_vehicle: VehicleKind) -> Self {
        Self { seed, player_vehicle, map: MapId::default() }
    }

    pub fn runtime(player_vehicle: VehicleKind) -> Self {
        Self::new(BattleSeed::runtime(), player_vehicle)
    }

    /// Runtime config honoring the `WOT_MAP` env override: a map slug (e.g. "bystra-valley")
    /// — or, for the editor's playtest loop (map-editor D2), a path to a `.map.ron`
    /// blueprint, which installs the document as `MapId::Scratch` for this process. This is
    /// the opt-in gate for maps not in the rotation: unset, unknown or unreadable values
    /// keep the default map, so nothing changes for anyone who has not asked.
    pub fn runtime_from_env(player_vehicle: VehicleKind) -> Self {
        let map = std::env::var("WOT_MAP")
            .ok()
            .and_then(|value| {
                let value = value.trim();
                if value.ends_with(".map.ron") {
                    let source = std::fs::read_to_string(value)
                        .map_err(|error| eprintln!("WOT_MAP: cannot read {value}: {error}"))
                        .ok()?;
                    map_forge::set_scratch_source(source)
                        .map_err(|error| eprintln!("WOT_MAP: {value}: {error}"))
                        .ok()?;
                    return Some(MapId::Scratch);
                }
                MapId::from_slug(value)
            })
            .unwrap_or_default();
        Self::runtime(player_vehicle).on_map(map)
    }

    pub fn on_map(mut self, map: MapId) -> Self {
        self.map = map;
        self
    }
}

/// Time limit for a random battle, the safety net that guarantees every battle ends: two
/// last-tank survivors camping opposite corners (or two stuck bots) must not hang the battle
/// forever. Ten minutes fits the 7v7 scale — real fights resolve well under it.
pub const RANDOM_BATTLE_TIME_LIMIT_S: u32 = 600;

/// Why a battle ended without a winner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawReason {
    /// The last tanks of both teams died on the same tick (shell crossfire, mutual ram).
    MutualElimination,
    /// The battle clock ran out with both teams still alive.
    TimeExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleOutcome {
    TeamEliminated {
        winning_team: TeamId,
    },
    /// The clock ran out with one team holding more tanks than the other. Not an elimination — a
    /// decision on the board, and the side that won the fight is told so.
    TeamAhead {
        winning_team: TeamId,
    },
    Draw {
        reason: DrawReason,
    },
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
        } else if alive.is_empty() && eliminated > 0 {
            Some(Self::Draw { reason: DrawReason::MutualElimination })
        } else {
            None
        }
    }

    pub(crate) fn from_tanks(tanks: &[TankState]) -> Option<Self> {
        Self::from_team_alive_counts(alive_by_team(tanks))
    }

    /// The verdict when the clock runs out: **the board decides.**
    ///
    /// A measured 7v7 ended `Draw { TimeExpired }` with one team holding four tanks against the
    /// other's one — a fourfold advantage called even. That is the least satisfying resolution
    /// available and it punishes the side that won the fight, so the timer now awards the battle
    /// to whoever is ahead on hulls. Only a genuine tie is a draw.
    ///
    /// Tanks remaining is the whole tiebreak, deliberately: it is the one quantity every player can
    /// read off the scoreboard mid-battle and play toward. A damage-dealt tiebreak would be fairer
    /// on paper and unreadable in the moment.
    pub fn from_time_expiry<I>(counts: I) -> Self
    where
        I: IntoIterator<Item = (TeamId, usize)>,
    {
        let mut best: Option<(TeamId, usize)> = None;
        let mut tied = false;
        for (team, alive) in counts {
            match best {
                Some((_, most)) if alive > most => {
                    best = Some((team, alive));
                    tied = false;
                }
                Some((_, most)) if alive == most => tied = true,
                Some(_) => {}
                None => best = Some((team, alive)),
            }
        }
        match best {
            Some((winning_team, alive)) if !tied && alive > 0 => Self::TeamAhead { winning_team },
            _ => Self::Draw { reason: DrawReason::TimeExpired },
        }
    }

    pub(crate) fn from_tanks_at_time_expiry(tanks: &[TankState]) -> Self {
        Self::from_time_expiry(alive_by_team(tanks))
    }

    /// The victor, or `None` for a draw.
    pub fn winning_team(self) -> Option<TeamId> {
        match self {
            Self::TeamEliminated { winning_team } | Self::TeamAhead { winning_team } => {
                Some(winning_team)
            }
            Self::Draw { .. } => None,
        }
    }
}

/// Living hulls per team, in first-seen order. Shared by the elimination check and the timer
/// verdict so the two can never count the board differently.
fn alive_by_team(tanks: &[TankState]) -> Vec<(TeamId, usize)> {
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
    counts
}
