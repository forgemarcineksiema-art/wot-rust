use std::collections::HashMap;

use bevy_ecs::prelude::*;
use game_core::{TankId, TrackDamageMask};
use net::TankSnapshot;

use crate::attitude::{HullAttitude, TankMotion};
use crate::components::{
    DestroyedModules, GunPitch, GunRecoil, Health, ModuleHitPoints, PresentationTank,
    RenderTransform, Spotted, TankEntity, Team, Time, TrackAnimation, TurretJerk, TurretYaw,
    Vehicle, VehicleDamage,
};
use crate::sync_cues::{attitude_sample, suspension_pool_fraction};

/// Persistent client presentation world: a `bevy_ecs` world of presentation entities projected
/// from the snapshot buffer. The renderer and HUD read this, not `net::TankSnapshot` directly.
///
/// Gameplay truth stays server-side; this world is a presentation projection only.
pub struct PresentationWorld {
    world: World,
    /// `NetId -> Entity` so each networked tank keeps a stable entity across frames (and snapshots
    /// only ever update, never re-spawn, an existing tank).
    entities: HashMap<TankId, Entity>,
}

impl Default for PresentationWorld {
    fn default() -> Self {
        let mut world = World::new();
        world.insert_resource(Time::default());
        Self { world, entities: HashMap::new() }
    }
}

impl PresentationWorld {
    /// Current render clock.
    pub fn time(&self) -> Time {
        *self.world.resource::<Time>()
    }

    /// Longest frame delta the presentation clock accepts. A stall (debugger, OS suspend, window
    /// drag) otherwise lands as one huge `delta_seconds`, and every frame-domain consumer that
    /// does not clamp locally lurches through it in a single step.
    const MAX_FRAME_DELTA_SECONDS: f32 = 0.1;

    /// Advance the render clock by one presented frame.
    pub fn advance_time(&mut self, delta_seconds: f32) {
        let delta_seconds = delta_seconds.clamp(0.0, Self::MAX_FRAME_DELTA_SECONDS);
        let mut time = self.world.resource_mut::<Time>();
        time.tick += 1;
        time.delta_seconds = delta_seconds;
        time.elapsed_seconds += delta_seconds;
    }

    /// Number of live presentation entities.
    pub fn tank_count(&self) -> usize {
        self.entities.len()
    }

    /// Project the latest tank set into the world: spawn new ids, update existing entities in
    /// place (stable `Entity`), and despawn entities whose tank is no longer present. Each tank
    /// arrives with its tick-domain [`TankMotion`] — the attitude cues must never re-derive
    /// motion from the presented positions (see `attitude.rs` for why).
    pub fn sync_tanks(&mut self, tanks: &[(TankSnapshot, TankMotion)]) {
        let dt = self.time().delta_seconds;
        for (tank, motion) in tanks {
            let bundle = (
                TankEntity { id: tank.tank_id },
                RenderTransform { translation: tank.position, hull_yaw_rad: tank.yaw_rad },
                TurretYaw(tank.turret_yaw_rad),
                GunPitch(tank.gun_pitch_rad),
                Vehicle(tank.vehicle),
                Team(tank.team),
                Health { hit_points: tank.hit_points },
                DestroyedModules(tank.destroyed_modules_mask),
                Spotted(tank.spotted_by_teams_mask),
                ModuleHitPoints(tank.module_hit_points),
                VehicleDamage {
                    mask: tank.track_damage_mask,
                    track_hp: tank.track_hp,
                    break_t: tank.track_break_t,
                    armor_breaches: tank.armor_breaches.clone(),
                    engine_fire: tank.engine_fire,
                    fuel_fire: tank.fuel_fire,
                },
            );
            // Track distance is accumulated from the pose *delta*, so it must be folded in before
            // the bundle overwrites the previous `RenderTransform`. Kept out of the bundle so it
            // persists and accumulates rather than resetting each frame.
            // The gauge the belts scroll on is where the tracks actually ARE — the contact
            // footprint's centre line, from the same blueprint the running gear is placed by. It
            // used to read the hitbox half-width, which on a T-54 is 1.75 m against a real 1.32 m
            // centre line: a third too wide, so the inner and outer belts disagreed by a third too
            // much through every turn.
            let half_gauge = game_core::ContactFootprint::for_vehicle(tank.vehicle).half_gauge_x;
            // The spring's base is the authoritative replicated attitude plus the
            // thrown-track lean — see `sync_cues` for the derivation.
            let sample = attitude_sample(tank);
            let suspension = suspension_pool_fraction(tank);
            match self.entities.get(&tank.tank_id) {
                Some(&entity) => {
                    let mut anim =
                        self.world.get::<TrackAnimation>(entity).copied().unwrap_or_default();
                    let damage = TrackDamageMask::from_bits(tank.track_damage_mask);
                    anim.accumulate(tank.position, tank.yaw_rad, half_gauge, damage);
                    let mut attitude =
                        self.world.get::<HullAttitude>(entity).copied().unwrap_or_default();
                    attitude.step(tank.position, *motion, sample, suspension, dt);
                    let mut recoil =
                        self.world.get::<GunRecoil>(entity).copied().unwrap_or_default();
                    recoil.step(dt);
                    let mut jerk =
                        self.world.get::<TurretJerk>(entity).copied().unwrap_or_default();
                    jerk.step(dt);
                    self.world.entity_mut(entity).insert(bundle);
                    self.world.entity_mut(entity).insert(anim);
                    self.world.entity_mut(entity).insert(attitude);
                    self.world.entity_mut(entity).insert(recoil);
                    self.world.entity_mut(entity).insert(jerk);
                }
                None => {
                    let mut anim = TrackAnimation::default();
                    let damage = TrackDamageMask::from_bits(tank.track_damage_mask);
                    anim.accumulate(tank.position, tank.yaw_rad, half_gauge, damage);
                    let mut attitude = HullAttitude::default();
                    attitude.step(tank.position, *motion, sample, suspension, dt);
                    let entity = self.world.spawn(bundle).id();
                    self.world.entity_mut(entity).insert(anim);
                    self.world.entity_mut(entity).insert(attitude);
                    self.world.entity_mut(entity).insert(GunRecoil::default());
                    self.world.entity_mut(entity).insert(TurretJerk::default());
                    self.entities.insert(tank.tank_id, entity);
                }
            }
        }
        self.despawn_missing(tanks);
    }

    /// The live entity for a networked tank id, if the world has seen it.
    pub(crate) fn entity_of(&self, tank_id: TankId) -> Option<Entity> {
        self.entities.get(&tank_id).copied()
    }

    /// One tank's sprung presentation attitude — `(spring pitch, heave)` — for the camera's
    /// feel layer (Immersja B1): the camera rides a fraction of the same spring the rendered
    /// hull rides, instead of standing on a tripod while the hull nods. Read-only by design:
    /// presentation cues never flow back into the springs, and the camera must never
    /// re-derive motion from presented poses.
    pub fn sprung_attitude(&self, tank_id: TankId) -> Option<(f32, f32)> {
        let entity = self.entity_of(tank_id)?;
        self.world
            .get::<HullAttitude>(entity)
            .map(|attitude| (attitude.pitch_rad, attitude.heave_m))
    }

    /// Mutable access to the raw ECS world for sibling presentation modules (`sync_cues`).
    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    fn despawn_missing(&mut self, tanks: &[(TankSnapshot, TankMotion)]) {
        let gone: Vec<TankId> = self
            .entities
            .keys()
            .copied()
            .filter(|id| !tanks.iter().any(|(tank, _)| tank.tank_id == *id))
            .collect();
        for id in gone {
            if let Some(entity) = self.entities.remove(&id) {
                self.world.despawn(entity);
            }
        }
    }

    /// Extract the presentation view of every live tank, ordered by id for deterministic rendering
    /// and tests.
    pub fn presentation_tanks(&mut self) -> Vec<PresentationTank> {
        let mut query = self.world.query::<(
            &TankEntity,
            &RenderTransform,
            &TurretYaw,
            &GunPitch,
            &Vehicle,
            &Team,
            &Health,
            &DestroyedModules,
            &Spotted,
            &ModuleHitPoints,
            &VehicleDamage,
            &TrackAnimation,
            &HullAttitude,
            &GunRecoil,
            &TurretJerk,
        )>();
        let mut tanks: Vec<PresentationTank> = query
            .iter(&self.world)
            .map(
                |(
                    entity,
                    transform,
                    turret,
                    pitch,
                    vehicle,
                    team,
                    health,
                    destroyed,
                    spotted,
                    modules,
                    damage,
                    track,
                    attitude,
                    recoil,
                    jerk,
                )| {
                    PresentationTank {
                        id: entity.id,
                        team: team.0,
                        vehicle: vehicle.0,
                        translation: transform.translation,
                        hull_yaw_rad: transform.hull_yaw_rad,
                        turret_yaw_rad: turret.0 + jerk.offset_rad,
                        gun_pitch_rad: pitch.0,
                        hit_points: health.hit_points,
                        destroyed_modules_mask: destroyed.0,
                        spotted_by_teams_mask: spotted.0,
                        module_hit_points: modules.0,
                        track_damage_mask: damage.mask,
                        track_hp: damage.track_hp,
                        track_break_t: damage.break_t,
                        engine_fire: damage.engine_fire,
                        fuel_fire: damage.fuel_fire,
                        armor_breaches: damage.armor_breaches.clone(),
                        track_left_m: track.left_m,
                        track_right_m: track.right_m,
                        attitude_pitch_rad: attitude.pitch_rad,
                        attitude_roll_rad: attitude.roll_rad,
                        attitude_heave_m: attitude.heave_m,
                        accel_long_mps2: attitude.accel_long_mps2,
                        gun_recoil_m: recoil.offset_m,
                    }
                },
            )
            .collect();
        tanks.sort_by_key(|tank| tank.id.0);
        tanks
    }
}
