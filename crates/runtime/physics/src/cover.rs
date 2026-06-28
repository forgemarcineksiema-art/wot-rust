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
    if cover.is_empty() || !blocked(attempted, yaw_rad, footprint, cover) {
        return attempted;
    }
    let x_only = Vec3::new(attempted.x, attempted.y, previous.z);
    if !blocked(x_only, yaw_rad, footprint, cover) {
        return x_only;
    }
    let z_only = Vec3::new(previous.x, attempted.y, attempted.z);
    if !blocked(z_only, yaw_rad, footprint, cover) {
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
/// axis-aligned obstacle, i.e. a yaw-0 footprint in the shared SAT.
fn blocked(
    position: Vec3,
    yaw_rad: f32,
    footprint: TankFootprint,
    cover: &[StaticCoverObject],
) -> bool {
    let hull = TankObstacle::new(position, yaw_rad, footprint);
    cover.iter().any(|object| obstacles_overlap(&hull, &cover_obstacle(object)))
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
