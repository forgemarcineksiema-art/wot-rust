//! Snapshot-derived inputs for the sprung-hull attitude: the terrain sample under one hull and
//! the live suspension pool. Split from `world.rs` to keep the sync loop within the
//! reviewability budget.

use game_core::{HitboxProfile, ModuleSlot};
use net::TankSnapshot;

use crate::attitude::AttitudeSample;

/// Live suspension pool `0..=1` from the replicated module HP against the vehicle's full pool.
/// The sprung-hull attitude reads it as spring softness: a wounded suspension wallows.
pub(crate) fn suspension_pool_fraction(tank: &TankSnapshot) -> f32 {
    let full = tank.vehicle.spec().module_health.hit_points(ModuleSlot::Suspension);
    let live = tank.module_hit_points[ModuleSlot::Suspension.wire_index()];
    (live as f32 / full.max(1) as f32).clamp(0.0, 1.0)
}

/// Terrain pitch/roll under one hull, sampled at the wheelbase extents in the hull frame. The
/// probes match the contact patch (not the bumper-to-bumper length) so a curb under the nose tips
/// the hull the way real bogies would.
pub(crate) fn sample_attitude(
    terrain: Option<&terrain::HeightMap>,
    tank: &TankSnapshot,
    hitbox: &HitboxProfile,
) -> AttitudeSample {
    let Some(map) = terrain else {
        return AttitudeSample::default();
    };
    let (sin, cos) = tank.yaw_rad.sin_cos();
    let (x, z) = (tank.position[0], tank.position[2]);
    let probe_z = (hitbox.half_length_m * 0.62).max(0.5);
    let probe_x = (hitbox.half_width_m * 0.8).max(0.5);
    let center = map.sample_height(x, z).unwrap_or(tank.position[1]);
    let at = |dx: f32, dz: f32| map.sample_height(x + dx, z + dz).unwrap_or(center);
    // Hull-local forward is (sin, cos) in XZ; right is (cos, -sin).
    let front = at(sin * probe_z, cos * probe_z);
    let back = at(-sin * probe_z, -cos * probe_z);
    let right = at(cos * probe_x, -sin * probe_x);
    let left = at(-cos * probe_x, sin * probe_x);
    AttitudeSample {
        terrain_pitch_rad: ((front - back) / (2.0 * probe_z)).atan(),
        terrain_roll_rad: ((right - left) / (2.0 * probe_x)).atan(),
    }
}
