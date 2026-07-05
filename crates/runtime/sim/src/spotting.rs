//! Line-of-sight spotting, v1. Every fixed interval the server recomputes which teams can see
//! each tank: a tank is spotted by a team if any living member of that team has an unobstructed
//! sight line to it within view range. Terrain ridges and static cover (buildings, rail berms,
//! tree lines) block the line; wrecks are always visible to everyone, and a team always sees its
//! own tanks.
//!
//! Honesty caveat: v1 only produces the `spotted` masks — the full snapshot still carries every
//! tank's position to every client, so this gates UI (minimap, enemy HP bars), not replication.
//! Per-client snapshot filtering is the real anti-wallhack follow-up; this mask is its foundation.

use game_core::TeamId;
use glam::Vec3;
use terrain::{HeightMap, StaticCoverObject};

use crate::TankState;

/// How far a tank can be spotted, flat across eras in v1.
pub const VIEW_RANGE_M: f32 = 400.0;
/// Recompute cadence: every 6 ticks = 10 Hz at the 60 Hz simulation.
pub const SPOTTING_INTERVAL_TICKS: u64 = 6;

/// The team-bit for a `TeamId` in the `spotted` mask (team 1 -> bit 0). Teams beyond 8 are
/// clamped into the top bit; the roster never approaches that.
fn team_bit(team: TeamId) -> u8 {
    let index = (team.0 as u32).saturating_sub(1).min(7);
    1u8 << index
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

/// A full sight line: terrain unobstructed and no cover box in the way.
pub fn line_of_sight(
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    from: Vec3,
    to: Vec3,
) -> bool {
    if let Some(heightmap) = heightmap
        && !terrain_clear(heightmap, from, to)
    {
        return false;
    }
    !cover.iter().any(|c| segment_hits_box(from, to, c.center, c.half_extents_m))
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
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) {
    if !tick.is_multiple_of(SPOTTING_INTERVAL_TICKS) {
        return;
    }
    let masks = compute_spotted_masks(tanks, heightmap, cover);
    for (tank, mask) in tanks.iter_mut().zip(masks) {
        tank.spotted_mask = mask;
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
        masks[i] |= team_bit(target.team);
        if target.hit_points == 0 {
            masks[i] = u8::MAX;
            continue;
        }
        let points = target_points(target);
        for observer in tanks.iter() {
            if observer.hit_points == 0
                || observer.team == target.team
                || masks[i] & team_bit(observer.team) != 0
            {
                continue;
            }
            let eye = observer_eye(observer);
            if eye.distance(target.position) > VIEW_RANGE_M {
                continue;
            }
            if points.iter().any(|&p| line_of_sight(heightmap, cover, eye, p)) {
                masks[i] |= team_bit(observer.team);
            }
        }
    }
    masks
}
