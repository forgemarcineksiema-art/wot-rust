//! Snapshot-derived inputs for the per-frame presentation sync: the attitude sample each hull's
//! spring chases and the suspension softness it runs at. Split from `world.rs` to keep the sync
//! loop itself within the reviewability budget.

use game_core::{ModuleSlot, TankId, TrackDamageMask};
use net::TankSnapshot;

use crate::attitude::AttitudeSample;
use crate::components::GunRecoil;
use crate::world::PresentationWorld;

/// The attitude target for one synced tank: the AUTHORITATIVE support-plane attitude replicated
/// in the snapshot (protocol v14) plus the thrown-track lean. The spring smooths the 20 Hz steps
/// and layers weight-transfer theatrics on top — it does not re-derive terrain itself.
pub(crate) fn attitude_sample(tank: &TankSnapshot) -> AttitudeSample {
    AttitudeSample {
        terrain_pitch_rad: tank.hull_pitch_rad,
        terrain_roll_rad: tank.hull_roll_rad
            + broken_track_lean_rad(TrackDamageMask::from_bits(tank.track_damage_mask)),
    }
}

/// A thrown track drops the hull ~a road-wheel rubber's worth on that side: left broken seats
/// the left side down (negative roll — "+roll" is right side up), right broken mirrors it, both
/// broken sit level (just beaten, which the heave dip already sells).
fn broken_track_lean_rad(damage: TrackDamageMask) -> f32 {
    const LEAN_RAD: f32 = 0.028;
    match (
        damage.is_broken(game_core::TrackSide::Left),
        damage.is_broken(game_core::TrackSide::Right),
    ) {
        (true, false) => -LEAN_RAD,
        (false, true) => LEAN_RAD,
        _ => 0.0,
    }
}

/// Live suspension pool `0..=1` from the replicated module HP against the vehicle's full pool.
/// The sprung-hull attitude reads it as spring softness: a wounded suspension wallows.
pub(crate) fn suspension_pool_fraction(tank: &TankSnapshot) -> f32 {
    let full = tank.vehicle.spec().module_health.hit_points(ModuleSlot::Suspension);
    let live = tank.module_hit_points[ModuleSlot::Suspension.wire_index()];
    (live as f32 / full.max(1) as f32).clamp(0.0, 1.0)
}

impl PresentationWorld {
    /// One replicated shot from `tank_id`: throw its barrel into recoil and rock its sprung hull
    /// by the turret's heading. Both cues are presentation-only springs; a fire event for a tank
    /// the world has not seen yet is dropped (its first frame has nothing to animate anyway).
    /// `recoil_scale` is the round's (Inny Poziom S3): both springs take it, so the barrel
    /// stroke and the hull's rock grow with the gun together.
    pub fn apply_fire_recoil(&mut self, tank_id: TankId, turret_yaw_rad: f32, recoil_scale: f32) {
        let Some(entity) = self.entity_of(tank_id) else {
            return;
        };
        if let Some(mut recoil) = self.world_mut().get_mut::<GunRecoil>(entity) {
            recoil.kick(recoil_scale);
        }
        if let Some(mut attitude) = self.world_mut().get_mut::<crate::HullAttitude>(entity) {
            attitude.fire_impulse(turret_yaw_rad, recoil_scale);
        }
    }

    /// An incoming shell rocks the struck tank's sprung hull from the side it came in (Inny
    /// Poziom S5) — every tank in view, not only the player's: a hit is seen in the body of
    /// the hull that took it. `bearing_hull_rad` and `energy` as [`HullAttitude::hit_impulse`].
    pub fn apply_hit_impulse(&mut self, tank_id: TankId, bearing_hull_rad: f32, energy: f32) {
        let Some(entity) = self.entity_of(tank_id) else {
            return;
        };
        if let Some(mut attitude) = self.world_mut().get_mut::<crate::HullAttitude>(entity) {
            attitude.hit_impulse(bearing_hull_rad, energy);
        }
    }
}
