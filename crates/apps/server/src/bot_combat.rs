//! The bots' combat brain — who a bot may engage and the stand-and-shoot command that fights
//! them. Split from `bots.rs` (roster, postures, unstuck) for the reviewability budget.
//!
//! Performance contract: target selection is the only place a bot fires its own LOS raycasts,
//! and it never allocates. Candidates are filtered by the cheap gates first (team, alive,
//! range, the authoritative team-spotting mask) and a raycast runs ONLY when a candidate is
//! nearer than the best visible target found so far — so the nearest-visible result is
//! identical to the old sorted walk, at a fraction of the raycasts and zero heap traffic.

use game_core::TankId;
use sim::{TankCommand, TankState, VIEW_RANGE_M};

use crate::bot_aim::BotFiringSolution;

/// The nearest enemy this bot may ENGAGE: team-spotted, in range, and in the bot's OWN line of
/// sight. The team mask alone says "someone on my team sees it" — a bot acting on just that parks
/// and shells the front of a hill (or a building) for the whole reload cycle. A bot with no
/// engageable target keeps driving its route instead of aiming at terrain.
pub(crate) fn bot_nearest_engageable_enemy<'a>(
    tank: &TankState,
    tanks: &'a [TankState],
    exclude: Option<TankId>,
    heightmap: Option<&terrain::HeightMap>,
    cover: &[terrain::StaticCoverObject],
) -> Option<&'a TankState> {
    let mut best: Option<(&TankState, f32)> = None;
    for target in tanks {
        if Some(target.id) == exclude
            || target.team == tank.team
            || target.hit_points == 0
            || target.spotted_mask & tank.team.spotting_bit() == 0
        {
            continue;
        }
        let d2 = tank.position.distance_squared(target.position);
        if d2 > VIEW_RANGE_M * VIEW_RANGE_M {
            continue;
        }
        // Only a candidate that would BEAT the best visible target earns a raycast.
        if best.is_some_and(|(_, best_d2)| d2 >= best_d2) {
            continue;
        }
        if sim::tank_line_of_sight(tank, target, heightmap, cover) {
            best = Some((target, d2));
        }
    }
    best.map(|(target, _)| target)
}

/// The cheap per-tick gate for a target chosen on an earlier tick: still an enemy worth the
/// gun (alive, team-spotted, in range). The bot's own LOS is re-verified on the next full
/// re-selection — a target that just broke line of sight stays aimed-at for a fraction of a
/// second, which is exactly how a human gunner tracks a tank ducking behind a crest.
pub(crate) fn bot_target_still_engageable(tank: &TankState, target: &TankState) -> bool {
    target.team != tank.team
        && target.hit_points > 0
        && target.spotted_mask & tank.team.spotting_bit() != 0
        && tank.position.distance_squared(target.position) <= VIEW_RANGE_M * VIEW_RANGE_M
}

pub(crate) fn find_tank(tanks: &[TankState], id: TankId) -> Option<&TankState> {
    tanks.iter().find(|tank| tank.id == id)
}

/// Stand and lay the gun on the cached ballistic firing solution; the trigger waits for the
/// lay to close inside the angle the target actually subtends at this range. The expensive
/// solve produced `solution` on its own cadence — this per-tick half is pure arithmetic.
pub(crate) fn bot_combat_command(tank: &TankState, solution: &BotFiringSolution) -> TankCommand {
    let aim = solution.errors(tank);
    TankCommand {
        throttle: 0.0,
        steer: 0.0,
        brake: 0.35,
        turret_yaw_delta: (aim.turret_error * 4.0).clamp(-1.0, 1.0),
        gun_pitch_delta: (aim.pitch_error * 4.0).clamp(-1.0, 1.0),
        fire: aim.on_target() && tank.reload_remaining_s <= 0.0,
        select_ammo: None,
    }
}

#[cfg(test)]
mod tests {
    use game_core::{TankId, TeamId};
    use glam::Vec3;

    use super::*;
    use crate::bots::test_support::tank_with_mask as tank;

    #[test]
    fn bots_target_only_enemies_spotted_by_their_team() {
        let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let unspotted_enemy =
            tank(2, TeamId(2), Vec3::new(305.0, 0.0, 305.0), TeamId(2).spotting_bit());

        assert!(
            bot_nearest_engageable_enemy(
                &observer,
                &[observer.clone(), unspotted_enemy],
                None,
                None,
                &[]
            )
            .is_none(),
            "bot AI must reuse the authoritative spotting mask instead of running a private LOS target"
        );
    }

    /// A team-spotted enemy is not necessarily in THIS bot's line of sight — acting on the mask
    /// alone, a bot parked and shelled the front of a hill for the rest of the battle. Locked
    /// here: a masked-but-occluded enemy is not engaged (the bot keeps driving its route), a
    /// farther enemy with a clear line wins over a nearer occluded one, and clearing the
    /// obstruction makes the nearest enemy the target again.
    #[test]
    fn bots_engage_only_enemies_in_their_own_line_of_sight() {
        let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mask = TeamId(2).spotting_bit() | TeamId(1).spotting_bit();
        let near_enemy = tank(2, TeamId(2), Vec3::new(300.0, 0.0, 360.0), mask);
        let far_enemy = tank(3, TeamId(2), Vec3::new(360.0, 0.0, 300.0), mask);
        let tanks = [observer.clone(), near_enemy, far_enemy];
        // A wall across the sight line to the near enemy only.
        let wall = terrain::StaticCoverObject {
            id: "wall".into(),
            name: "wall".into(),
            kind: terrain::StaticCoverKind::FarmBuilding,
            center: [300.0, 5.0, 330.0],
            half_extents_m: [20.0, 10.0, 2.0],
        };

        let blocked = bot_nearest_engageable_enemy(
            &observer,
            &tanks,
            None,
            None,
            std::slice::from_ref(&wall),
        );
        assert_eq!(
            blocked.map(|target| target.id),
            Some(TankId(3)),
            "a nearer but occluded enemy must fall through to a farther visible one"
        );

        let clear = bot_nearest_engageable_enemy(&observer, &tanks, None, None, &[]);
        assert_eq!(
            clear.map(|target| target.id),
            Some(TankId(2)),
            "with the line clear the nearest enemy is engaged"
        );
    }

    /// The cheap between-reselections gate: a dead, unspotted, or out-of-range target is
    /// dropped at once; a live spotted one in range is kept without any raycast.
    #[test]
    fn cached_targets_drop_the_moment_the_cheap_gates_fail() {
        let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mask = TeamId(2).spotting_bit() | TeamId(1).spotting_bit();
        let live = tank(2, TeamId(2), Vec3::new(320.0, 0.0, 300.0), mask);
        assert!(bot_target_still_engageable(&observer, &live));

        let mut dead = live.clone();
        dead.hit_points = 0;
        assert!(!bot_target_still_engageable(&observer, &dead));

        let mut unspotted = live.clone();
        unspotted.spotted_mask = TeamId(2).spotting_bit();
        assert!(!bot_target_still_engageable(&observer, &unspotted));

        let mut far = live.clone();
        far.position = Vec3::new(300.0 + VIEW_RANGE_M + 10.0, 0.0, 300.0);
        assert!(!bot_target_still_engageable(&observer, &far));
    }
}
