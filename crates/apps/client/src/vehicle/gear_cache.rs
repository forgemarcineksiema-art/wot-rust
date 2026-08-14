//! Running-gear placements, kept between frames because most of the time nothing moved them.
//!
//! Every tank on the field rebuilds its whole running gear every presented frame: 204 placements
//! for a T-54, 2,856 for a 7v7, each a `Mat4` built from scratch — 0.29 ms of frame, measured by
//! `probe -- battle_age_cost`, in fourteen freshly allocated `Vec`s dropped the same frame.
//!
//! Most of that work reproduces the previous frame exactly. The placements are HULL-LOCAL — the
//! world transform is applied afterwards — so they depend only on how far each track has scrolled,
//! how far each road wheel has travelled, how tight the top runs are, and whether a track is
//! thrown. A wreck moves none of those ever again, and a tank holding a firing position moves none
//! of them either. Late in a battle most of the roster is one or the other.
//!
//! So the placements are cached against exactly those inputs and rebuilt only when one of them
//! actually changes. The check is about twenty float comparisons against a stored pose; the work
//! it skips is 204 matrix constructions and an allocation. A live, driving tank misses every
//! frame and pays the comparison, which is noise beside the build it would have done anyway.
//!
//! The detail tier is deliberately NOT part of the key: the tier picks meshes, never placements
//! (`running_gear_objects.rs`), so a tank crossing a tier boundary keeps its cached gear.

use std::collections::HashMap;

use game_core::{TankId, VehicleKind};
use vehicle_geometry::{
    GearDynamics, GearPlacement, RunningGearKinematics, running_gear_placements_dynamic_into,
};

/// Everything the placements are a function of. Compared, never rebuilt, on the hit path.
#[derive(Debug, Default)]
struct GearPose {
    kind: Option<VehicleKind>,
    left_m: f32,
    right_m: f32,
    left_travel: Vec<f32>,
    right_travel: Vec<f32>,
    left_sag_scale: f32,
    right_sag_scale: f32,
    left_break_t: Option<f32>,
    right_break_t: Option<f32>,
}

impl GearPose {
    /// Bit-exact comparison, on purpose: the question is whether the SAME placements would come
    /// out, and identical inputs are the only honest answer to that. A NaN anywhere compares
    /// unequal and simply rebuilds, which is the safe direction.
    fn matches(
        &self,
        kind: VehicleKind,
        left_m: f32,
        right_m: f32,
        dynamics: &GearDynamics<'_>,
    ) -> bool {
        self.kind == Some(kind)
            && self.left_m == left_m
            && self.right_m == right_m
            && self.left_sag_scale == dynamics.left_sag_scale
            && self.right_sag_scale == dynamics.right_sag_scale
            && self.left_break_t == dynamics.left_break_t
            && self.right_break_t == dynamics.right_break_t
            && self.left_travel == dynamics.left_travel
            && self.right_travel == dynamics.right_travel
    }

    /// Record the pose these placements were built for. The two travel buffers are cleared and
    /// refilled rather than replaced, so a miss costs no allocation after the first frame.
    fn record(
        &mut self,
        kind: VehicleKind,
        left_m: f32,
        right_m: f32,
        dynamics: &GearDynamics<'_>,
    ) {
        self.kind = Some(kind);
        self.left_m = left_m;
        self.right_m = right_m;
        self.left_sag_scale = dynamics.left_sag_scale;
        self.right_sag_scale = dynamics.right_sag_scale;
        self.left_break_t = dynamics.left_break_t;
        self.right_break_t = dynamics.right_break_t;
        self.left_travel.clear();
        self.left_travel.extend_from_slice(dynamics.left_travel);
        self.right_travel.clear();
        self.right_travel.extend_from_slice(dynamics.right_travel);
    }
}

#[derive(Debug, Default)]
struct CachedGear {
    pose: GearPose,
    placements: Vec<GearPlacement>,
    /// Whether this tank was asked about since the last [`GearPlacementCache::retain_live`].
    seen: bool,
}

/// Per-tank EASED drive tension: the sag scale the frame actually draws, chasing the sim's
/// instantaneous target with a short time constant.
///
/// The target is built from the hull's longitudinal acceleration, which STEPS — a throttle tap
/// slams it from 1.0 to its clamp inside one tick. The belt geometry now responds to tension
/// continuously (depth only, never topology — `running_gear_belt.rs` v5), but a stepped input
/// still reads as the belt twitching taut "at once", which is exactly what the 2026-08-14
/// review called out. A ~0.3 s ease is how fast a physical belt visibly redistributes its
/// slack; it also keeps the tension change out of the single-frame pose diff, so the placement
/// cache sees a short glide and then EXACT equality (the snap below) instead of an asymptote
/// that rebuilds a resting tank's gear forever.
#[derive(Debug, Default)]
pub(crate) struct TensionEase {
    tanks: HashMap<TankId, EaseState>,
}

#[derive(Debug)]
struct EaseState {
    value: f32,
    last_tick: u64,
    /// Whether this tank was asked about since the last [`TensionEase::retain_live`].
    seen: bool,
}

impl TensionEase {
    /// Time for the drawn tension to close ~63% of its distance to the target.
    const TAU_S: f32 = 0.30;
    /// Close enough is EQUAL: the ease must terminate bit-exactly or the placement cache
    /// rebuilds a settled tank's gear every tick against an asymptote it can never reach.
    const SNAP: f32 = 0.005;

    /// The sag scale to draw this frame: last frame's value pulled toward `target` by the
    /// authoritative time that actually passed. A tank seen for the first time starts AT its
    /// target — a hull entering the field carries its real tension, it does not ease in from
    /// rest. Callers without a battle clock (`now_tick` pinned at 0) get the target straight
    /// through, so the garage and the probes stay pure functions of the pose they were given.
    pub(crate) fn eased(&mut self, tank: TankId, target: f32, now_tick: u64) -> f32 {
        let state = self.tanks.entry(tank).or_insert(EaseState {
            value: target,
            last_tick: now_tick,
            seen: false,
        });
        state.seen = true;
        if now_tick == 0 {
            // No battle clock (the garage, the probes, the pre-snapshot frames): no time can
            // pass, so no ease — the target goes straight through. Holding the FIRST target
            // instead would freeze a clockless caller's tension forever.
            state.value = target;
            return state.value;
        }
        let dt = now_tick.saturating_sub(state.last_tick) as f32
            / sim::DEFAULT_SIMULATION_TICK_HZ as f32;
        state.last_tick = now_tick;
        if dt > 0.0 {
            let alpha = 1.0 - (-dt / Self::TAU_S).exp();
            state.value += (target - state.value) * alpha;
        }
        if (state.value - target).abs() < Self::SNAP {
            state.value = target;
        }
        state.value
    }

    /// Forget the tanks that are no longer on the field (same sweep discipline as the
    /// placement cache: the catalog outlives the battle, this map must not).
    pub(crate) fn retain_live(&mut self) {
        self.tanks.retain(|_, state| std::mem::take(&mut state.seen));
    }
}

/// One entry per tank on the field.
#[derive(Debug, Default)]
pub(crate) struct GearPlacementCache {
    tanks: HashMap<TankId, CachedGear>,
    /// How many times a pose miss actually rebuilt. The point of this cache is that this number
    /// stops growing when the field stops moving, which is a fact worth a lock rather than a
    /// comment.
    #[cfg(test)]
    rebuilds: usize,
}

impl GearPlacementCache {
    /// This tank's hull-local running-gear placements, rebuilt only if something that shapes them
    /// moved since the last frame.
    pub(crate) fn placements(
        &mut self,
        tank: TankId,
        kind: VehicleKind,
        kin: &RunningGearKinematics,
        left_m: f32,
        right_m: f32,
        dynamics: GearDynamics<'_>,
    ) -> &[GearPlacement] {
        #[cfg(test)]
        let missed = self
            .tanks
            .get(&tank)
            .is_none_or(|cached| !cached.pose.matches(kind, left_m, right_m, &dynamics));
        #[cfg(test)]
        {
            self.rebuilds += usize::from(missed);
        }
        let cached = self.tanks.entry(tank).or_default();
        cached.seen = true;
        if !cached.pose.matches(kind, left_m, right_m, &dynamics) {
            running_gear_placements_dynamic_into(
                &mut cached.placements,
                kin,
                left_m,
                right_m,
                dynamics,
            );
            cached.pose.record(kind, left_m, right_m, &dynamics);
        }
        &cached.placements
    }

    /// Forget the tanks that are no longer on the field. Called once a frame from the fleet
    /// build — without it the map grows for the life of the catalog, which outlives the battle.
    pub(crate) fn retain_live(&mut self) {
        self.tanks.retain(|_, cached| std::mem::take(&mut cached.seen));
    }

    /// How many tanks the cache is holding — a diagnostic, and what the eviction lock measures.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.tanks.len()
    }

    #[cfg(test)]
    pub(crate) fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

#[cfg(test)]
mod tests {
    use vehicle_geometry::running_gear_placements_dynamic;

    use super::*;

    const TANK: TankId = TankId(7);
    const KIND: VehicleKind = VehicleKind::T54_1951;

    fn kinematics() -> RunningGearKinematics {
        RunningGearKinematics::for_vehicle(KIND).expect("the benchmark vehicle has running gear")
    }

    /// One pose, owning its travel buffers so a table of them can be written out flat.
    #[derive(Clone)]
    struct Pose {
        what_moved: &'static str,
        left_m: f32,
        right_m: f32,
        left_travel: Vec<f32>,
        right_travel: Vec<f32>,
        left_sag: f32,
        right_sag: f32,
        left_break_t: Option<f32>,
        right_break_t: Option<f32>,
    }

    impl Pose {
        fn base() -> Self {
            Self {
                what_moved: "nothing",
                left_m: 0.0,
                right_m: 0.0,
                left_travel: vec![0.0; 5],
                right_travel: vec![0.0; 5],
                left_sag: 1.0,
                right_sag: 1.0,
                left_break_t: None,
                right_break_t: None,
            }
        }

        fn dynamics(&self) -> GearDynamics<'_> {
            GearDynamics {
                left_travel: &self.left_travel,
                right_travel: &self.right_travel,
                left_sag_scale: self.left_sag,
                right_sag_scale: self.right_sag,
                left_break_t: self.left_break_t,
                right_break_t: self.right_break_t,
            }
        }
    }

    /// The base pose, edited in exactly ONE field per entry.
    fn one_field_edits() -> Vec<Pose> {
        let edit = |what_moved, apply: fn(&mut Pose)| {
            let mut pose = Pose::base();
            pose.what_moved = what_moved;
            apply(&mut pose);
            pose
        };
        vec![
            edit("the left track scrolled", |p| p.left_m = 1.37),
            edit("the right track scrolled", |p| p.right_m = 2.11),
            edit("a left road wheel travelled", |p| p.left_travel[2] = 0.09),
            edit("a right road wheel travelled", |p| p.right_travel[1] = -0.04),
            edit("the left top run tightened", |p| p.left_sag = 0.8),
            edit("the right top run slackened", |p| p.right_sag = 1.4),
            edit("the left track was thrown", |p| p.left_break_t = Some(0.3)),
            edit("the right track was thrown", |p| p.right_break_t = Some(0.7)),
        ]
    }

    /// THE safety property: a cached answer is the answer a fresh build would have given.
    ///
    /// Each pose is asked for immediately after the BASE pose, so it differs from what the cache
    /// last saw in exactly one field. That interleaving is the whole test: an earlier version
    /// walked the poses in sequence, where every pose differed from its predecessor in two fields
    /// at once, and dropping a field from the key passed it unharmed.
    #[test]
    fn the_cache_returns_exactly_what_a_fresh_build_would() {
        let kin = kinematics();
        let mut cache = GearPlacementCache::default();
        let base = Pose::base();
        for pose in one_field_edits() {
            cache.placements(TANK, KIND, &kin, base.left_m, base.right_m, base.dynamics());
            let fresh =
                running_gear_placements_dynamic(&kin, pose.left_m, pose.right_m, pose.dynamics());
            let cached = cache
                .placements(TANK, KIND, &kin, pose.left_m, pose.right_m, pose.dynamics())
                .to_vec();
            assert_ne!(
                fresh,
                running_gear_placements_dynamic(&kin, base.left_m, base.right_m, base.dynamics()),
                "'{}' does not actually change the placements, so it proves nothing here",
                pose.what_moved,
            );
            assert_eq!(
                cached, fresh,
                "after '{}' the cache handed back the previous pose's placements; its key does \
                 not cover that field",
                pose.what_moved,
            );
        }
    }

    /// And the point of it: a field that stopped moving stops being rebuilt. A wreck never moves
    /// again, and a tank holding a firing position moves none of these either.
    #[test]
    fn a_pose_that_did_not_move_is_not_rebuilt() {
        let kin = kinematics();
        let mut cache = GearPlacementCache::default();
        let travel = vec![0.0; 5];
        let dynamics = GearDynamics {
            left_travel: &travel,
            right_travel: &travel,
            left_sag_scale: 1.0,
            right_sag_scale: 1.0,
            left_break_t: None,
            right_break_t: None,
        };
        let first = cache.placements(TANK, KIND, &kin, 4.0, 4.0, dynamics).to_vec();
        assert_eq!(cache.rebuilds(), 1, "the first frame has to build");
        for _ in 0..30 {
            let again = cache.placements(TANK, KIND, &kin, 4.0, 4.0, dynamics);
            assert_eq!(again, first.as_slice(), "a still hull changed its own gear");
        }
        assert_eq!(cache.rebuilds(), 1, "a hull that did not move rebuilt its gear anyway");

        // And it does rebuild the moment it moves.
        cache.placements(TANK, KIND, &kin, 4.5, 4.0, dynamics);
        assert_eq!(cache.rebuilds(), 2, "a track that scrolled must rebuild");
    }

    /// The 2026-08-14 lock on "the tracks tighten AT ONCE": a stepped tension target must
    /// reach the drawn belt as a glide over several ticks, and then SETTLE bit-exactly so the
    /// placement cache stops rebuilding a tank whose tension stopped changing.
    #[test]
    fn a_tension_step_glides_in_and_settles_exactly() {
        let hz = sim::DEFAULT_SIMULATION_TICK_HZ as u64;
        let mut ease = TensionEase::default();
        // A tank seen for the first time carries its real tension — no ease-in from rest.
        assert_eq!(ease.eased(TANK, 1.0, 0), 1.0);
        // The throttle tap: the target steps to the driven clamp in one tick.
        let after_one_tick = ease.eased(TANK, 0.72, 1);
        assert!(
            (0.73..0.99).contains(&after_one_tick),
            "one tick after the step the belt must be BETWEEN the two tensions, got \
             {after_one_tick}"
        );
        // Same tick asked again (a 120 FPS frame between sim ticks): nothing moves, so the
        // placement cache can hit.
        assert_eq!(ease.eased(TANK, 0.72, 1), after_one_tick);
        // Two seconds of ticks later the glide has terminated EXACTLY at the target (the snap
        // band is crossed after ~1.2 s at this tau).
        let mut settled = after_one_tick;
        for tick in 2..=2 * hz {
            settled = ease.eased(TANK, 0.72, tick);
        }
        assert_eq!(settled, 0.72, "the ease must snap to its target, not chase an asymptote");

        // No battle clock (the garage, the probes): the target passes straight through.
        let mut still = TensionEase::default();
        assert_eq!(still.eased(TankId(9), 1.0, 0), 1.0);
        assert_eq!(still.eased(TankId(9), 1.5, 0), 1.5);
    }

    /// The ease map follows the same sweep discipline as the placement cache: a tank that left
    /// the field is forgotten, one that was just asked about is kept.
    #[test]
    fn the_tension_ease_forgets_departed_tanks() {
        let mut ease = TensionEase::default();
        ease.eased(TankId(1), 1.0, 0);
        ease.eased(TankId(2), 1.0, 0);
        ease.retain_live();
        ease.eased(TankId(1), 1.0, 1);
        ease.retain_live();
        assert!(ease.tanks.contains_key(&TankId(1)), "a live tank survives the sweep");
        assert!(!ease.tanks.contains_key(&TankId(2)), "a departed tank is forgotten");
    }

    /// The map must not outlive the battle: a tank that left the field is forgotten on the next
    /// sweep, not carried for the life of the catalog.
    #[test]
    fn a_tank_that_left_the_field_is_forgotten() {
        let kin = kinematics();
        let mut cache = GearPlacementCache::default();
        let travel = vec![0.0; 5];
        let dynamics = GearDynamics {
            left_travel: &travel,
            right_travel: &travel,
            left_sag_scale: 1.0,
            right_sag_scale: 1.0,
            left_break_t: None,
            right_break_t: None,
        };
        for id in 0..4 {
            cache.placements(TankId(id), KIND, &kin, 0.0, 0.0, dynamics);
        }
        cache.retain_live();
        assert_eq!(cache.len(), 4, "a sweep must not evict the tanks it just saw");

        // Next frame only two of them are on the field.
        for id in 0..2 {
            cache.placements(TankId(id), KIND, &kin, 0.0, 0.0, dynamics);
        }
        cache.retain_live();
        assert_eq!(cache.len(), 2, "the departed tanks are still held");
    }
}
