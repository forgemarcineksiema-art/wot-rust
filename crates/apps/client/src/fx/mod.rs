//! The client-side battle-FX system: one CPU particle pool feeding the renderer's unlit FX pass.
//! Muzzle flash, gun smoke, kicked-up dust, impact bursts, and shell tracers all spawn here and
//! come out as one sorted vertex batch per frame — a single system, budgeted and testable,
//! instead of a per-effect renderer.

mod billboard;
#[cfg(test)]
mod budget;
mod collapse;
#[cfg(test)]
mod contact_lock;
mod decals;
mod emitters;
mod fire;
mod impacts;
mod lights;
mod particle;
mod terrain_scars;
mod tracer;
mod track_marks;

use glam::Vec3;
use renderer_api::FxVertex;

pub use decals::{append_decal_quads, decal_from_damage_event};
pub(crate) use fire::{FireEvent, resolve_shots};
pub(crate) use particle::{MAX_PARTICLES, Particle};
pub use terrain_scars::TerrainScars;
pub(crate) use tracer::{ShellTrails, append_shell_tracers};
pub(crate) use track_marks::{TRACK_MARK_SPACING_M, TrackMarks};

/// One frame's tracer batch for a bare shell list — the offscreen/screenshot path, which renders
/// straight from a server snapshot without a `ClientApp`. The in-game path goes through
/// [`FxSystem`] + [`append_shell_tracers`] instead.
pub fn shell_tracer_vertices(shells: &[net::ShellSnapshot], eye: [f32; 3]) -> Vec<FxVertex> {
    let mut vertices = Vec::new();
    tracer::append_shell_tracers(&mut vertices, shells, Vec3::from_array(eye));
    vertices
}

/// The collapse theatre `seconds` after a cover box came down, as one FX vertex batch — the
/// offscreen probe path (`tenement_probe` renders the mid-collapse frame with it). The in-game
/// path is `FxSystem::cover_collapse` on the live system.
pub fn collapse_theatre_vertices(
    center: [f32; 3],
    half_extents_m: [f32; 3],
    seconds: f32,
    eye: [f32; 3],
    target: [f32; 3],
) -> Vec<FxVertex> {
    let mut fx = FxSystem::default();
    fx.cover_collapse(Vec3::from_array(center), Vec3::from_array(half_extents_m));
    let steps = (seconds / 0.05).round().max(1.0) as usize;
    for _ in 0..steps {
        fx.tick(seconds / steps as f32);
    }
    fx.vertices(Vec3::from_array(eye), Vec3::from_array(target))
}

#[derive(Debug, Default)]
pub(crate) struct FxSystem {
    particles: Vec<Particle>,
    /// The paths every seen shell has flown, kept under a second after it passed (A8).
    shell_trails: ShellTrails,
    rng_state: u64,
    /// Fractional spawn budget of the hangar's dust-mote trickle (E1), so the rate stays
    /// framerate-independent.
    mote_budget: f32,
    /// Fractional spawn budget of the heating duct's steam wisps (E2).
    steam_budget: f32,
    /// Fractional spawn budget of the welding arc's spark fountain (K1).
    spark_budget: f32,
    /// The FX clock, seconds of presented time, that staged sequences are scheduled on (Z1).
    stage_clock_s: f32,
    /// Beats of a collapse waiting on the clock (`collapse`).
    staged: Vec<collapse::StagedEmission>,
    /// The shot and the hit as light (S1): transient pulses feeding the profile's local slots.
    lights: Vec<lights::LightPulse>,
}

impl FxSystem {
    /// Add one particle, recycling the particle nearest death once the pool is full: a barrage
    /// degrades by dropping the oldest smoke, never by unbounded growth.
    pub fn spawn(&mut self, particle: Particle) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(particle);
            return;
        }
        if let Some(oldest) = self.particles.iter_mut().max_by(|a, b| a.life().total_cmp(&b.life()))
        {
            *oldest = particle;
        }
    }

    /// Advance every particle one presented frame and drop the dead.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.particles.retain_mut(|particle| particle.tick(dt));
        self.fire_staged(dt);
        self.tick_lights();
    }

    #[cfg(test)]
    pub fn live_particles(&self) -> usize {
        self.particles.len()
    }

    /// Build this frame's FX vertex batch: billboarded (or velocity-stretched) soft quads,
    /// sorted back-to-front so premultiplied smoke composites correctly over itself.
    pub fn vertices(&self, eye: Vec3, target: Vec3) -> Vec<FxVertex> {
        let (right, up) = billboard::view_plane_basis(eye, target);
        // Depth keys are computed ONCE per particle, not per comparison — a full 2048-particle
        // pool pays ~2k distance evaluations instead of ~45k inside the comparator.
        let mut order: Vec<(f32, usize)> = self
            .particles
            .iter()
            .enumerate()
            .map(|(index, particle)| (particle.position.distance_squared(eye), index))
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));

        let mut vertices = Vec::with_capacity(self.particles.len() * 6);
        for (_, index) in order {
            let particle = &self.particles[index];
            let color = particle.color();
            let size = particle.size_m();
            let streak_half = particle.velocity_mps * particle.stretch_s * 0.5;
            if streak_half.length_squared() > (size * 0.5) * (size * 0.5) {
                billboard::push_stretched(
                    &mut vertices,
                    particle.position,
                    streak_half,
                    size * 0.5,
                    color,
                    eye,
                );
            } else {
                billboard::push_billboard(&mut vertices, particle.position, size, color, right, up);
            }
        }
        vertices
    }

    /// A deterministic uniform sample in `[0, 1)` — splitmix64, the same generator the sim's
    /// dispersion uses, so FX spread is reproducible under test without an RNG dependency.
    pub fn rand_unit(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.rng_state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^= value >> 31;
        ((value >> 40) as f32) / ((1u64 << 24) as f32)
    }

    /// A deterministic uniform sample in `[-1, 1)`.
    pub fn rand_signed(&mut self) -> f32 {
        self.rand_unit() * 2.0 - 1.0
    }
}

impl FxSystem {
    /// Note where every live shell is this frame and age the remembered paths (Inny Poziom A8).
    pub fn record_shells(&mut self, shells: &[net::ShellSnapshot], dt_s: f32) {
        self.shell_trails.record(shells, dt_s);
    }

    /// The remembered shell paths as dimming lines, appended to this frame's FX batch.
    pub fn append_shell_trails(&self, vertices: &mut Vec<FxVertex>, eye: Vec3) {
        self.shell_trails.append(vertices, eye);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn puff(position: Vec3, ttl_s: f32) -> Particle {
        Particle {
            position,
            velocity_mps: Vec3::ZERO,
            gravity_factor: 0.0,
            drag_per_s: 0.0,
            age_s: 0.0,
            ttl_s,
            size_begin_m: 1.0,
            size_end_m: 1.0,
            color_begin: [1.0; 4],
            color_end: [0.0; 4],
            stretch_s: 0.0,
        }
    }

    /// Światło służy czołgowi: the shaft glow stays QUIET. The beams were re-priced after
    /// the user's lattice verdict (0.20 -> 0.13) — a beam brighter than this is a second
    /// light show over the floor, and the E1 pricing already proved louder blades eat the
    /// hero-over-room margin.
    #[test]
    fn the_shaft_glow_stays_quiet() {
        let vertices = crate::look_harness::hangar_shaft_fx_vertices();
        assert!(!vertices.is_empty(), "the day hall hangs its blades");
        for vertex in &vertices {
            assert!(
                vertex.color[0] <= 0.14 && vertex.color[1] <= 0.14 && vertex.color[2] <= 0.14,
                "a blade shouts: {:?}",
                vertex.color
            );
        }
    }

    /// K1: sparks fly only while the arc burns, and the fountain is rate-budgeted like
    /// every hall emitter — a quiet arc adds not one particle.
    #[test]
    fn sparks_fly_only_while_the_arc_burns() {
        let corner = Vec3::from_array(scene_build::hangar::WELDING_CORNER);
        let mut fx = FxSystem::default();
        for _ in 0..60 {
            fx.welding_sparks(corner, false, 1.0 / 60.0);
        }
        assert_eq!(fx.live_particles(), 0, "a quiet arc must not spark");
        for _ in 0..60 {
            fx.welding_sparks(corner, true, 1.0 / 60.0);
        }
        let burst = fx.live_particles();
        assert!(
            (25..=60).contains(&burst),
            "a second of arc is a real fountain, budgeted: {burst}"
        );
    }

    /// E1: the mote trickle is rate-budgeted and spawns INSIDE the blade volumes — dust is
    /// visible only where the light is, and the emitter cannot silently flood the pool.
    #[test]
    fn motes_trickle_inside_the_blades() {
        let blades = scene_build::hangar::sun_shaft_quads();
        let mut fx = FxSystem::default();
        fx.hangar_motes(&blades, 2.0);
        let spawned = fx.live_particles();
        assert!((3..=8).contains(&spawned), "2 s of trickle is a handful of motes, got {spawned}");
        for particle in &fx.particles {
            let p = particle.position;
            assert!(p.y > 0.0 && p.y < 11.5, "a mote floats in the hall's air: {p:?}");
            assert!(p.x.abs() < 11.0 && p.z.abs() < 22.0, "a mote stays indoors: {p:?}");
            assert!(
                particle.velocity_mps.length() < 0.1,
                "a mote drifts, it does not fly: {:?}",
                particle.velocity_mps
            );
        }
    }

    #[test]
    fn pool_is_budgeted_and_recycles_the_particle_nearest_death() {
        let mut fx = FxSystem::default();
        for _ in 0..MAX_PARTICLES {
            fx.spawn(puff(Vec3::ZERO, 10.0));
        }
        // Age one particle close to death by making its ttl tiny.
        fx.particles[7].ttl_s = 0.1;
        fx.particles[7].age_s = 0.09;

        fx.spawn(puff(Vec3::splat(99.0), 10.0));

        assert_eq!(fx.live_particles(), MAX_PARTICLES, "pool never exceeds the budget");
        assert_eq!(fx.particles[7].position, Vec3::splat(99.0), "the dying particle is recycled");
    }

    #[test]
    fn tick_removes_expired_particles() {
        let mut fx = FxSystem::default();
        fx.spawn(puff(Vec3::ZERO, 0.05));
        fx.spawn(puff(Vec3::ZERO, 10.0));
        fx.tick(0.1);
        assert_eq!(fx.live_particles(), 1);
    }

    #[test]
    fn vertices_are_emitted_back_to_front_for_correct_transparency() {
        let mut fx = FxSystem::default();
        let eye = Vec3::ZERO;
        fx.spawn(puff(Vec3::new(0.0, 0.0, 5.0), 1.0)); // near
        fx.spawn(puff(Vec3::new(0.0, 0.0, 50.0), 1.0)); // far

        let vertices = fx.vertices(eye, Vec3::Z);

        assert_eq!(vertices.len(), 12);
        let first_quad_z = vertices[0].position[2];
        let second_quad_z = vertices[6].position[2];
        assert!(
            first_quad_z > second_quad_z,
            "farther quad must draw first: {first_quad_z} vs {second_quad_z}"
        );
    }

    #[test]
    fn fast_particles_with_stretch_emit_streaks_not_billboards() {
        let mut fx = FxSystem::default();
        let mut spark = puff(Vec3::new(0.0, 1.0, 10.0), 1.0);
        spark.velocity_mps = Vec3::new(0.0, 0.0, 40.0);
        spark.stretch_s = 0.05; // 2 m streak at 40 m/s vs a 1 m round size
        fx.spawn(spark);

        let vertices = fx.vertices(Vec3::new(5.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 10.0));

        let max_z = vertices.iter().map(|v| v.position[2]).fold(f32::MIN, f32::max);
        let min_z = vertices.iter().map(|v| v.position[2]).fold(f32::MAX, f32::min);
        assert!((max_z - min_z - 2.0).abs() < 1.0e-3, "streak spans stretch_s x speed");
    }

    #[test]
    fn deterministic_rng_is_stable_across_runs() {
        let mut a = FxSystem::default();
        let mut b = FxSystem::default();
        for _ in 0..16 {
            let sample = a.rand_unit();
            assert_eq!(sample, b.rand_unit());
            assert!((0.0..1.0).contains(&sample));
        }
    }
}
