//! Snapshot ingest: fanning one authoritative snapshot out to every consumer — feedback feeds,
//! FX, scars, the camera shudder, the kill confirmation, the render buffer and the predictor.
//! Split from `prediction.rs` for the reviewability budget.

use super::ClientApp;

impl ClientApp {
    pub(super) fn accept_and_sync(&mut self, snapshot: net::Snapshot) {
        let player = snapshot.tanks.iter().find(|tank| tank.tank_id == self.player_tank).cloned();
        self.hit_indicator.ingest_damage_events(&snapshot.damage_events, self.player_tank);
        self.damage_log.ingest(&snapshot.damage_events, self.player_tank, &snapshot.tanks);
        self.incoming_hits.ingest(&snapshot.damage_events, self.player_tank, &snapshot.tanks);
        // Feel the hit, not just read it: every incoming strike rocks the camera rig, scaled by
        // how much of the health pool it took (a bounce still lands a small clang).
        let full_hp = self.player_max_hit_points().max(1) as f32;
        for event in &snapshot.damage_events {
            if event.target == self.player_tank && event.source != self.player_tank {
                let push = self.predictor.position() - event.hit_position;
                self.camera_controller.damage_shudder(push, event.damage_hp as f32 / full_hp);
            }
        }
        // Every shell death gets its world-space burst: absorbed shells speak the surface they
        // died against, armor strikes answer with sparks (plus the penetration signature). A
        // shell the ground swallowed also digs a crater that outlives the dust.
        for impact in &snapshot.shell_impacts {
            self.fx.impact_burst(impact.position, impact.surface);
            if impact.surface == game_core::ImpactSurface::Terrain {
                self.terrain_scars.record(impact.position, &self.battlefield.heightmap);
            }
        }
        for event in &snapshot.damage_events {
            if event.cause != game_core::DamageCause::Shell {
                continue;
            }
            self.fx.armor_hit(event.hit_position, event.penetrated, event.ricocheted);
            // The strike also scars the target: a permanent hole for a penetration, a fading
            // scuff/gouge otherwise, recorded in the plate's own rotating frame.
            if let Some(target) = snapshot.tanks.iter().find(|tank| tank.tank_id == event.target)
                && let Some(decal) = crate::fx::decal_from_damage_event(event, target)
            {
                self.tank_scars.entry(event.target).or_default().record_hit(decal);
            }
        }
        // Shots fired since the previous snapshot: diffed here, where both snapshots exist side
        // by side, then fanned out to every fire cue (muzzle FX, recoil, hull rock, camera kick).
        let fired = self.render_state.latest_snapshot().map_or_else(Vec::new, |previous| {
            crate::fx::detect_fired(
                &previous.tanks,
                &snapshot.tanks,
                self.player_tank,
                self.player_barrel_scale(),
            )
        });
        // The payoff beat: a vehicle the player damaged died in this snapshot.
        if crate::hud::kill_marker::player_scored_kill(
            self.render_state.latest_snapshot(),
            &snapshot,
            self.player_tank,
        ) {
            self.kill_confirm_age_s = Some(0.0);
        }
        self.render_state.accept_authoritative_snapshot(snapshot);
        self.apply_fire_events(&fired);
        if let Some(tank) = player {
            self.predictor.sync_to(&tank);
        }
    }

    /// Fan one batch of replicated shots out to the presentation cues. Every firing tank gets
    /// muzzle FX, barrel recoil, and the hull rock; the player's own shot also kicks the camera.
    fn apply_fire_events(&mut self, events: &[crate::fx::FireEvent]) {
        for event in events {
            let ground_y = self.battlefield.heightmap.sample_height(event.muzzle.x, event.muzzle.z);
            self.fx.muzzle_blast(event.muzzle, event.direction, ground_y);
            self.presentation.apply_fire_recoil(event.tank_id, event.turret_yaw_rad);
            if event.tank_id == self.player_tank {
                self.camera_controller.fire_kick(self.desired_aim.yaw_rad());
            }
        }
    }
}
