//! Per-frame upkeep of the battle-scar state: ageing hit decals, dropping scars of despawned
//! tanks, and the dead-engine smoke column — the local, causal damage cues that replaced the
//! old whole-submesh damage tint.

use game_core::{HitboxProfile, ModuleSlot};
use glam::Vec3;
use renderer_api::FxVertex;

use super::ClientApp;

/// Seconds between smoke puffs from a knocked-out engine: a steady column, not a wall.
const ENGINE_SMOKE_PERIOD_S: f32 = 0.16;

impl ClientApp {
    /// Age every tank's scars and run the damage-driven emitters (engine smoke). Called once per
    /// presented frame from the render loop.
    pub(super) fn tick_battle_scars(&mut self, dt: f32) {
        for variation in self.tank_scars.values_mut() {
            variation.tick(dt);
        }
        let tanks = self.render_state.interpolated_tanks();
        self.tank_scars.retain(|id, _| tanks.iter().any(|tank| tank.tank_id == *id));
        self.engine_smoke_accum_s.retain(|id, _| tanks.iter().any(|tank| tank.tank_id == *id));

        for tank in &tanks {
            let engine_dead =
                tank.destroyed_modules_mask & ModuleSlot::Engine.destroyed_mask_bit() != 0;
            if !engine_dead || tank.hit_points == 0 {
                continue;
            }
            let accum = self.engine_smoke_accum_s.entry(tank.tank_id).or_insert(0.0);
            *accum += dt;
            while *accum >= ENGINE_SMOKE_PERIOD_S {
                *accum -= ENGINE_SMOKE_PERIOD_S;
                self.fx.engine_smoke_puff(engine_deck_world(tank));
            }
        }
    }

    /// Append every tank's accumulated hit decals to this frame's FX batch.
    pub(super) fn append_scar_quads(&self, vertices: &mut Vec<FxVertex>) {
        for tank in &self.render_state.interpolated_tanks() {
            if let Some(variation) = self.tank_scars.get(&tank.tank_id) {
                crate::fx::append_decal_quads(vertices, variation.decals(), tank);
            }
        }
    }
}

/// Where a dead engine smokes from: over the rear deck, at deck height. A diffuse cloud does not
/// need the sprung attitude — hull yaw and position place it within centimeters.
fn engine_deck_world(tank: &net::TankSnapshot) -> Vec3 {
    let hitbox = HitboxProfile::for_vehicle(tank.vehicle);
    let local =
        Vec3::new(0.0, hitbox.turret_min_y_m + hitbox.center_y_m, -hitbox.half_length_m * 0.6);
    let (sin, cos) = tank.yaw_rad.sin_cos();
    Vec3::from_array(tank.position)
        + Vec3::new(cos * local.x + sin * local.z, local.y, -sin * local.x + cos * local.z)
}

#[cfg(test)]
mod tests {
    use game_core::ModuleSlot;

    use super::super::ClientApp;

    #[test]
    fn a_dead_engine_smokes_and_a_dead_tank_does_not() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(6);

        // Mark the practice target's engine destroyed in the latest snapshot.
        let mut snapshot = app.render_state.latest_snapshot().cloned().expect("snapshot present");
        snapshot.server_tick += 1;
        let target =
            snapshot.tanks.iter_mut().find(|tank| tank.tank_id != app.player_tank).expect("target");
        target.destroyed_modules_mask |= ModuleSlot::Engine.destroyed_mask_bit();
        let target_id = target.tank_id;
        app.accept_and_sync(snapshot);

        app.tick_battle_scars(1.0); // several smoke periods
        let smoking = app.fx.live_particles();
        assert!(smoking >= 5, "a knocked-out engine smokes steadily, got {smoking}");

        // Kill the tank outright: the wreck tint takes over and the column stops growing.
        let mut snapshot = app.render_state.latest_snapshot().cloned().expect("snapshot");
        snapshot.server_tick += 1;
        snapshot
            .tanks
            .iter_mut()
            .find(|tank| tank.tank_id == target_id)
            .expect("target")
            .hit_points = 0;
        app.accept_and_sync(snapshot);
        app.fx = crate::fx::FxSystem::default();
        app.tick_battle_scars(1.0);
        assert_eq!(app.fx.live_particles(), 0, "a wreck's engine no longer smokes");
    }

    #[test]
    fn a_penetrated_wreck_gets_a_dented_per_instance_hull() {
        use game_core::VehicleKind;
        use glam::Vec3;
        use renderer_api::{MaterialHandle, MeshHandle, RenderObject};

        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(6);

        // Record a hull penetration on the target, then kill it in the next snapshot.
        let mut snapshot = app.render_state.latest_snapshot().cloned().expect("snapshot present");
        snapshot.server_tick += 1;
        let target =
            snapshot.tanks.iter_mut().find(|tank| tank.tank_id != app.player_tank).expect("target");
        let target_id = target.tank_id;
        let hit = glam::Vec3::from_array(target.position) + Vec3::new(0.0, 1.2, 1.0);
        snapshot.damage_events.push(game_core::DamageEvent {
            source: app.player_tank,
            target: target_id,
            hit_position: hit,
            damage_hp: 400,
            penetrated: true,
            cause: game_core::DamageCause::Shell,
            armor_zone: game_core::ArmorZone::UpperGlacis,
            ..Default::default()
        });
        target.hit_points = 0;
        app.accept_and_sync(snapshot);

        // The wreck now owns a dented hull handle distinct from the shared base hull mesh.
        let handle =
            app.wreck_hull_meshes.get(&target_id).copied().expect("wreck has a dented hull");
        let base = app.vehicle_asset_catalog.vehicle_entry(VehicleKind::T54_1951).map(|e| e.hull);
        if let Some(base) = base {
            assert_ne!(handle, base, "the dented wreck hull is a distinct per-instance mesh");
        }

        // The render override points the wreck's hull object at the dented handle.
        let mut objects = vec![RenderObject {
            tank_id: Some(target_id),
            mesh: MeshHandle(999),
            material: MaterialHandle(0),
            transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
            tint: [0.3, 0.3, 0.3],
        }];
        app.apply_wreck_deform(&mut objects);
        assert_eq!(objects[0].mesh, handle, "the wreck's hull object draws the dented mesh");
    }

    #[test]
    fn scars_of_despawned_tanks_are_dropped() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(6);
        app.tank_scars.insert(game_core::TankId(4242), Default::default());

        app.tick_battle_scars(1.0 / 60.0);

        assert!(!app.tank_scars.contains_key(&game_core::TankId(4242)));
    }
}
