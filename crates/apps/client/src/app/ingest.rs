//! Snapshot ingest: fanning one authoritative snapshot out to every consumer — feedback feeds,
//! FX, scars, the camera shudder, the kill confirmation, the render buffer and the predictor.
//! Split from `prediction.rs` for the reviewability budget.

use super::ClientApp;

impl ClientApp {
    pub(super) fn accept_and_sync(&mut self, snapshot: net::Snapshot) {
        let player = snapshot.tanks.iter().find(|tank| tank.tank_id == self.player_tank).cloned();
        for tank in &snapshot.tanks {
            self.tank_scars.entry(tank.tank_id).or_default().sync_from_snapshot(tank);
        }
        self.hit_indicator.ingest_damage_events(&snapshot.damage_events, self.player_tank);
        self.damage_log.ingest(&snapshot.damage_events, self.player_tank, &snapshot.tanks);
        self.track_feedback.ingest(&snapshot.damage_events, self.player_tank);
        // Drive the re-seat bars off the player's own replicated broken mask.
        if let Some(player) = &player {
            let mask = game_core::TrackDamageMask::from_bits(player.track_damage_mask);
            self.track_feedback.sync_player_broken(
                mask.is_broken(game_core::TrackSide::Left),
                mask.is_broken(game_core::TrackSide::Right),
            );
        }
        self.incoming_hits.ingest(&snapshot.damage_events, self.player_tank, &snapshot.tanks);
        // Feel the hit, not just read it: every incoming strike rocks the camera rig, scaled by
        // how much of the health pool it took (a bounce still lands a small clang).
        let full_hp = self.player_max_hit_points().max(1) as f32;
        for event in &snapshot.damage_events {
            if event.target == self.player_tank && event.source != self.player_tank {
                let push = self.predictor.position() - event.hit_position;
                self.camera_controller.damage_shudder(push, event.damage_hp as f32 / full_hp);
            }
            // A track the player took OR dealt speaks its own metallic voice — a snap when it
            // throws, a grind when it only bites — distinct from the plate clang, and heard even
            // on a clean 0-HP track break the armor clang would otherwise skip.
            if let Some(hit) = event.track_hit
                && (event.target == self.player_tank || event.source == self.player_tank)
            {
                self.queue_audio(audio::AudioEvent::TrackSnapped {
                    position: event.hit_position,
                    broken: hit.broke,
                });
            }
        }
        // Every shell death gets its world-space burst: absorbed shells speak the surface they
        // died against, armor strikes answer with sparks (plus the penetration signature). A
        // shell the ground swallowed also digs a crater that outlives the dust.
        for impact in &snapshot.shell_impacts {
            self.fx.impact_burst(impact.position, impact.surface);
            // The same shell death also speaks: soil swallows, structures and hulls knock, and
            // a high-explosive round (protocol v17 carries the type) detonates instead.
            self.queue_audio(audio::AudioEvent::ShellAbsorbed {
                position: impact.position,
                surface: match impact.surface {
                    game_core::ImpactSurface::Terrain => audio::GroundKind::Soil,
                    _ => audio::GroundKind::Structure,
                },
                high_explosive: impact.shell_type == game_core::ShellType::HighExplosive,
            });
            if impact.surface == game_core::ImpactSurface::Terrain {
                self.terrain_scars.record(impact.position, &self.battlefield.heightmap);
            }
            // The blast grammar (D2): a NEAR HE detonation reads through the rig even when it
            // scratches nothing — the world shoves the camera away from the burst, scaled by
            // proximity. Direct hits keep their own, stronger shudder; a far crump stays a
            // picture. Sniper mode keeps only the vertical dip (damage_shudder's own rule).
            if impact.shell_type == game_core::ShellType::HighExplosive
                && let Some(player) = snapshot.tanks.iter().find(|t| t.tank_id == self.player_tank)
            {
                const BLAST_READ_RANGE_M: f32 = 30.0;
                let away = glam::Vec3::from_array(player.position) - impact.position;
                let distance = away.length();
                if distance < BLAST_READ_RANGE_M && distance > 1.0e-3 {
                    let proximity = 1.0 - distance / BLAST_READ_RANGE_M;
                    self.camera_controller.damage_shudder(away, proximity * proximity * 0.8);
                }
            }
        }
        for event in &snapshot.damage_events {
            // Splash strikes (HE blast damage without the shell body) still detonate audibly.
            if event.cause == game_core::DamageCause::Splash {
                self.queue_audio(audio::AudioEvent::ArmorStruck {
                    position: event.hit_position,
                    penetrated: false,
                    ricocheted: false,
                    high_explosive: true,
                });
            }
            if event.cause != game_core::DamageCause::Shell {
                continue;
            }
            self.fx.armor_hit(event.hit_position, event.penetrated, event.ricocheted);
            // And rings: the struck plate's clang (or the HE charge's own burst), with the
            // outcome (penetration thunk / ricochet whine) layered in by the voice itself.
            self.queue_audio(audio::AudioEvent::ArmorStruck {
                position: event.hit_position,
                penetrated: event.penetrated,
                ricocheted: event.ricocheted,
                high_explosive: event.shell_type == game_core::ShellType::HighExplosive,
            });
            // The strike also scars the target: a permanent hole for a penetration, a fading
            // scuff/gouge otherwise, recorded in the plate's own rotating frame and seated on the
            // target's visual armor via its cached mesh-contact index.
            if let Some(target) = snapshot.tanks.iter().find(|tank| tank.tank_id == event.target) {
                let contact = self.vehicle_asset_catalog.contact_index(target.vehicle);
                if let Some(decal) =
                    crate::fx::decal_from_damage_event(event, target, contact.as_deref())
                {
                    self.tank_scars.entry(event.target).or_default().record_hit(decal);
                }
            }
        }
        // A freshly knocked-out hull is DENTED where it was hit: build its per-instance deformed
        // hull once from the penetrations already recorded above, and forget despawned wrecks.
        self.sync_wreck_deform(&snapshot);
        // Static cover that collapsed or was cleared this snapshot: burst dust at each newly
        // destroyed object and flag the scene for a rebuild (buildings -> rubble, foliage -> gone).
        self.sync_cover_destruction(&snapshot);
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
        // A decapitated wreck (ammo-rack detonation, protocol v20): start its flying-turret arc
        // once, with a burst at the ring, and forget wrecks that have despawned.
        self.sync_turret_popoffs(&snapshot);
        // The payoff beat: a vehicle the player damaged died in this snapshot.
        if crate::hud::kill_marker::player_scored_kill(
            self.render_state.latest_snapshot(),
            &snapshot,
            self.player_tank,
        ) {
            self.kill_confirm_age_s = Some(0.0);
            self.queue_audio(audio::AudioEvent::KillConfirmed);
        }
        self.render_state.accept_authoritative_snapshot(snapshot);
        // The remote interpolation phase restarts with the window it measures.
        self.ticks_since_snapshot = 0;
        self.apply_fire_events(&fired);
        if let Some(tank) = player {
            self.predictor.sync_to(&tank);
        }
    }

    /// Start a flying-turret animation for every newly-decapitated wreck and drop the ones whose
    /// wreck has despawned. The arc is deterministic in `(tank_id, ring)`, so no transform crosses
    /// the wire; the ring position is read from the wreck's replicated pose at detonation.
    fn sync_turret_popoffs(&mut self, snapshot: &net::Snapshot) {
        for &id in &snapshot.detached_turrets {
            if self.turret_popoffs.contains_key(&id) {
                continue;
            }
            let Some(tank) = snapshot.tanks.iter().find(|tank| tank.tank_id == id) else {
                continue;
            };
            let ring = crate::vehicle::pose::VehiclePose::from_snapshot(tank).turret_translation();
            let popoff = crate::vehicle::turret_popoff::TurretPopoff::launch(
                id,
                tank.vehicle,
                ring,
                Some(&self.battlefield.heightmap),
            );
            self.turret_popoffs.insert(id, popoff);
            // The kaboom at the ring: a spark-and-flash burst, a puff of smoke, a ring of dust.
            self.fx.armor_hit(ring, true, false);
            self.fx.engine_smoke_puff(ring);
            self.fx.track_dust(ring);
        }
        self.turret_popoffs.retain(|id, _| snapshot.tanks.iter().any(|tank| tank.tank_id == *id));
    }

    /// Build the dented hull for each freshly-knocked-out wreck from its recorded hull-frame
    /// penetrations, once, and drop wrecks that have despawned. Presentation only — the deform
    /// kernel never touches the hitbox (see `vehicle::wreck_deform`).
    fn sync_wreck_deform(&mut self, snapshot: &net::Snapshot) {
        use crate::vehicle::variation::{DecalFrame, DecalKind};

        for tank in &snapshot.tanks {
            if tank.hit_points != 0 || self.wreck_hull_meshes.contains_key(&tank.tank_id) {
                continue;
            }
            let penetrations: Vec<glam::Vec3> = self
                .tank_scars
                .get(&tank.tank_id)
                .map(|variation| {
                    variation
                        .decals()
                        .iter()
                        .filter(|d| d.kind == DecalKind::Penetration && d.frame == DecalFrame::Hull)
                        .map(|d| glam::Vec3::from_array(d.local_position))
                        .collect()
                })
                .unwrap_or_default();
            if penetrations.is_empty() {
                continue;
            }
            if let Some(handle) = self.vehicle_asset_catalog.wreck_hull_mesh(
                tank.vehicle,
                tank.tank_id,
                &penetrations,
            ) {
                self.wreck_hull_meshes.insert(tank.tank_id, handle);
            }
        }
        self.wreck_hull_meshes.retain(|id, _| snapshot.tanks.iter().any(|t| t.tank_id == *id));
    }

    /// React to the replicated cover phases (protocol v21): the first snapshot seeds the baseline
    /// silently; afterwards any object that stepped up in destruction (intact -> rubble/gone, or
    /// rubble -> gone) bursts dust and flags the scene for a rebuild so the collapse actually shows.
    fn sync_cover_destruction(&mut self, snapshot: &net::Snapshot) {
        let states = &snapshot.cover_states;
        if states == &self.cover_phase_bytes {
            return;
        }
        // Seed silently on the first sight (or a mid-battle map swap changing the count): no phantom
        // collapses of cover that was already down when we joined.
        let seeding = self.cover_phase_bytes.len() != states.len();
        if !seeding {
            for (index, &phase) in states.iter().enumerate() {
                let was = self.cover_phase_bytes.get(index).copied().unwrap_or(0);
                if phase > was
                    && let Some(object) = self.battlefield.static_cover.get(index)
                {
                    let center = glam::Vec3::from_array(object.center);
                    // Masonry/timber dust at the wreck of the object.
                    self.fx.impact_burst(center, game_core::ImpactSurface::Cover);
                    self.fx.track_dust(center);
                }
            }
        }
        self.cover_phase_bytes = states.clone();
        self.scene_cover_dirty = true;
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
            // The report, sized by the firing gun's caliber. The player's own shot answers the
            // trigger instantly; everyone else's arrives at the speed of sound.
            let caliber_mm = self
                .render_state
                .latest_snapshot()
                .and_then(|snapshot| {
                    snapshot.tanks.iter().find(|tank| tank.tank_id == event.tank_id)
                })
                .map_or(100.0, |tank| tank.vehicle.spec().gun.shell.caliber_mm);
            self.queue_audio(audio::AudioEvent::CannonFired {
                position: event.muzzle,
                caliber_mm,
                own_shot: event.tank_id == self.player_tank,
            });
        }
    }
}

#[cfg(test)]
mod blast_grammar_tests {
    use super::super::ClientApp;

    /// D2's contract: a NEAR high-explosive burst shoves the rig away from the detonation even
    /// when it damages nothing; a far crump and a kinetic slap near you stay pictures.
    #[test]
    fn a_near_he_burst_shoves_the_camera_and_a_far_or_kinetic_one_does_not() {
        let shove = |offset: [f32; 3], shell_type: game_core::ShellType| {
            let mut app = ClientApp::new_seeded(11);
            app.confirm_garage_selection();
            app.run_fixed_ticks(6);
            let mut snapshot =
                app.render_state.latest_snapshot().cloned().expect("snapshot present");
            snapshot.server_tick += 1;
            let player = snapshot
                .tanks
                .iter()
                .find(|tank| tank.tank_id == app.player_tank)
                .expect("player present")
                .position;
            app.camera_controller.zero_motion_for_test();
            snapshot.shell_impacts.push(game_core::ShellImpact {
                owner: game_core::TankId(999),
                position: glam::Vec3::from_array(player) + glam::Vec3::from_array(offset),
                surface: game_core::ImpactSurface::Terrain,
                shell_type,
            });
            app.accept_and_sync(snapshot);
            app.camera_controller.anchor_speed_for_test()
        };

        let near_he = shove([6.0, 0.0, 3.0], game_core::ShellType::HighExplosive);
        assert!(near_he > 0.3, "a 7 m HE burst must shove the rig, got {near_he}");
        let far_he = shove([80.0, 0.0, 10.0], game_core::ShellType::HighExplosive);
        assert!(far_he < 1.0e-3, "an 80 m crump is a picture, got {far_he}");
        let near_ap = shove([6.0, 0.0, 3.0], game_core::ShellType::ArmorPiercing);
        assert!(near_ap < 1.0e-3, "a kinetic slap into soil is not a blast, got {near_ap}");
    }
}
