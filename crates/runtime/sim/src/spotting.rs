//! Line-of-sight spotting, v1. Every fixed interval the server recomputes which teams can see
//! each tank: a tank is spotted by a team if any living member of that team has an unobstructed
//! sight line to it within view range. Terrain ridges and static cover (buildings, rail berms,
//! tree lines) block the line; wrecks are always visible to everyone, and a team always sees its
//! own tanks.
//!
//! Honesty caveat: v1 only produces the `spotted` masks — the full snapshot still carries every
//! tank's position to every client, so this gates UI (minimap, enemy HP bars), not replication.
//! Per-client snapshot filtering is the real anti-wallhack follow-up; this mask is its foundation.

use std::collections::HashMap;

use game_core::TankId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use terrain::{HeightMap, StaticCoverObject};

use crate::TankState;

/// Legacy flat view range. v29 spots per OBSERVER spec (`TankSpec::view_range_m`, per vehicle);
/// this constant remains as the mid-era reference for fog tuning and bot heuristics.
pub const VIEW_RANGE_M: f32 = 400.0;
/// Recompute cadence: every 6 ticks = 10 Hz at the 60 Hz simulation.
pub const SPOTTING_INTERVAL_TICKS: u64 = 6;
/// How long a target stays lit after the fresh line of sight breaks (2 s at the 60 Hz sim).
pub const SPOTTED_HOLD_TICKS: u64 = 120;

/// A `u8` mask carries up to eight teams.
const MAX_SPOTTING_TEAMS: usize = 8;

/// One bit per observing hull: which crews' own eyes are on a given tank.
///
/// Widened from `u16` because sixteen was not a design decision, it was the type's width used as
/// a cap. A 7v7 fields fourteen hulls; an 8v8 fields exactly sixteen and would have sat on the
/// boundary, and anything past it stopped spotting **silently** — no panic, no log, just a crew
/// that never sees anyone. Thirty-two leaves room for the modes this game might actually field.
pub type ObserverMask = u32;

/// The most observing hulls one [`ObserverMask`] can name.
///
/// Derived from the mask's own width rather than written down beside it. The old code held the
/// same fact in three places — the `u16` type, a bare `.take(16)`, and an unguarded
/// `1 << viewer_index` in `net` — so changing the mask without finding all three would have cost
/// a whole crew their vision with nothing to show for it.
pub const MAX_OBSERVERS: usize = ObserverMask::BITS as usize;
/// Sentinel for "this team has never had fresh sight of the tank".
const NEVER_SEEN: u64 = u64::MAX;

/// THE SPOTTING DECISION (2.6, taken 2026-08-02) — two rules, one sentence each:
///
///   * **a stationary tank is seen from 70 % of range** — sitting still is the whole of
///     concealment, binary and readable, no percentage camouflage to memorise per vehicle;
///   * **firing makes you fully visible for 8 seconds** — the muzzle flash is the loudest,
///     brightest thing on a battlefield, and the sim already records the shot as a fact.
///
/// Together they create the scout loop this game lacked: a hull that stops and holds fire is
/// genuinely harder to find; the moment it shoots, it is lit. Deterministic, explainable, and
/// honest — nothing is hidden that a crew would see.
pub const STATIONARY_SPOT_FACTOR: f32 = 0.7;

/// Below this planar speed a hull counts as stationary. Half a metre a second is a crawl no
/// driver holds by accident — creeping to a ridge is still "moving".
pub const STATIONARY_SPEED_MPS: f32 = 0.5;

/// How long a shot keeps its firer fully visible (8 s at the 60 Hz sim).
pub const FIRE_REVEAL_TICKS: u64 = 480;

/// The fraction of an observer's view range at which `target` can currently be seen.
///
/// One rule shared by the team recompute and the personal observer masks, so the two kinds of
/// sight can never disagree about what concealment means.
pub fn spotting_range_factor(target: &TankState, tick: u64) -> f32 {
    let recently_fired =
        target.last_shot_tick.is_some_and(|shot| tick.saturating_sub(shot) <= FIRE_REVEAL_TICKS);
    let planar_speed = Vec3::new(target.velocity_mps.x, 0.0, target.velocity_mps.z).length();
    if recently_fired || planar_speed > STATIONARY_SPEED_MPS { 1.0 } else { STATIONARY_SPOT_FACTOR }
}

/// Per-tank memory of the last tick each team had FRESH line of sight. The LOS test is boolean
/// and recomputed at 10 Hz, so a target dancing on a ridge line strobes in and out several times
/// a second — its model pops, the minimap blinks, and the shooter's ballistic aim point flips
/// between the hull and the terrain behind it. Holding the spot for [`SPOTTED_HOLD_TICKS`] after
/// the line breaks (WoT's minimum spotted duration, and honest — the crew just saw it) turns the
/// strobe into one clean spot-then-fade cycle.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpottingMemory {
    last_fresh_tick: HashMap<TankId, [u64; MAX_SPOTTING_TEAMS]>,
}

impl SpottingMemory {
    /// How many tanks this memory still holds a hold-clock for. Bounded by the live roster —
    /// see [`Self::forget_departed`], which is what this exists to lock.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.last_fresh_tick.len()
    }

    /// Record this recompute's fresh sightings and return the mask with held bits added.
    fn hold(&mut self, tank: TankId, fresh_mask: u8, tick: u64) -> u8 {
        let entry = self.last_fresh_tick.entry(tank).or_insert([NEVER_SEEN; MAX_SPOTTING_TEAMS]);
        let mut mask = fresh_mask;
        for (team, last_fresh) in entry.iter_mut().enumerate() {
            let bit = 1u8 << team;
            if fresh_mask & bit != 0 {
                *last_fresh = tick;
            } else if *last_fresh != NEVER_SEEN
                && tick.saturating_sub(*last_fresh) <= SPOTTED_HOLD_TICKS
            {
                mask |= bit;
            }
        }
        mask
    }

    /// Forget tanks that are no longer in the battle. Ids are never reused — `spawn_tank` and
    /// `replace_tank_with_spec` both mint fresh ones — so without this the map grows for the
    /// life of the `SimulationState`, one dead entry per vehicle swap. The length guard keeps
    /// the ordinary tick (nothing left) free.
    fn forget_departed(&mut self, tanks: &[crate::TankState]) {
        if self.last_fresh_tick.len() <= tanks.len() {
            return;
        }
        self.last_fresh_tick.retain(|id, _| tanks.iter().any(|tank| tank.id == *id));
    }
}

/// The coarsest the sight line may ever step, in metres.
///
/// On a 5 m grid this is 2.5 samples per cell — the margin the check has always had, and the
/// reason it never missed a ridge. The cap keeps that: a coarser grid must not make the eye
/// coarser too, or a long sight line over gentle ground starts skipping crests.
const LOS_STEP_CEILING_M: f32 = 2.0;

/// How far apart the sight line samples the ground, for a heightmap of this cell size.
///
/// Two samples per cell, never coarser than [`LOS_STEP_CEILING_M`]. Public because the promise is
/// not "some step that works today" — it is that the eye never samples the terrain more coarsely
/// than the terrain is defined, on any grid a map might use.
pub fn terrain_sight_step_m(cell_size_m: f32) -> f32 {
    (cell_size_m * 0.5).clamp(0.1, LOS_STEP_CEILING_M)
}

/// Whether the segment `from -> to` clears the terrain: step along it and fail if the ground ever
/// rises above the sight line (with a little slack so grazing a crest still counts as seeing over).
///
/// THE STEP IS TIED TO THE GRID, at two samples per cell. It used to be a flat 2 m, which is fine
/// while cells are 5 m wide and quietly wrong once they are not: at 2.5 m that is 1.25 samples per
/// cell and at 1.25 m it is 0.6 — the sight line steps clean over ridges a shell would hit, and
/// `shell_trace` marches its own terrain sweep at 1 m regardless. A bot would then fire at
/// something it "sees" through a crest and eat the crest, which is the honesty doctrine breaking
/// in the one place it is least visible: what blocks the shell must block the eye.
///
/// At today's 5 m grid the cap wins and nothing changes, so this costs no ticks until a map is
/// actually densified — which is the point of writing it before densifying one.
fn terrain_clear(heightmap: &HeightMap, from: Vec3, to: Vec3) -> bool {
    let segment = to - from;
    let step_m = terrain_sight_step_m(heightmap.cell_size_m());
    let steps = (segment.length() / step_m).ceil().max(1.0) as u32;
    for step in 1..steps {
        let point = from + segment * (step as f32 / steps as f32);
        if heightmap.sample_height(point.x, point.z).is_some_and(|g| g > point.y + 0.3) {
            return false;
        }
    }
    true
}

/// Whether the segment enters the axis-aligned cover box (slab method). A hit means the sight line
/// is blocked by that cover.
fn segment_hits_box(from: Vec3, to: Vec3, center: [f32; 3], half: [f32; 3]) -> bool {
    let dir = to - from;
    let (mut t_min, mut t_max) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        let d = dir[axis];
        let lo = center[axis] - half[axis];
        let hi = center[axis] + half[axis];
        if d.abs() < 1.0e-6 {
            if from[axis] < lo || from[axis] > hi {
                return false; // parallel and outside this slab
            }
        } else {
            let mut t1 = (lo - from[axis]) / d;
            let mut t2 = (hi - from[axis]) / d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return false;
            }
        }
    }
    true
}

/// A full sight line: terrain unobstructed and no cover box in the way. Cover goes first: a
/// slab test costs nanoseconds while the terrain walk samples the heightmap every 2 m of the
/// segment — and in a town fight (Bystra fields ~38 boxes) a building is the common reason a
/// line is blocked, so the cheap test usually decides. The XZ-rect broadphase in front of the
/// slab is what keeps an urban box count (150+) honest: most boxes are nowhere near a given
/// sight line, and rejecting them costs four comparisons instead of a three-axis slab (the
/// in-module property test locks that the prefilter never changes a verdict).
pub fn line_of_sight(
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    from: Vec3,
    to: Vec3,
) -> bool {
    let blocked = cover.iter().any(|c| {
        !game_core::math::segment_xz_disjoint(
            from,
            to,
            c.center[0],
            c.center[2],
            c.half_extents_m[0],
            c.half_extents_m[2],
        ) && segment_hits_box(from, to, c.center, c.half_extents_m)
    });
    if blocked {
        return false;
    }
    heightmap.is_none_or(|heightmap| terrain_clear(heightmap, from, to))
}

/// Whether `observer`'s commander eye has a clear line to any of `target`'s sample points — the
/// exact geometry one observer contributes to the spotting recompute. The bot brain uses this to
/// engage only targets IT can see: a team-spotted mask says "someone on my team sees it", not
/// "my own shell has a path", and firing on the mask alone means shelling the front of a hill.
pub fn tank_line_of_sight(
    observer: &TankState,
    target: &TankState,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) -> bool {
    let eye = observer_eye(observer);
    target_points(target).into_iter().any(|point| line_of_sight(heightmap, cover, eye, point))
}

/// The commander's eye of an observer: the top of the hull box.
fn observer_eye(tank: &TankState) -> Vec3 {
    tank.position + Vec3::Y * (tank.spec.hitbox.center_y_m + tank.spec.hitbox.half_height_m)
}

/// Sample points on a target that count as "seen": the hull centre and the turret top.
fn target_points(tank: &TankState) -> [Vec3; 2] {
    let hitbox = &tank.spec.hitbox;
    [
        tank.position + Vec3::Y * hitbox.center_y_m,
        tank.position + Vec3::Y * (hitbox.center_y_m + hitbox.half_height_m),
    ]
}

/// Refresh every tank's `spotted_mask` on the fixed spotting cadence. Runs off `tick` before the
/// sim advances it, so tick 0 seeds the masks and the recompute lands every
/// `SPOTTING_INTERVAL_TICKS` thereafter.
pub(crate) fn refresh_spotted_masks(
    tick: u64,
    tanks: &mut [TankState],
    memory: &mut SpottingMemory,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) {
    if !tick.is_multiple_of(SPOTTING_INTERVAL_TICKS) {
        return;
    }
    apply_spotted_masks_with_hold(tick, tanks, memory, heightmap, cover);
}

/// One full recompute: fresh LOS masks, folded through the spotting memory's hold.
pub(crate) fn apply_spotted_masks_with_hold(
    tick: u64,
    tanks: &mut [TankState],
    memory: &mut SpottingMemory,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) {
    let masks = compute_spotted_masks(tanks, tick, heightmap, cover);
    for (tank, fresh_mask) in tanks.iter_mut().zip(masks) {
        tank.spotted_mask = memory.hold(tank.id, fresh_mask, tick);
    }
    memory.forget_departed(tanks);
}

/// Compute, for each tank (in `tanks` order), the bitmask of teams that can currently see it. A
/// team sees a tank when any living member has LOS within ITS OWN view range (per-vehicle optics,
/// `TankSpec::view_range_m`); a tank's own team always sees it, and a wreck is public.
///
/// The radio speaks here (v29): an observer with a DESTROYED radio no longer contributes to the
/// team mask — its sightings stay its own. What it personally sees is tracked separately in
/// [`compute_observer_masks`], so a radio-dead crew still fights everything in front of its own
/// eyes; it just cannot light targets for the team, nor the team for it.
pub fn compute_spotted_masks(
    tanks: &[TankState],
    tick: u64,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) -> Vec<u8> {
    let mut masks = vec![0u8; tanks.len()];
    for (i, target) in tanks.iter().enumerate() {
        // Own team always sees its own vehicles; a wreck is public to all teams.
        masks[i] |= target.team.spotting_bit();
        if target.hit_points == 0 {
            masks[i] = u8::MAX;
            continue;
        }
        let points = target_points(target);
        for observer in tanks.iter() {
            if observer.hit_points == 0
                || observer.team == target.team
                || masks[i] & observer.team.spotting_bit() != 0
                || !observer.modules.is_functional(game_core::ModuleSlot::Radio)
            {
                continue;
            }
            let eye = observer_eye(observer);
            // The commander IS the eyes: his station covered or scarred shortens what the crew
            // picks out (crew-damage, v46) — a multiplier on range, never a blindfold.
            let commander = observer.crew.effectiveness(game_core::CrewRole::Commander);
            let range =
                observer.spec.view_range_m() * spotting_range_factor(target, tick) * commander;
            if eye.distance(target.position) > range {
                continue;
            }
            if points.iter().any(|&p| line_of_sight(heightmap, cover, eye, p)) {
                masks[i] |= observer.team.spotting_bit();
            }
        }
    }
    masks
}

/// Per-target mask of OBSERVER TANKS (bit = index in `tanks`, up to 16) with personal fresh
/// line of sight. Radio state is irrelevant here — these are the observer's own eyes. The
/// per-viewer snapshot filter unions this with the team mask so no enemy in plain sight ever
/// vanishes off a radio-dead crew's screen.
pub fn compute_observer_masks(
    tanks: &[TankState],
    tick: u64,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) -> Vec<ObserverMask> {
    // A roster past the cap does not half-work: hulls beyond it observe nobody. Loud in dev and
    // in every test rather than a quiet blind spot in a shipped mode.
    debug_assert!(
        tanks.len() <= MAX_OBSERVERS,
        "{} hulls exceeds the {MAX_OBSERVERS}-observer mask: hulls past it would spot nobody",
        tanks.len()
    );
    let mut masks = vec![0 as ObserverMask; tanks.len()];
    for (i, target) in tanks.iter().enumerate() {
        if target.hit_points == 0 {
            masks[i] = ObserverMask::MAX;
            continue;
        }
        let points = target_points(target);
        for (observer_index, observer) in tanks.iter().enumerate().take(MAX_OBSERVERS) {
            if observer.hit_points == 0 {
                continue;
            }
            if observer.team == target.team {
                masks[i] |= 1 << observer_index;
                continue;
            }
            let eye = observer_eye(observer);
            // The SAME concealment rule as the team recompute: personal eyes obey what the crew
            // could actually pick out, or the two kinds of sight drift apart — the commander's
            // state included.
            let commander = observer.crew.effectiveness(game_core::CrewRole::Commander);
            let range =
                observer.spec.view_range_m() * spotting_range_factor(target, tick) * commander;
            if eye.distance(target.position) > range {
                continue;
            }
            if points.iter().any(|&p| line_of_sight(heightmap, cover, eye, p)) {
                masks[i] |= 1 << observer_index;
            }
        }
    }
    masks
}

#[cfg(test)]
mod memory_tests {
    use game_core::{TankSpec, TeamId};
    use glam::Vec3;

    use crate::SimulationState;

    /// Tank ids are never reused, so a battle that swaps vehicles used to leave one dead entry
    /// per swap in the spotting memory for the life of the `SimulationState`. The live roster is
    /// the bound.
    #[test]
    fn the_spotting_memory_does_not_outgrow_the_live_roster() {
        let mut state = SimulationState::new();
        let mut tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
        state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 60.0));
        for _ in 0..40 {
            state.refresh_spotting(None, &[]);
            tank = state
                .replace_tank_with_spec(tank, TankSpec::tiger_i_ausf_e())
                .expect("the seat is replaced");
        }
        state.refresh_spotting(None, &[]);
        assert!(
            state.spotting_memory_len() <= state.tanks().len(),
            "spotting memory holds {} entries for {} live tanks",
            state.spotting_memory_len(),
            state.tanks().len()
        );
    }
}

#[cfg(test)]
mod broadphase_tests {
    use super::*;
    use terrain::StaticCoverKind;

    /// A deterministic 150-box street grid the size of an urban core.
    fn urban_cover() -> Vec<StaticCoverObject> {
        let mut out = Vec::new();
        for column in 0..10 {
            for row in 0..15 {
                out.push(StaticCoverObject {
                    id: format!("block_c{column}_r{row}"),
                    name: format!("block {column}/{row}"),
                    kind: StaticCoverKind::FarmBuilding,
                    center: [60.0 + column as f32 * 42.0, 4.0, 60.0 + row as f32 * 30.0],
                    half_extents_m: [8.0 + (row % 3) as f32, 4.0, 5.0 + (column % 2) as f32],
                });
            }
        }
        out
    }

    fn xorshift(state: &mut u32) -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        (*state % 10_000) as f32 / 10_000.0
    }

    /// The broadphase is a pure prefilter: over hundreds of random sight lines - including
    /// grazing, in-canyon, and box-origin segments - the prefiltered verdict equals the
    /// exact all-boxes slab walk. This is the license to ship the early-out at all.
    #[test]
    fn the_prefilter_never_changes_a_line_of_sight_verdict() {
        let cover = urban_cover();
        let mut state = 0x1234_5678u32;
        let mut disagreements = Vec::new();
        for case in 0..600 {
            let from = Vec3::new(
                xorshift(&mut state) * 520.0,
                1.0 + xorshift(&mut state) * 9.0,
                xorshift(&mut state) * 520.0,
            );
            let to = Vec3::new(
                xorshift(&mut state) * 520.0,
                1.0 + xorshift(&mut state) * 9.0,
                xorshift(&mut state) * 520.0,
            );
            let exact_clear =
                !cover.iter().any(|c| segment_hits_box(from, to, c.center, c.half_extents_m));
            let filtered_clear = line_of_sight(None, &cover, from, to);
            if exact_clear != filtered_clear {
                disagreements.push((case, from, to));
            }
        }
        assert!(disagreements.is_empty(), "the broadphase changed a verdict: {disagreements:?}");
    }
}
