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

/// How far a tank can be spotted, flat across eras in v1.
pub const VIEW_RANGE_M: f32 = 400.0;
/// Recompute cadence: every 6 ticks = 10 Hz at the 60 Hz simulation.
pub const SPOTTING_INTERVAL_TICKS: u64 = 6;
/// How long a target stays lit after the fresh line of sight breaks (2 s at the 60 Hz sim).
pub const SPOTTED_HOLD_TICKS: u64 = 120;

/// A `u8` mask carries up to eight teams.
const MAX_SPOTTING_TEAMS: usize = 8;
/// Sentinel for "this team has never had fresh sight of the tank".
const NEVER_SEEN: u64 = u64::MAX;

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
}

/// Whether the segment `from -> to` clears the terrain: step along it and fail if the ground ever
/// rises above the sight line (with a little slack so grazing a crest still counts as seeing over).
fn terrain_clear(heightmap: &HeightMap, from: Vec3, to: Vec3) -> bool {
    let segment = to - from;
    let steps = (segment.length() / 2.0).ceil().max(1.0) as u32;
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
/// line is blocked, so the cheap test usually decides.
pub fn line_of_sight(
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    from: Vec3,
    to: Vec3,
) -> bool {
    if cover.iter().any(|c| segment_hits_box(from, to, c.center, c.half_extents_m)) {
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
    let masks = compute_spotted_masks(tanks, heightmap, cover);
    for (tank, fresh_mask) in tanks.iter_mut().zip(masks) {
        tank.spotted_mask = memory.hold(tank.id, fresh_mask, tick);
    }
}

/// Compute, for each tank (in `tanks` order), the bitmask of teams that can currently see it. A
/// team sees a tank when any living member has LOS within `VIEW_RANGE_M`; a tank's own team always
/// sees it, and a wreck is visible to everyone.
pub fn compute_spotted_masks(
    tanks: &[TankState],
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
            {
                continue;
            }
            let eye = observer_eye(observer);
            if eye.distance(target.position) > VIEW_RANGE_M {
                continue;
            }
            if points.iter().any(|&p| line_of_sight(heightmap, cover, eye, p)) {
                masks[i] |= observer.team.spotting_bit();
            }
        }
    }
    masks
}
