//! The bots' combat brain — who a bot may engage and the stand-and-shoot command that fights
//! them. Split from `bots.rs` (roster, postures, unstuck) for the reviewability budget.

use sim::{TankCommand, TankState, VIEW_RANGE_M};

/// The nearest enemy this bot may ENGAGE: team-spotted, in range, and in the bot's OWN line of
/// sight. The team mask alone says "someone on my team sees it" — a bot acting on just that parks
/// and shells the front of a hill (or a building) for the whole reload cycle. Candidates are
/// walked nearest-first so a closer but masked enemy falls through to a farther visible one, and
/// a bot with no engageable target keeps driving its route instead of aiming at terrain.
pub(crate) fn bot_nearest_engageable_enemy<'a>(
    tank: &TankState,
    tanks: &'a [TankState],
    heightmap: Option<&terrain::HeightMap>,
    cover: &[terrain::StaticCoverObject],
) -> Option<&'a TankState> {
    let mut candidates: Vec<&TankState> = tanks
        .iter()
        .filter(|target| {
            target.team != tank.team
                && target.hit_points > 0
                && target.position.distance(tank.position) <= VIEW_RANGE_M
                && target.spotted_mask & tank.team.spotting_bit() != 0
        })
        .collect();
    candidates.sort_by(|a, b| {
        tank.position
            .distance_squared(a.position)
            .total_cmp(&tank.position.distance_squared(b.position))
    });
    candidates.into_iter().find(|target| sim::tank_line_of_sight(tank, target, heightmap, cover))
}

/// Stand and lay the gun on the ballistic firing solution (`bot_aim`); the trigger waits for the
/// lay to close inside the angle the target actually subtends at this range.
pub(crate) fn bot_combat_command(tank: &TankState, target: &TankState) -> TankCommand {
    let aim = crate::bot_aim::bot_aim_solution(tank, target);
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

        let blocked =
            bot_nearest_engageable_enemy(&observer, &tanks, None, std::slice::from_ref(&wall));
        assert_eq!(
            blocked.map(|target| target.id),
            Some(TankId(3)),
            "a nearer but occluded enemy must fall through to a farther visible one"
        );

        let clear = bot_nearest_engageable_enemy(&observer, &tanks, None, &[]);
        assert_eq!(
            clear.map(|target| target.id),
            Some(TankId(2)),
            "with the line clear the nearest enemy is engaged"
        );
    }
}
