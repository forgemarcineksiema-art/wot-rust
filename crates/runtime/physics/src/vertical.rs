//! Vertical resolution of the hull against the terrain. The ground either *carries* the hull —
//! the kinematic follow that keeps every drivable slope glued under the tracks — or lets go of
//! it: a hull that drives off a face steeper than the tracks can follow becomes a ballistic body
//! until the terrain catches it again. Deterministic and shared by the server and the client
//! predictor, like the rest of the planar rigid-body model.

use game_core::math::GRAVITY_MPS2;

use crate::movement::TankKinematicState;

/// Vertical clearance above the sampled ground below which the hull counts as grounded.
pub const AIRBORNE_CLEARANCE_M: f32 = 0.02;

/// Steepest downhill grade (rise/run) the tracks keep following before the hull leaves the
/// ground. Sits between tank gradeability (0.6 — so every drivable slope stays a ground follow)
/// and the railway embankment's steepest face (~0.75 — so driving off it at speed launches the
/// hull instead of teleporting it down).
pub const MAX_FOLLOW_GRADE: f32 = 0.68;

/// Absolute per-tick drop the hull always follows, so crawl-speed descents and float noise never
/// flicker into micro-flights.
pub const MIN_FOLLOW_DROP_M: f32 = 0.03;

/// The vertical outcome of one tick: whether the hull ended it carried by the ground, and the
/// downward speed the terrain absorbed if this tick ended a flight (`0.0` otherwise).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundStep {
    pub grounded: bool,
    pub landing_impact_mps: f32,
}

impl GroundStep {
    /// The resting outcome: on the ground, nothing absorbed.
    pub const fn resting() -> Self {
        Self { grounded: true, landing_impact_mps: 0.0 }
    }
}

/// True when the hull is close enough to the sampled ground to count as carried by it.
pub fn is_grounded(position_y: f32, ground_y: f32) -> bool {
    position_y <= ground_y + AIRBORNE_CLEARANCE_M
}

/// Resolve the hull's height against the ground under it for one tick. `moved_xz_m` is the
/// horizontal distance the hull covered this tick: the drop the ground may take before the hull
/// unsticks scales with it, which makes the follow/launch decision a pure grade threshold
/// independent of speed and tick rate.
pub fn resolve_vertical(
    state: &mut TankKinematicState,
    ground_y: f32,
    was_grounded: bool,
    moved_xz_m: f32,
    dt: f32,
) -> GroundStep {
    if was_grounded {
        // Rising ground always carries the hull (the old kinematic snap, spawn seeding
        // included); falling ground carries it as long as the drop reads as a followable slope.
        let drop = state.position.y - ground_y;
        let follow_limit = (moved_xz_m * MAX_FOLLOW_GRADE).max(MIN_FOLLOW_DROP_M);
        if drop <= follow_limit {
            state.position.y = ground_y;
            state.velocity.y = 0.0;
            return GroundStep::resting();
        }
    }
    // Ballistic: gravity is the only force in flight, until the terrain catches the hull.
    state.velocity.y -= GRAVITY_MPS2 * dt;
    state.position.y += state.velocity.y * dt;
    if state.position.y <= ground_y {
        let landing_impact_mps = (-state.velocity.y).max(0.0);
        state.position.y = ground_y;
        state.velocity.y = 0.0;
        return GroundStep { grounded: true, landing_impact_mps };
    }
    GroundStep { grounded: false, landing_impact_mps: 0.0 }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn state_at(y: f32) -> TankKinematicState {
        TankKinematicState { position: Vec3::new(0.0, y, 0.0), ..Default::default() }
    }

    #[test]
    fn a_followable_drop_stays_glued_and_a_cliff_lets_go() {
        // Gentle grade under fast travel: glued, no impact.
        let mut state = state_at(5.0);
        let step = resolve_vertical(&mut state, 4.95, true, 0.25, DT);
        assert!(step.grounded && step.landing_impact_mps == 0.0);
        assert_eq!(state.position.y, 4.95);
        // The same travel over a cliff-sized drop: airborne, gravity takes over.
        let mut state = state_at(5.0);
        let step = resolve_vertical(&mut state, 0.0, true, 0.25, DT);
        assert!(!step.grounded);
        assert!(state.velocity.y < 0.0 && state.position.y < 5.0 && state.position.y > 4.9);
    }

    #[test]
    fn a_flight_accelerates_downward_and_the_ground_catch_reports_the_impact() {
        let mut state = state_at(5.0);
        resolve_vertical(&mut state, 0.0, true, 0.25, DT);
        let mut last_fall = 0.0;
        let mut landing = None;
        for _ in 0..200 {
            let before = state.position.y;
            let step = resolve_vertical(&mut state, 0.0, false, 0.0, DT);
            if step.grounded {
                landing = Some(step.landing_impact_mps);
                break;
            }
            let fall = before - state.position.y;
            assert!(fall > last_fall, "ballistic fall must accelerate");
            last_fall = fall;
        }
        let impact = landing.expect("the terrain must catch the hull");
        // v = sqrt(2 g h) for a 5 m drop, discretization tolerance included.
        assert!((impact - (2.0 * GRAVITY_MPS2 * 5.0).sqrt()).abs() < 0.6, "impact {impact}");
        assert_eq!(state.position.y, 0.0);
        assert_eq!(state.velocity.y, 0.0);
    }

    #[test]
    fn a_stationary_hull_follows_small_settles_but_not_a_ledge() {
        let mut state = state_at(1.0);
        assert!(resolve_vertical(&mut state, 0.98, true, 0.0, DT).grounded);
        let mut state = state_at(1.0);
        assert!(!resolve_vertical(&mut state, 0.5, true, 0.0, DT).grounded);
    }
}
