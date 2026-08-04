//! Living ground (Inna Liga D1): motion writes into the air. Rolling tracks pull dust off the
//! soil — per SIDE, from the accumulated track distance the presentation already carries, so a
//! pivot turn dusts the working track and a parked tank is dead still — and the exhaust
//! breathes with the drive. Presentation only; every particle rides the existing capped FX
//! pool, so a 14-tank column costs what the pool already budgeted.

use game_core::HitboxProfile;
use glam::Vec3;

use super::ClientApp;

/// Below this track speed (m/s) the soil stays down.
const DUST_MIN_TRACK_SPEED: f32 = 1.1;
/// Seconds between dust puffs per side at reference speed; faster tracks puff faster.
const DUST_PERIOD_S: f32 = 0.16;
/// Reference speed for the dust cadence scale.
const DUST_REFERENCE_SPEED: f32 = 6.0;
/// Idle engines still breathe, but the plume only reads once the drive works.
const EXHAUST_MIN_SPEED: f32 = 0.6;
const EXHAUST_PERIOD_S: f32 = 0.34;
/// Water deep enough to read as fording (D7): below this the field is merely damp.
const WADING_DEPTH_M: f32 = 0.15;
/// Bow-splash cadence at the reference speed; the spray keeps pace with the hull.
const SPLASH_PERIOD_S: f32 = 0.12;
const SPLASH_MIN_SPEED: f32 = 1.2;

#[derive(Default)]
pub(super) struct MotionFxState {
    last_track_m: [f32; 2],
    primed: bool,
    dust_accum_s: [f32; 2],
    exhaust_accum_s: f32,
    /// D5 rut writer per side: where the last stamped segment ended, and the track travel
    /// accumulated since — the pen touches down at the first contact and lifts on a break.
    rut_from: [Option<Vec3>; 2],
    rut_travel_m: [f32; 2],
    splash_accum_s: f32,
}

impl ClientApp {
    /// Per presented frame, before the render objects are built: read each live tank's track
    /// motion and let the world answer it.
    pub(super) fn tick_motion_fx(&mut self, tanks: &[engine::PresentationTank], dt: f32) {
        if dt <= 0.0 {
            return;
        }
        // Rain-softened ground takes a darker rut (D5); one judgment per frame for the field.
        let wetness = self.weather_frame.surface_wetness;
        let dust_dryness = 1.0 - wetness;
        self.motion_fx.retain(|id, _| tanks.iter().any(|tank| tank.id == *id));
        for tank in tanks {
            if tank.hit_points == 0 {
                continue;
            }
            let state = self.motion_fx.entry(tank.id).or_default();
            let tracks = [tank.track_left_m, tank.track_right_m];
            if !state.primed {
                // First sight of this tank: accumulated distances are history, not motion.
                state.last_track_m = tracks;
                state.primed = true;
                continue;
            }
            let hitbox = HitboxProfile::for_vehicle(tank.vehicle);
            let (sin, cos) = tank.hull_yaw_rad.sin_cos();
            let rotate = |local: Vec3| {
                Vec3::new(cos * local.x + sin * local.z, local.y, -sin * local.x + cos * local.z)
            };
            let base = Vec3::from_array(tank.translation);
            let heading = rotate(Vec3::Z);
            // The river is present (D7): a hull in deep-enough water swaps the ground grammar —
            // foam instead of dust, wake instead of ruts, spray off the bow. One judgment per
            // tank from the same `depth_over` rule that drives wading and drowning.
            let ground_y =
                self.battlefield.heightmap.sample_height(base.x, base.z).unwrap_or(base.y);
            let water_surface = self
                .battlefield
                .water
                .filter(|water| water.depth_over(ground_y) > WADING_DEPTH_M)
                .map(|water| water.surface_level_m);

            let mut fastest = 0.0_f32;
            for (side, x_sign) in [(0_usize, -1.0_f32), (1, 1.0)] {
                let signed_delta = tracks[side] - state.last_track_m[side];
                let speed = signed_delta.max(0.0) / dt;
                state.last_track_m[side] = tracks[side];
                fastest = fastest.max(speed);

                // D5: the track writes its rut — a segment every ~1.5 m of travel (reverse
                // counts; the soil does not care which way the links roll). A broken track
                // lifts the pen: the drivetrain's truth reaches the ground.
                let contact =
                    base + rotate(Vec3::new(x_sign * hitbox.half_width_m * 0.86, 0.0, 0.0));
                if tank.track_break_t[side].is_some() {
                    state.rut_from[side] = None;
                    state.rut_travel_m[side] = 0.0;
                } else {
                    state.rut_travel_m[side] += signed_delta.abs();
                    match state.rut_from[side] {
                        None => {
                            state.rut_from[side] = Some(contact);
                            state.rut_travel_m[side] = 0.0;
                        }
                        Some(from)
                            if state.rut_travel_m[side] >= crate::fx::TRACK_MARK_SPACING_M =>
                        {
                            // Fording writes its wake on the surface; dry ground takes a rut
                            // as deep as the GROUND under it remembers (one classifier read
                            // at the segment midpoint — rock takes none).
                            match water_surface {
                                Some(surface) => {
                                    self.track_marks.record_foam_segment(from, contact, surface);
                                }
                                None => {
                                    let mid = (from + contact) * 0.5;
                                    let rut_depth_m = self
                                        .ground
                                        .properties_at(&self.battlefield.heightmap, mid.x, mid.z)
                                        .rut_depth_m;
                                    self.track_marks.record_segment(
                                        from,
                                        contact,
                                        wetness,
                                        rut_depth_m,
                                        &self.battlefield.heightmap,
                                    );
                                }
                            }
                            state.rut_from[side] = Some(contact);
                            state.rut_travel_m[side] = 0.0;
                        }
                        Some(_) => {}
                    }
                }
                // Dust and splash are mutually exclusive: wet soil does not rise.
                if water_surface.is_some() || speed < DUST_MIN_TRACK_SPEED || dust_dryness < 0.15 {
                    state.dust_accum_s[side] = 0.0;
                    continue;
                }
                state.dust_accum_s[side] +=
                    dt * (speed / DUST_REFERENCE_SPEED).clamp(0.5, 2.2) * dust_dryness;
                while state.dust_accum_s[side] >= DUST_PERIOD_S {
                    state.dust_accum_s[side] -= DUST_PERIOD_S;
                    let ground = base
                        + rotate(Vec3::new(
                            x_sign * hitbox.half_width_m * 0.86,
                            0.0,
                            -hitbox.half_length_m * 0.72,
                        ));
                    self.fx.rolling_dust(
                        ground,
                        heading,
                        (speed / 10.0).clamp(0.3, 1.0) * dust_dryness,
                    );
                }
            }

            // The bow shoulders the river aside (D7): spray off the leading edge, paced with
            // the hull. The wake foam above tells the story behind; this is the story ahead.
            if let Some(surface) = water_surface {
                if fastest > SPLASH_MIN_SPEED {
                    state.splash_accum_s += dt * (fastest / DUST_REFERENCE_SPEED).clamp(0.5, 2.2);
                    while state.splash_accum_s >= SPLASH_PERIOD_S {
                        state.splash_accum_s -= SPLASH_PERIOD_S;
                        let bow = base + rotate(Vec3::new(0.0, 0.0, hitbox.half_length_m * 0.9));
                        let at_waterline = Vec3::new(bow.x, surface, bow.z);
                        self.fx.bow_splash(at_waterline, heading, (fastest / 8.0).clamp(0.3, 1.0));
                    }
                } else {
                    state.splash_accum_s = 0.0;
                }
            } else {
                state.splash_accum_s = 0.0;
            }

            // The exhaust breathes with the drive: working tracks or a hard shove on the
            // throttle read as denser, faster puffs off the left-rear deck (the T-54 family's
            // exhaust side; close enough for the whole fleet at battle range).
            let working = fastest > EXHAUST_MIN_SPEED || tank.accel_long_mps2 > 1.5;
            if working {
                state.exhaust_accum_s += dt * (1.0 + fastest / 12.0);
                while state.exhaust_accum_s >= EXHAUST_PERIOD_S {
                    state.exhaust_accum_s -= EXHAUST_PERIOD_S;
                    let port = base
                        + rotate(Vec3::new(
                            -hitbox.half_width_m * 0.7,
                            hitbox.turret_min_y_m + hitbox.center_y_m - 0.15,
                            -hitbox.half_length_m * 0.55,
                        ));
                    self.fx.exhaust_puff(port, (fastest / 10.0).clamp(0.25, 1.0));
                }
            } else {
                state.exhaust_accum_s = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use engine::PresentationTank;
    use game_core::{TankId, TeamId, VehicleKind};

    use super::super::ClientApp;

    /// Pin the app's world to an analytic heightfield and rebuild the ground rule to match.
    /// The rut writer reads real ground now (teren A3) — a test that drives on "wherever
    /// the default map happens to be at (10, 20)" would assert on geography, not on code.
    fn pin_ground(app: &mut ClientApp, height_at: impl Fn(f32, f32) -> f32) {
        let battlefield = std::sync::Arc::make_mut(&mut app.battlefield);
        battlefield.heightmap = terrain::heightmap_from_fn(65, 5.0, height_at);
        battlefield.water = None;
        battlefield.roads.clear();
        app.ground = terrain::GroundClassifier::new(&app.battlefield);
    }

    fn tank(id: u64, track_m: f32, hit_points: u32) -> PresentationTank {
        PresentationTank {
            id: TankId(id),
            team: TeamId(1),
            vehicle: VehicleKind::T54_1951,
            translation: [10.0, 0.0, 20.0],
            hull_yaw_rad: 0.4,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points,
            destroyed_modules_mask: 0,
            spotted_by_teams_mask: 0b11,
            module_hit_points: [100; game_core::MODULE_SLOT_COUNT],
            track_damage_mask: 0,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: track_m,
            track_right_m: track_m,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        }
    }

    /// D1's contract: motion writes into the air — a rolling tank dusts and breathes, a parked
    /// one is dead still, a wreck never stirs, and the first sighting primes instead of
    /// exploding (accumulated track history is not motion).
    #[test]
    fn rolling_tanks_dust_and_breathe_parked_and_dead_ones_do_not() {
        let mut app = ClientApp::new();
        app.fx = crate::fx::FxSystem::default();

        // Priming frame: history, not motion — no particles.
        app.tick_motion_fx(&[tank(1, 500.0, 900)], 0.1);
        assert_eq!(app.fx.live_particles(), 0, "first sight primes, never bursts");

        // A second of rolling at ~6 m/s: dust from both tracks plus exhaust breath.
        for step in 1..=10 {
            app.tick_motion_fx(&[tank(1, 500.0 + step as f32 * 0.6, 900)], 0.1);
        }
        let rolling = app.fx.live_particles();
        assert!(rolling >= 8, "a rolling tank stirs the ground and breathes, got {rolling}");

        // Parked: the world settles (no NEW particles).
        app.fx = crate::fx::FxSystem::default();
        for _ in 0..10 {
            app.tick_motion_fx(&[tank(1, 506.0, 900)], 0.1);
        }
        assert_eq!(app.fx.live_particles(), 0, "a parked tank is dead still");

        // A wreck rolling downhill (track deltas) still stirs nothing.
        app.fx = crate::fx::FxSystem::default();
        for step in 0..10 {
            app.tick_motion_fx(&[tank(2, step as f32 * 0.6, 0)], 0.1);
        }
        assert_eq!(app.fx.live_particles(), 0, "a wreck never stirs the ground");
    }

    /// D5's contract: rolling tracks press rut segments into the soil every ~1.5 m of travel,
    /// a parked tank writes nothing, and a broken track lifts the pen while the healthy side
    /// keeps writing.
    #[test]
    fn rolling_tracks_write_ruts_and_a_broken_track_stops() {
        let mut app = ClientApp::new();
        pin_ground(&mut app, |_, _| 0.0);

        // Prime, then roll ~9 m — the hull actually travels (a rut is ground covered, not an
        // odometer; a tank spinning its tracks on ice writes nothing).
        let rolling_tank = |travel_m: f32| {
            let mut t = tank(1, 100.0 + travel_m, 900);
            t.translation = [10.0, 0.0, 20.0 + travel_m];
            t
        };
        app.tick_motion_fx(&[rolling_tank(0.0)], 0.1);
        for step in 1..=15 {
            app.tick_motion_fx(&[rolling_tank(step as f32 * 0.6)], 0.1);
        }
        let rolling = app.track_marks.live_marks();
        assert!(
            (8..=14).contains(&rolling),
            "9 m of travel writes ~1 segment per 1.5 m per side, got {rolling}"
        );

        // Parked: no new ruts.
        for _ in 0..10 {
            app.tick_motion_fx(&[rolling_tank(9.0)], 0.1);
        }
        assert_eq!(app.track_marks.live_marks(), rolling, "a parked tank writes no rut");

        // Break the left track: only the right side keeps writing over the same distance.
        let mut app = ClientApp::new();
        pin_ground(&mut app, |_, _| 0.0);
        let broken = |travel_m: f32| {
            let mut t = tank(1, 100.0 + travel_m, 900);
            t.translation = [10.0, 0.0, 20.0 + travel_m];
            t.track_break_t = [Some(0.5), None];
            t
        };
        app.tick_motion_fx(&[broken(0.0)], 0.1);
        for step in 1..=15 {
            app.tick_motion_fx(&[broken(step as f32 * 0.6)], 0.1);
        }
        let one_sided = app.track_marks.live_marks();
        assert!(
            one_sided > 0 && one_sided <= rolling / 2 + 1,
            "a broken track lifts the pen — one side writes, got {one_sided} vs both-sides {rolling}"
        );
    }

    /// D7's contract: a fording hull swaps the ground grammar — spray and wake foam instead of
    /// dust and ruts (mutually exclusive), and the river closes over the wake in seconds while
    /// a dry run's ruts endure.
    #[test]
    fn a_fording_tank_sprays_and_wakes_instead_of_dusting_and_rutting() {
        let rolling_tank = |travel_m: f32| {
            let mut t = tank(1, 100.0 + travel_m, 900);
            t.translation = [10.0, 0.0, 20.0 + travel_m];
            t
        };

        // Dry baseline: dust rises and ruts outlive 8 s of ticking.
        let mut dry = ClientApp::new();
        pin_ground(&mut dry, |_, _| 0.0);
        dry.fx = crate::fx::FxSystem::default();
        dry.tick_motion_fx(&[rolling_tank(0.0)], 0.1);
        for step in 1..=15 {
            dry.tick_motion_fx(&[rolling_tank(step as f32 * 0.6)], 0.1);
        }
        assert!(dry.fx.live_particles() > 0, "dry ground dusts");
        let dry_marks = dry.track_marks.live_marks();
        for _ in 0..80 {
            dry.track_marks.tick(0.1);
        }
        assert_eq!(dry.track_marks.live_marks(), dry_marks, "dry ruts endure 8 s easily");

        // The same run through water over the hull's floor: no dust — spray instead — and the
        // wake is foam the river closes over.
        let mut fording = ClientApp::new();
        pin_ground(&mut fording, |_, _| 0.0);
        // Flood the flat run: the surface sits half a meter over the ground.
        std::sync::Arc::make_mut(&mut fording.battlefield).water =
            Some(terrain::WaterBody { surface_level_m: 0.5 });
        fording.fx = crate::fx::FxSystem::default();
        fording.tick_motion_fx(&[rolling_tank(0.0)], 0.1);
        for step in 1..=15 {
            fording.tick_motion_fx(&[rolling_tank(step as f32 * 0.6)], 0.1);
        }
        assert!(fording.fx.live_particles() > 0, "the bow throws spray");
        let wake = fording.track_marks.live_marks();
        assert!(wake > 0, "the wake writes foam streaks");
        for _ in 0..80 {
            fording.track_marks.tick(0.1);
        }
        assert_eq!(fording.track_marks.live_marks(), 0, "the river closes over the wake");
    }

    /// Teren A3's contract: the rut writer consults the GROUND, not only the weather. The
    /// same 9 m drive prints on turf and prints NOTHING on bare rock — "nothing marks rock"
    /// stops being an unread field and becomes a picture.
    #[test]
    fn tracks_read_the_ground_not_just_the_weather() {
        let rolling_tank = |travel_m: f32| {
            let mut t = tank(1, 100.0 + travel_m, 900);
            t.translation = [10.0, 0.0, 20.0 + travel_m];
            t
        };

        let mut turf = ClientApp::new();
        pin_ground(&mut turf, |_, _| 0.0);
        turf.tick_motion_fx(&[rolling_tank(0.0)], 0.1);
        for step in 1..=15 {
            turf.tick_motion_fx(&[rolling_tank(step as f32 * 0.6)], 0.1);
        }
        assert!(turf.track_marks.live_marks() > 0, "turf takes the mark");

        // A 60-degree stone face: the classifier reads pure rock there (steep saturates),
        // and rock's rut depth is 0.0 — the pen never touches the ground.
        let mut stone = ClientApp::new();
        pin_ground(&mut stone, |x, _| x * 1.7);
        stone.tick_motion_fx(&[rolling_tank(0.0)], 0.1);
        for step in 1..=15 {
            stone.tick_motion_fx(&[rolling_tank(step as f32 * 0.6)], 0.1);
        }
        assert_eq!(stone.track_marks.live_marks(), 0, "stone remembers nothing");
    }
}
