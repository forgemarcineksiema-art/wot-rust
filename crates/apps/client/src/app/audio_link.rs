//! The audio side of the frame: game state in, [`audio::AudioEvent`]s and control values out.
//! Events accumulate in `ClientApp::pending_audio` wherever gameplay produces them (snapshot
//! ingest, garage clicks, the ready flash) and are flushed to the platform stream once per
//! rendered frame — the same cadence and philosophy as the visual FX system.

use super::ClientApp;

impl ClientApp {
    /// Queue a sound; it reaches the device on the next presented frame. Cheap and safe to call
    /// from anywhere, headless included (tests read the queue instead of a device).
    pub(super) fn queue_audio(&mut self, event: audio::AudioEvent) {
        self.pending_audio.push(event);
    }

    /// Per-frame flush: listener pose, powerplant state, scene wind, then every queued event.
    /// Without a device (headless, CI, no output hardware) the queue is dropped — gameplay code
    /// never behaves differently for lacking ears.
    pub(super) fn flush_audio(&mut self, eye: Option<[f32; 3]>, target: Option<[f32; 3]>) {
        let events = std::mem::take(&mut self.pending_audio);
        let Some(output) = &self.audio else {
            return;
        };
        let listener = match (eye, target) {
            (Some(eye), Some(target)) => {
                let position = glam::Vec3::from_array(eye);
                let forward = (glam::Vec3::from_array(target) - position).normalize_or_zero();
                Some(audio::Listener { position, forward })
            }
            _ => None,
        };
        let in_garage = self.garage.is_open();
        // The hangar is sheltered: a breath of air, engine off. The field gets the full wind.
        let wind_level = if in_garage { 0.2 } else { 1.0 };
        let (rpm_norm, load, speed_mps, running) = self.player_engine_audio_state(in_garage);
        output.with_engine(move |engine| {
            if let Some(listener) = listener {
                engine.set_listener(listener);
            }
            engine.set_wind_level(wind_level);
            engine.set_player_engine(rpm_norm, load, speed_mps, running);
            for event in events {
                engine.push_event(event);
            }
        });
    }

    /// The powerplant knobs for the engine bed: RPM follows the faster of actual ground speed
    /// (fraction of the vehicle's top speed) and throttle demand — a tank crawling uphill at
    /// full power screams, one coasting downhill hums.
    fn player_engine_audio_state(&self, in_garage: bool) -> (f32, f32, f32, bool) {
        if in_garage {
            return (0.0, 0.0, 0.0, false);
        }
        let alive = self.player_hud_hit_points() > 0;
        let speed_mps = self.predictor.speed_mps().abs();
        let top_speed = self.player_spec().max_forward_speed_mps.max(1.0);
        let load = self.input.throttle().abs().clamp(0.0, 1.0);
        let rpm_norm = (speed_mps / top_speed).clamp(0.0, 1.0).max(load * 0.85);
        (rpm_norm, load, speed_mps, alive)
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::super::ClientApp;

    fn deployed_app() -> ClientApp {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        app
    }

    #[test]
    fn a_replicated_shot_queues_a_cannon_event_with_the_guns_caliber() {
        let mut app = deployed_app();
        app.pending_audio.clear();
        // Fire the player's own gun through the real loop: latch fire, run ticks until the
        // snapshot diff detects it.
        app.input.fire_pending = true;
        app.run_fixed_ticks(12);

        let own_caliber = app.player_spec().gun.shell.caliber_mm;
        let cannon = app.pending_audio.iter().find_map(|event| match event {
            audio::AudioEvent::CannonFired { caliber_mm, own_shot, .. } => {
                Some((*caliber_mm, *own_shot))
            }
            _ => None,
        });
        let (caliber_mm, own_shot) = cannon.expect("the player's shot must queue a cannon sound");
        assert_eq!(caliber_mm, own_caliber);
        assert!(own_shot, "the player's own gun must be flagged for instant playback");
    }

    #[test]
    fn battle_sounds_accumulate_until_a_frame_flushes_them() {
        let mut app = deployed_app();
        app.pending_audio.clear();
        app.queue_audio(audio::AudioEvent::GunReady);
        app.queue_audio(audio::AudioEvent::KillConfirmed);
        assert_eq!(app.pending_audio.len(), 2);

        // Headless flush (no device): the queue must drain, not grow across frames forever.
        app.flush_audio(Some([0.0, 2.0, -5.0]), Some([0.0, 2.0, 0.0]));
        assert!(app.pending_audio.is_empty(), "a frame flush must drain the queue");
    }

    #[test]
    fn the_garage_is_engine_off_and_the_field_is_engine_on() {
        let mut app = deployed_app();
        let (_, _, _, running) = app.player_engine_audio_state(false);
        assert!(running, "a live deployed tank idles audibly");

        app.open_garage();
        let (rpm, load, speed, running) = app.player_engine_audio_state(true);
        assert_eq!((rpm, load, speed, running), (0.0, 0.0, 0.0, false), "hangar tanks sit cold");
    }

    #[test]
    fn full_throttle_raises_rpm_even_before_the_hull_moves() {
        let mut app = deployed_app();
        app.input.forward = true;
        let (rpm_norm, load, _, _) = app.player_engine_audio_state(false);
        assert!(load > 0.9, "throttle demand is the load");
        assert!(rpm_norm > 0.7, "the governor answers the pedal, not just the speedometer");
    }

    #[test]
    fn garage_commits_click_and_browsing_clicks_differently() {
        let mut app = ClientApp::new();
        app.pending_audio.clear();
        app.select_garage_vehicle(game_core::VehicleKind::TigerI);
        assert!(
            app.pending_audio
                .iter()
                .any(|e| matches!(e, audio::AudioEvent::UiClick { accent: false })),
            "browsing must click"
        );
        app.pending_audio.clear();
        app.confirm_garage_selection();
        assert!(
            app.pending_audio
                .iter()
                .any(|e| matches!(e, audio::AudioEvent::UiClick { accent: true })),
            "the Battle commit must land the accented click"
        );
    }

    #[test]
    fn incoming_armor_strikes_queue_spatial_impact_sounds() {
        let mut app = deployed_app();
        app.pending_audio.clear();
        let mut snapshot = app.local_server.latest_snapshot_for_player();
        snapshot.damage_events.push(game_core::DamageEvent {
            source: game_core::TankId(1),
            target: app.player_tank,
            hit_position: Vec3::new(10.0, 1.5, 20.0),
            damage_hp: 50,
            penetrated: true,
            ..game_core::DamageEvent::default()
        });
        snapshot.shell_impacts.push(game_core::ShellImpact {
            owner: game_core::TankId(1),
            position: Vec3::new(30.0, 0.0, 40.0),
            surface: game_core::ImpactSurface::Terrain,
        });
        app.accept_and_sync(snapshot);

        assert!(
            app.pending_audio
                .iter()
                .any(|e| matches!(e, audio::AudioEvent::ArmorStruck { penetrated: true, .. })),
            "the armor strike must ring"
        );
        assert!(
            app.pending_audio.iter().any(|e| matches!(
                e,
                audio::AudioEvent::ShellAbsorbed { surface: audio::GroundKind::Soil, .. }
            )),
            "the terrain impact must thud as soil"
        );
    }
}
