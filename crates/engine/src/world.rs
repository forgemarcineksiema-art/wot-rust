use std::collections::HashMap;

use bevy_ecs::prelude::*;
use game_core::TankId;
use net::TankSnapshot;

use crate::components::{
    DestroyedModules, GunPitch, Health, PresentationTank, RenderTransform, TankEntity, Time,
    TurretYaw, Vehicle,
};

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

    /// Advance the render clock by one presented frame.
    pub fn advance_time(&mut self, delta_seconds: f32) {
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
    /// place (stable `Entity`), and despawn entities whose tank is no longer present.
    pub fn sync_tanks(&mut self, tanks: &[TankSnapshot]) {
        for tank in tanks {
            let bundle = (
                TankEntity { id: tank.tank_id },
                RenderTransform { translation: tank.position, hull_yaw_rad: tank.yaw_rad },
                TurretYaw(tank.turret_yaw_rad),
                GunPitch(tank.gun_pitch_rad),
                Vehicle(tank.vehicle),
                Health { hit_points: tank.hit_points },
                DestroyedModules(tank.destroyed_modules_mask),
            );
            match self.entities.get(&tank.tank_id) {
                Some(&entity) => {
                    self.world.entity_mut(entity).insert(bundle);
                }
                None => {
                    let entity = self.world.spawn(bundle).id();
                    self.entities.insert(tank.tank_id, entity);
                }
            }
        }
        self.despawn_missing(tanks);
    }

    fn despawn_missing(&mut self, tanks: &[TankSnapshot]) {
        let gone: Vec<TankId> = self
            .entities
            .keys()
            .copied()
            .filter(|id| !tanks.iter().any(|tank| tank.tank_id == *id))
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
            &Health,
            &DestroyedModules,
        )>();
        let mut tanks: Vec<PresentationTank> = query
            .iter(&self.world)
            .map(|(entity, transform, turret, pitch, vehicle, health, destroyed)| {
                PresentationTank {
                    id: entity.id,
                    vehicle: vehicle.0,
                    translation: transform.translation,
                    hull_yaw_rad: transform.hull_yaw_rad,
                    turret_yaw_rad: turret.0,
                    gun_pitch_rad: pitch.0,
                    hit_points: health.hit_points,
                    destroyed_modules_mask: destroyed.0,
                }
            })
            .collect();
        tanks.sort_by_key(|tank| tank.id.0);
        tanks
    }
}
