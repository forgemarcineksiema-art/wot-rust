//! The matchmaker's pure core (`docs/game-modes.md` M7a, rules R2–R5): tickets × now → battle
//! plans. No socket, no clock of its own, no randomness — the same tickets at the same instant
//! deal the same battles on every run and every machine, which is what a lock can hold and what
//! the coordinator (M7b) will call once a crew can be handed a host.
//!
//! The rules it keeps, each with a test at the bottom:
//! - one queue per (format, band): a battle is anchored on its OLDEST ticket and admits only
//!   crews within `VehicleKind::MATCHMAKING_SPREAD` of the anchor's tier — the band never
//!   widens, whatever the wait (R3);
//! - a battle starts the moment both sides are full of crews, or when the anchor has waited the
//!   format's fill deadline — then every empty seat is a bot (R3);
//! - crews are dealt across the two sides in a snake by rating, or by arrival until a rating
//!   exists, so the sides differ by at most one crew (R4);
//! - bots mirror the crews' tiers: the sides' tier histograms are made equal wherever the empty
//!   seats allow, and the remaining seats take the anchor's tier (R5).

use game_core::{BattleFormat, VehicleKind};

/// A crew in the queue. Until identity lands (M8) the id is whatever the coordinator hands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrewId(pub u64);

/// One crew's wish: the vehicle it will drive, the format it chose, its rating when one exists,
/// and the instant it entered the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    pub crew: CrewId,
    pub vehicle: VehicleKind,
    pub format: BattleFormat,
    /// `None` until M8: the snake then falls back to arrival order.
    pub rating: Option<u32>,
    pub entered_at_ms: u64,
}

impl Ticket {
    pub fn tier(&self) -> u8 {
        self.vehicle.tier()
    }
}

/// One seat of one side of a planned battle: a crew, or a bot the host draws at this tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seat {
    Crew(CrewId),
    Bot { tier: u8 },
}

/// Why the battle started now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Both sides full of crews.
    Full,
    /// The oldest ticket reached the format's fill deadline; the bots fill the rest.
    Deadline,
}

/// A battle ready to be hosted: every seat of both sides named, crews first, bots after — the
/// order the host seats them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlePlan {
    pub format: BattleFormat,
    pub anchor_tier: u8,
    pub start: Start,
    pub teams: [Vec<Seat>; 2],
}

impl BattlePlan {
    pub fn crews(&self) -> impl Iterator<Item = CrewId> + '_ {
        self.teams.iter().flatten().filter_map(|seat| match seat {
            Seat::Crew(crew) => Some(*crew),
            Seat::Bot { .. } => None,
        })
    }

    /// How many seats of each side a crew holds.
    pub fn crews_per_side(&self) -> [usize; 2] {
        [0, 1].map(|side| {
            self.teams[side].iter().filter(|seat| matches!(seat, Seat::Crew(_))).count()
        })
    }

    /// The tier of every seat on a side, sorted — the histogram R5 mirrors.
    pub fn tiers_of_side(&self, side: usize, tickets: &[Ticket]) -> Vec<u8> {
        let mut tiers: Vec<u8> = self.teams[side]
            .iter()
            .map(|seat| match seat {
                Seat::Crew(crew) => tickets
                    .iter()
                    .find(|ticket| ticket.crew == *crew)
                    .map(Ticket::tier)
                    .unwrap_or(self.anchor_tier),
                Seat::Bot { tier } => *tier,
            })
            .collect();
        tiers.sort_unstable();
        tiers
    }
}

/// The outcome of one deal: the battles that start now and the crews that keep waiting.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Deal {
    pub battles: Vec<BattlePlan>,
    pub waiting: Vec<CrewId>,
}

/// Deal the queue at `now_ms`. Pure: the tickets' order does not matter, the wall clock is an
/// argument, and a crew appears in at most one battle.
pub fn deal(tickets: &[Ticket], now_ms: u64) -> Deal {
    let mut result = Deal::default();
    for format in BattleFormat::ALL {
        let mut pool: Vec<Ticket> =
            tickets.iter().copied().filter(|ticket| ticket.format == format).collect();
        // Oldest first, the crew id as the tie-break: arrival order is the queue's one order.
        pool.sort_by_key(|ticket| (ticket.entered_at_ms, ticket.crew));
        let deadline_ms = u64::from(format.fill_deadline_s()) * 1_000;

        let mut index = 0;
        while index < pool.len() {
            let anchor = pool[index];
            let anchor_tier = anchor.tier();
            let in_band = |ticket: &Ticket| {
                ticket.tier().abs_diff(anchor_tier) <= VehicleKind::MATCHMAKING_SPREAD
            };
            let candidates: Vec<usize> = (index..pool.len())
                .filter(|&at| in_band(&pool[at]))
                .take(format.total_seats())
                .collect();
            let full = candidates.len() == format.total_seats();
            let overdue = now_ms.saturating_sub(anchor.entered_at_ms) >= deadline_ms;
            if !(full || overdue) {
                // The anchor keeps waiting; a younger crew may still anchor a battle of its own.
                result.waiting.push(anchor.crew);
                index += 1;
                continue;
            }
            let start = if full { Start::Full } else { Start::Deadline };
            let crews: Vec<Ticket> = candidates.iter().map(|&at| pool[at]).collect();
            result.battles.push(plan(format, anchor_tier, start, &crews));
            // Dealt crews leave the pool; the others move up.
            let mut taken = candidates.into_iter().peekable();
            let mut kept = Vec::with_capacity(pool.len());
            for (at, ticket) in pool.into_iter().enumerate() {
                if taken.peek() == Some(&at) {
                    taken.next();
                } else {
                    kept.push(ticket);
                }
            }
            pool = kept;
            // `index` still points at the first ticket not yet judged: the anchor and every
            // candidate after it left, and whatever waited before it is already in `waiting`.
        }
    }
    result
}

/// One battle from its crews: the snake across the sides (R4), then the bots mirroring the
/// crews' tiers (R5).
fn plan(format: BattleFormat, anchor_tier: u8, start: Start, crews: &[Ticket]) -> BattlePlan {
    let seats = format.seats_per_team();
    // The snake's order: the best-rated first; without ratings, the earliest.
    let mut ranked: Vec<Ticket> = crews.to_vec();
    ranked.sort_by_key(|ticket| {
        (
            ticket.rating.map_or(u32::MAX, |rating| u32::MAX - rating),
            ticket.entered_at_ms,
            ticket.crew,
        )
    });
    let mut teams: [Vec<Seat>; 2] = [Vec::with_capacity(seats), Vec::with_capacity(seats)];
    let mut tiers: [Vec<u8>; 2] = [Vec::new(), Vec::new()];
    for (rank, ticket) in ranked.iter().enumerate() {
        let side = BattleFormat::snake_side(rank);
        // A side that is full (only possible when the crews outnumber the seats, which the
        // caller never asks for) spills to the other.
        let side = if teams[side].len() < seats { side } else { 1 - side };
        teams[side].push(Seat::Crew(ticket.crew));
        tiers[side].push(ticket.tier());
    }
    // R5: where the empty seats allow, give each side the tiers the other side's crews have
    // and it lacks; then the anchor's tier for whatever is left.
    let low = anchor_tier.saturating_sub(VehicleKind::MATCHMAKING_SPREAD);
    let high = anchor_tier.saturating_add(VehicleKind::MATCHMAKING_SPREAD);
    for side in 0..2 {
        let other = 1 - side;
        for tier in low..=high {
            let mine = tiers[side].iter().filter(|t| **t == tier).count();
            let theirs = tiers[other].iter().filter(|t| **t == tier).count();
            for _ in mine..theirs {
                if teams[side].len() >= seats {
                    break;
                }
                teams[side].push(Seat::Bot { tier });
                tiers[side].push(tier);
            }
        }
    }
    for side in 0..2 {
        while teams[side].len() < seats {
            teams[side].push(Seat::Bot { tier: anchor_tier });
            tiers[side].push(anchor_tier);
        }
    }
    BattlePlan { format, anchor_tier, start, teams }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket(crew: u64, vehicle: VehicleKind, format: BattleFormat, entered_at_ms: u64) -> Ticket {
        Ticket { crew: CrewId(crew), vehicle, format, rating: None, entered_at_ms }
    }

    /// A tier-7 crew waits alone; a tier-9 crew (two tiers away) never joins it — not before
    /// the deadline and not after; at the deadline the battle starts with bots of the band.
    #[test]
    fn the_band_never_widens_and_the_deadline_always_starts() {
        let seven = ticket(1, VehicleKind::TigerI, BattleFormat::SevenVsSeven, 0);
        let nine = ticket(2, VehicleKind::T54_1951, BattleFormat::SevenVsSeven, 0);
        let deadline_ms = u64::from(BattleFormat::SevenVsSeven.fill_deadline_s()) * 1_000;

        let early = deal(&[seven, nine], deadline_ms - 1);
        assert!(early.battles.is_empty(), "nobody starts before the deadline");
        assert_eq!(early.waiting, vec![CrewId(1), CrewId(2)]);

        let due = deal(&[seven, nine], deadline_ms);
        assert_eq!(due.battles.len(), 2, "each crew anchors its own battle: the band never widens");
        for battle in &due.battles {
            assert_eq!(battle.start, Start::Deadline);
            assert_eq!(battle.crews().count(), 1);
            for side in 0..2 {
                assert_eq!(battle.teams[side].len(), 7);
                for tier in battle.tiers_of_side(side, &[seven, nine]) {
                    assert!(tier.abs_diff(battle.anchor_tier) <= VehicleKind::MATCHMAKING_SPREAD);
                }
            }
        }
        assert!(due.waiting.is_empty());
    }

    /// Five crews at the deadline: three and two across the sides (the snake), every crew
    /// seated before any bot, and the sides' tier histograms equal once the bots are dealt.
    #[test]
    fn humans_split_evenly_and_bots_mirror_the_tier_histogram() {
        let tickets = [
            ticket(10, VehicleKind::TigerI, BattleFormat::SevenVsSeven, 0), // 7
            ticket(11, VehicleKind::IS3, BattleFormat::SevenVsSeven, 1),    // 8
            ticket(12, VehicleKind::TigerI, BattleFormat::SevenVsSeven, 2), // 7
            ticket(13, VehicleKind::T34_85, BattleFormat::SevenVsSeven, 3), // 6
            ticket(14, VehicleKind::Centurion, BattleFormat::SevenVsSeven, 4), // 8
        ];
        let dealt = deal(&tickets, 30_000);
        assert_eq!(dealt.battles.len(), 1);
        let battle = &dealt.battles[0];
        assert_eq!(battle.crews_per_side(), [3, 2]);
        for side in 0..2 {
            let crews = battle.crews_per_side()[side];
            assert!(
                battle.teams[side][..crews].iter().all(|seat| matches!(seat, Seat::Crew(_))),
                "crews take a side's first seats"
            );
            assert_eq!(battle.teams[side].len(), 7);
        }
        assert_eq!(
            battle.tiers_of_side(0, &tickets),
            battle.tiers_of_side(1, &tickets),
            "the bots mirror the crews' tiers"
        );
        // The snake by arrival: crews 10 and 13 on side one, 11 and 12 on side two, 14 on one.
        assert_eq!(
            battle.teams[0][..3],
            [Seat::Crew(CrewId(10)), Seat::Crew(CrewId(13)), Seat::Crew(CrewId(14))]
        );
        assert_eq!(battle.teams[1][..2], [Seat::Crew(CrewId(11)), Seat::Crew(CrewId(12))]);
    }

    /// The order the tickets arrive in the slice is nobody's business: shuffled, the deal is
    /// the same plan, seat for seat.
    #[test]
    fn the_same_tickets_deal_the_same_battle() {
        let roster = [
            VehicleKind::TigerI,
            VehicleKind::IS3,
            VehicleKind::PantherII,
            VehicleKind::TigerII,
            VehicleKind::Centurion,
            VehicleKind::Jagdtiger,
            VehicleKind::T54_1951,
        ];
        let tickets: Vec<Ticket> = (0..9)
            .map(|i| {
                ticket(
                    100 + i,
                    roster[i as usize % roster.len()],
                    BattleFormat::FifteenVsFifteen,
                    i * 7,
                )
            })
            .collect();
        let straight = deal(&tickets, 60_000);
        let mut shuffled = tickets.clone();
        shuffled.rotate_left(4);
        shuffled.swap(0, 5);
        assert_eq!(deal(&shuffled, 60_000), straight);
        assert_eq!(straight.battles.len(), 1);
        assert_eq!(straight.battles[0].teams[0].len(), 15);
    }

    /// Both sides full of crews start at once, and the largest format needs all thirty.
    #[test]
    fn a_full_queue_starts_at_once_and_a_crew_is_dealt_once() {
        let fourteen: Vec<Ticket> = (0..14)
            .map(|i| ticket(i, VehicleKind::TigerII, BattleFormat::SevenVsSeven, 0))
            .collect();
        let dealt = deal(&fourteen, 0);
        assert_eq!(dealt.battles.len(), 1);
        assert_eq!(dealt.battles[0].start, Start::Full);
        assert_eq!(dealt.battles[0].crews_per_side(), [7, 7]);
        let mut crews: Vec<CrewId> = dealt.battles[0].crews().collect();
        crews.sort();
        crews.dedup();
        assert_eq!(crews.len(), 14, "every crew once");

        let fifteen: Vec<Ticket> = (0..15)
            .map(|i| ticket(i, VehicleKind::TigerII, BattleFormat::FifteenVsFifteen, 0))
            .collect();
        assert!(deal(&fifteen, 0).battles.is_empty(), "fifteen crews do not fill 15v15");
        let thirty: Vec<Ticket> = (0..30)
            .map(|i| ticket(i, VehicleKind::TigerII, BattleFormat::FifteenVsFifteen, 0))
            .collect();
        assert_eq!(deal(&thirty, 0).battles[0].start, Start::Full);
    }

    /// With ratings the snake runs by rating: the best crew opens side one, the next two go to
    /// side two — so the sides' rating sums stay close.
    #[test]
    fn with_ratings_the_snake_deals_by_rating() {
        let mut tickets: Vec<Ticket> = (0..4)
            .map(|i| ticket(i, VehicleKind::TigerII, BattleFormat::SevenVsSeven, 3 - i))
            .collect();
        for (ticket, rating) in tickets.iter_mut().zip([1_500, 1_900, 1_700, 1_300]) {
            ticket.rating = Some(rating);
        }
        let dealt = deal(&tickets, 30_000);
        let battle = &dealt.battles[0];
        assert_eq!(battle.teams[0][..2], [Seat::Crew(CrewId(1)), Seat::Crew(CrewId(3))]);
        assert_eq!(battle.teams[1][..2], [Seat::Crew(CrewId(2)), Seat::Crew(CrewId(0))]);
    }
}
