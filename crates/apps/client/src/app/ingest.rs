//! Snapshot ingest: fanning one authoritative snapshot out to every consumer — feedback feeds,
//! FX, scars, the camera shudder, the kill confirmation, the render buffer and the predictor.
//! Split from `prediction.rs` for the reviewability budget.

use super::ClientApp;

impl ClientApp {
    pub(super) fn accept_and_sync(&mut self, snapshot: net::Snapshot) {
        self.accept_and_sync_with(snapshot, None);
    }

    pub(super) fn accept_remote_and_sync(
        &mut self,
        snapshot: net::Snapshot,
        reconciliation: super::session::RemoteReconciliation,
    ) {
        self.accept_and_sync_with(snapshot, Some(reconciliation));
    }

    /// Fold this tick's replicated perforations into the client's own per-hull sets (protocol
    /// v39). Applying them through the SAME `ArmorBreachSet::add` the authoritative simulation
    /// ran — on the same values, in the order the reliable lane preserved — is what makes the
    /// two sides converge, merges and capacity evictions included.
    pub(super) fn apply_armor_breach_deltas(&mut self, deltas: Vec<net::ArmorBreachDelta>) {
        for delta in deltas {
            self.armor_breaches.apply(delta.tank, delta.breach);
        }
    }

    fn accept_and_sync_with(
        &mut self,
        mut snapshot: net::Snapshot,
        reconciliation: Option<super::session::RemoteReconciliation>,
    ) {
        // Perforations are client-local presentation state now: dress each hull from the set the
        // reliable lane has been building, since the snapshot itself no longer carries them.
        for tank in &mut snapshot.tanks {
            tank.armor_breaches = self.armor_breaches.get(tank.tank_id);
        }
        let live: Vec<game_core::TankId> = snapshot.tanks.iter().map(|tank| tank.tank_id).collect();
        self.armor_breaches.retain_live(&live);
        let snapshot = snapshot;
        // The replicated crater ledger (protocol v31) folds into OUR heightmap FIRST, before
        // any consumer below reads the ground: the terrain scars of this very snapshot's
        // impacts must drape onto the deformed field (a scorch floating over the bowl it
        // belongs to reads as a bug), and the predictor, camera probe and ribbons all stand
        // in the same holes the server does. Idempotent — an unchanged ledger costs one
        // compare. A moved ledger also flags the ground re-mesh (P4b).
        if self.battlefield.heightmap.crater_records() != snapshot.craters.as_slice() {
            // Copy-on-write: with no bake in flight this is an in-place mutation of a uniquely
            // held map (the common case, and cheaper than the unconditional deep clone the
            // worker handoff used to pay). While a bake IS running it forks once, and that
            // worker finishes against the ledger it was handed — the dirty flag re-fires for
            // the newer one, exactly as it did before.
            std::sync::Arc::make_mut(&mut self.battlefield)
                .heightmap
                .set_craters(&snapshot.craters);
            self.ground_deform_dirty = true;
        }
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
            // A thrown track sheds its ribbon onto the field (D6): posed once, deterministic,
            // budgeted — the oldest shed steel is recycled, and one side sheds only once.
            if let Some(hit) = event.track_hit
                && hit.broke
                && let Some(target) = snapshot.tanks.iter().find(|t| t.tank_id == event.target)
                && !self
                    .track_ribbons
                    .iter()
                    .any(|r| r.tank_id == event.target && r.side == hit.side)
            {
                let ribbon = crate::vehicle::track_ribbon::TrackRibbon::shed(
                    event.target,
                    target.vehicle,
                    hit.side,
                    glam::Vec3::from_array(target.position),
                    target.yaw_rad,
                    Some(&self.battlefield.heightmap),
                );
                if self.track_ribbons.len() >= crate::vehicle::track_ribbon::MAX_TRACK_RIBBONS {
                    self.track_ribbons.remove(0);
                }
                self.track_ribbons.push(ribbon);
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
                self.terrain_scars.record(impact, &self.battlefield.heightmap);
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
            // A ricochet's sparks leave ALONG the deflection — the wire has carried the
            // plate normal and shell direction since v19; the fan finally reads them (D6).
            let departure = if event.ricocheted {
                let d = event.shell_direction;
                let n = event.plate_normal;
                Some(d - n * (2.0 * d.dot(n)))
            } else {
                None
            };
            self.fx.armor_hit_directed(
                event.hit_position,
                event.penetrated,
                event.ricocheted,
                departure,
            );
            // The hit in the body (Inny Poziom S5): the struck hull rocks on its springs from
            // the side the shell came in, as hard as the shooter's round pushes on the S3
            // momentum scale — a bounce as hard as a penetration, an HE charge a third more.
            if let Some(target) = snapshot.tanks.iter().find(|t| t.tank_id == event.target) {
                let to_hit = event.hit_position - glam::Vec3::from_array(target.position);
                let (sin_yaw, cos_yaw) = target.yaw_rad.sin_cos();
                let local_x = to_hit.x * cos_yaw - to_hit.z * sin_yaw;
                let local_z = to_hit.x * sin_yaw + to_hit.z * cos_yaw;
                let bearing = local_x.atan2(local_z);
                let round = snapshot
                    .tanks
                    .iter()
                    .find(|t| t.tank_id == event.source)
                    .map_or(1.0, |shooter| shooter.vehicle.spec().gun.shell.recoil_scale());
                let charge =
                    if event.shell_type == game_core::ShellType::HighExplosive { 1.3 } else { 1.0 };
                self.presentation.apply_hit_impulse(event.target, bearing, round * charge);
            }
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
        // Shots the server says were fired this tick (protocol v41), resolved against the poses
        // that fired them and fanned out to every fire cue (muzzle FX, recoil, hull rock, camera
        // kick). No longer diffed out of two snapshots' reload clocks — a shot is a fact, and a
        // tank that fired and died in the same window used to lose its flash entirely.
        let fired = crate::fx::resolve_shots(
            &snapshot.shots_fired,
            &snapshot.tanks,
            self.player_tank,
            self.player_barrel_scale(),
        );
        // A decapitated wreck (ammo-rack detonation, protocol v20): start its flying-turret arc
        // once, with a burst at the ring, and forget wrecks that have despawned.
        self.sync_turret_popoffs(&snapshot);
        // The payoff beat: a vehicle the player damaged died in this snapshot.
        if crate::hud::kill_marker::player_scored_kill(&snapshot.damage_events, self.player_tank) {
            self.kill_confirm_age_s = Some(0.0);
            self.queue_audio(audio::AudioEvent::KillConfirmed);
        }
        self.render_state.accept_authoritative_snapshot(snapshot);
        // The remote interpolation phase restarts with the window it measures.
        self.ticks_since_snapshot = 0;
        self.apply_fire_events(&fired);
        if let Some(tank) = player {
            if let Some(reconciliation) = reconciliation {
                self.reconcile_remote_prediction(&tank, reconciliation);
            } else {
                self.predictor.sync_to(&tank);
            }
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
        // Fresh wounds on the walls (protocol v32) re-dress the statics even when no phase
        // stepped — the same rebuild the collapse takes, just with scars in the bake.
        if snapshot.cover_scars != self.cover_scar_list {
            self.cover_scar_list = snapshot.cover_scars.clone();
            self.scene_cover_dirty = true;
        }
        let Some(next_live_cover) = super::live_cover::LiveCoverCache::from_replicated(
            &self.battlefield.static_cover,
            &snapshot.cover_states,
        ) else {
            // An incomplete/default snapshot cannot turn a born ruin back into a full building.
            return;
        };
        if next_live_cover.phase_bytes() == self.live_cover.phase_bytes() {
            // Record that the born-phase bootstrap has now been confirmed by the authority.
            self.live_cover = next_live_cover;
            return;
        }
        // Seed silently on the first complete sight: no phantom collapses for a late join whose
        // opening snapshot already contains rubble.
        let seeding = !self.live_cover.is_replicated();
        if !seeding {
            for (index, &phase) in next_live_cover.phase_bytes().iter().enumerate() {
                let was = self.live_cover.phase_bytes().get(index).copied().unwrap_or(0);
                if phase > was
                    && let Some(object) = self.battlefield.static_cover.get(index)
                {
                    let center = glam::Vec3::from_array(object.center);
                    let half = glam::Vec3::from_array(object.half_extents_m);
                    // The collapse theatre (Inny Poziom Z1): a staged sequence sized by the
                    // box — curtain and chunks now, the settle wave, the haze — and the audio
                    // hit sized the same way. One burst at the centre was eleven particles for
                    // an 18 m tenement.
                    self.fx.cover_collapse(center, half);
                    self.queue_audio(audio::AudioEvent::CoverCollapse {
                        position: center,
                        footprint_m2: 4.0 * half.x * half.z,
                        height_m: 2.0 * half.y,
                    });
                }
            }
        }
        // One replacement publishes phases, blocking boxes, and camera obstacles together.
        self.live_cover = next_live_cover;
        self.scene_cover_dirty = true;
    }

    /// Fan one batch of replicated shots out to the presentation cues. Every firing tank gets
    /// muzzle FX, barrel recoil, and the hull rock; the player's own shot also kicks the camera.
    fn apply_fire_events(&mut self, events: &[crate::fx::FireEvent]) {
        for event in events {
            let ground_y = self.battlefield.heightmap.sample_height(event.muzzle.x, event.muzzle.z);
            // One recoil momentum, every channel (Inny Poziom S3): the round's scale rides the
            // event into the blast, the barrel stroke, the hull rock and the camera nudge.
            self.fx.muzzle_blast(event.muzzle, event.direction, ground_y, event.recoil_scale);
            self.fx.hull_dust(event.deck, event.hull_yaw_rad, event.recoil_scale);
            self.presentation.apply_fire_recoil(
                event.tank_id,
                event.turret_yaw_rad,
                event.recoil_scale,
            );
            if event.tank_id == self.player_tank {
                self.camera_controller.fire_kick(self.desired_aim.yaw_rad(), event.recoil_scale);
            }
            // The report, sized by the firing gun's caliber. The player's own shot answers the
            // trigger instantly; everyone else's arrives at the speed of sound.
            let caliber_mm = self
                .render_state
                .latest_snapshot()
                .and_then(|snapshot| {
                    snapshot.tanks.iter().find(|tank| tank.tank_id == event.tank_id)
                })
                .map_or(100.0, |tank| tank.vehicle.spec_ref().gun.shell.caliber_mm);
            self.queue_audio(audio::AudioEvent::CannonFired {
                position: event.muzzle,
                caliber_mm,
                own_shot: event.tank_id == self.player_tank,
            });
        }
    }
}

#[cfg(test)]
mod track_ribbon_tests {
    use super::super::ClientApp;

    /// D6's contract: a thrown track sheds one ribbon of links onto the field - once per side,
    /// deterministic, budgeted - and a mere damage tick (not a break) sheds nothing.
    #[test]
    fn a_thrown_track_sheds_a_ribbon_once_and_damage_alone_sheds_nothing() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app.run_fixed_ticks(6);

        let break_event = |app: &ClientApp, broke: bool| {
            let snapshot = app.render_state.latest_snapshot().cloned().expect("snapshot");
            let target =
                snapshot.tanks.iter().find(|tank| tank.tank_id != app.player_tank).expect("target");
            let mut event = game_core::DamageEvent {
                source: app.player_tank,
                target: target.tank_id,
                hit_position: glam::Vec3::from_array(target.position),
                cause: game_core::DamageCause::Shell,
                ..Default::default()
            };
            event.track_hit = Some(game_core::TrackHit { side: game_core::TrackSide::Left, broke });
            (snapshot, event)
        };

        // Damage without a break: no steel on the ground.
        let (mut snapshot, event) = break_event(&app, false);
        snapshot.server_tick += 1;
        snapshot.damage_events.push(event);
        app.accept_and_sync(snapshot);
        assert!(app.track_ribbons.is_empty(), "a bitten track sheds nothing");

        // The break throws the ribbon.
        let (mut snapshot, event) = break_event(&app, true);
        snapshot.server_tick += 1;
        snapshot.damage_events.push(event);
        app.accept_and_sync(snapshot);
        assert_eq!(app.track_ribbons.len(), 1, "the thrown track lies on the field");

        // The same side reported broken again does not lay a second ribbon.
        let (mut snapshot, event) = break_event(&app, true);
        snapshot.server_tick += 1;
        snapshot.damage_events.push(event);
        app.accept_and_sync(snapshot);
        assert_eq!(app.track_ribbons.len(), 1, "one side sheds once");
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
                owner: Some(game_core::TankId(999)),
                position: glam::Vec3::from_array(player) + glam::Vec3::from_array(offset),
                surface: game_core::ImpactSurface::Terrain,
                shell_type,
                ..Default::default()
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
