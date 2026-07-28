use glam::Vec3;
use terrain::StaticCoverObject;

use crate::collision::{TankFootprint, TankObstacle, obstacles_overlap, trim_velocity};

/// Keep a tank hull out of static cover footprints. Tries the full move, then each horizontal
/// axis alone so the hull slides along a wall instead of sticking; if every option still
/// overlaps cover the hull holds its previous horizontal position. `y` is taken from
/// `attempted`.
///
/// Blocking tests the hull's real oriented footprint against the cover box — the same XZ SAT
/// movement uses against other tanks — not a fixed-radius point. The old point-radius test let
/// long hulls bury their nose 1.6–2.5 m inside buildings; `tests/cover_footprint.rs` locks the
/// negative cases (no interpenetration, and no blocking on a clean near miss).
pub fn resolve_cover_collision(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    cover: &[StaticCoverObject],
) -> Vec3 {
    if cover.is_empty() || !footprint_blocked_by_cover(attempted, yaw_rad, footprint, cover) {
        return attempted;
    }
    let x_only = Vec3::new(attempted.x, attempted.y, previous.z);
    if !footprint_blocked_by_cover(x_only, yaw_rad, footprint, cover) {
        return x_only;
    }
    let z_only = Vec3::new(previous.x, attempted.y, attempted.z);
    if !footprint_blocked_by_cover(z_only, yaw_rad, footprint, cover) {
        return z_only;
    }
    Vec3::new(previous.x, attempted.y, previous.z)
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_cover_collision_with_velocity(
    previous: Vec3,
    attempted: Vec3,
    yaw_rad: f32,
    velocity: Vec3,
    footprint: TankFootprint,
    cover: &[StaticCoverObject],
) -> (Vec3, Vec3) {
    let resolved = resolve_cover_collision(previous, attempted, yaw_rad, footprint, cover);
    (resolved, trim_velocity(previous, attempted, resolved, velocity))
}

/// Whether the hull footprint at `position`/`yaw_rad` overlaps any cover box. A cover box is an
/// axis-aligned obstacle, i.e. a yaw-0 footprint in the shared SAT. Public because the world
/// step also asks it about a ROTATION candidate (yaw is collision-resolved like translation).
///
/// The circumradius early-out in front of the SAT is the urban-map broadphase: two shapes whose
/// XZ centre distance exceeds the sum of their circumscribed radii cannot overlap under ANY yaw,
/// so on a 100+-box map the full projection test runs only against the handful of nearby boxes
/// (the in-module property test locks that the early-out never changes a verdict).
pub fn footprint_blocked_by_cover(
    position: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    cover: &[StaticCoverObject],
) -> bool {
    let hull = TankObstacle::new(position, yaw_rad, footprint);
    let hull_reach = circumradius(footprint.half_width_m, footprint.half_length_m);
    cover.iter().any(|object| {
        let dx = position.x - object.center[0];
        let dz = position.z - object.center[2];
        let reach = hull_reach + circumradius(object.half_extents_m[0], object.half_extents_m[2]);
        dx * dx + dz * dz <= reach * reach && obstacles_overlap(&hull, &cover_obstacle(object))
    })
}

/// Whether the hull footprint at `position`/`yaw_rad` overlaps ONE cover box — the single-object
/// half of [`footprint_blocked_by_cover`], for callers that must know WHICH box the hull is on
/// top of rather than merely whether anything blocks. Cover crushing is the caller: what a hull
/// flattens has to be what its real oriented footprint reaches, or the hull bulldozes hedgerows
/// it drove cleanly past.
pub fn footprint_overlaps_cover_object(
    position: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    object: &StaticCoverObject,
) -> bool {
    obstacles_overlap(&TankObstacle::new(position, yaw_rad, footprint), &cover_obstacle(object))
}

/// Radius of the circle circumscribing a `half_x` × `half_z` footprint.
#[inline]
fn circumradius(half_x: f32, half_z: f32) -> f32 {
    (half_x * half_x + half_z * half_z).sqrt()
}

fn cover_obstacle(object: &StaticCoverObject) -> TankObstacle {
    TankObstacle::new(
        Vec3::new(object.center[0], object.center[1], object.center[2]),
        0.0,
        TankFootprint {
            half_width_m: object.half_extents_m[0].max(0.01),
            half_length_m: object.half_extents_m[2].max(0.01),
        },
    )
}

#[cfg(test)]
mod broadphase_tests {
    use super::*;
    use terrain::StaticCoverKind;

    fn xorshift(state: &mut u32) -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        (*state % 10_000) as f32 / 10_000.0
    }

    /// The circumradius early-out must be invisible in results: over hundreds of random hull
    /// poses — including hulls kissing a wall corner at full yaw — the early-outed verdict
    /// equals the exact all-boxes SAT walk.
    #[test]
    fn the_early_out_never_changes_a_blocking_verdict() {
        let mut cover = Vec::new();
        for column in 0..10 {
            for row in 0..15 {
                cover.push(StaticCoverObject {
                    id: format!("block_c{column}_r{row}"),
                    name: format!("block {column}/{row}"),
                    kind: StaticCoverKind::FarmBuilding,
                    center: [60.0 + column as f32 * 42.0, 4.0, 60.0 + row as f32 * 30.0],
                    half_extents_m: [8.0 + (row % 3) as f32, 4.0, 5.0 + (column % 2) as f32],
                });
            }
        }
        let footprint = TankFootprint { half_width_m: 1.7, half_length_m: 3.4 };
        let mut state = 0x0bad_cafeu32;
        for _ in 0..800 {
            let position =
                Vec3::new(xorshift(&mut state) * 520.0, 0.0, xorshift(&mut state) * 520.0);
            let yaw = xorshift(&mut state) * std::f32::consts::TAU;
            let hull = TankObstacle::new(position, yaw, footprint);
            let exact =
                cover.iter().any(|object| obstacles_overlap(&hull, &cover_obstacle(object)));
            let filtered = footprint_blocked_by_cover(position, yaw, footprint, &cover);
            assert_eq!(exact, filtered, "early-out changed the verdict at {position:?} yaw {yaw}");
        }
    }
}
