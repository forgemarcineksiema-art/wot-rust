//! The collapse theatre (Inny Poziom Z1). A cover object stepping up in destruction used to
//! fire one masonry burst and one puff of track dust at its CENTRE — five sparks and six puffs
//! for an 18 m tenement — and swap the baked mesh. The collapse is a staged sequence now, sized
//! by the box that fell:
//!
//! 1. **now** — a dust curtain along the whole top face, rolling down and outward, and falling
//!    masonry chunks off the same face (cards with gravity, dark and small);
//! 2. **the settle beat** (`SETTLE_BEAT_S`) — the walls meet the ground: a low, wide wave of
//!    dust along the perimeter, rolling outward;
//! 3. **the haze** (`HAZE_BEAT_S`) — slow, thin puffs hanging in the footprint for seconds.
//!
//! Counts scale with footprint area (the settle wave with the perimeter), each clamped so a
//! fence is not silent and a factory hall does not empty the pool; the audio hit rides the
//! same event (`audio::AudioEvent::CoverCollapse`). The beats are scheduled on the FX clock
//! and fired by `FxSystem::tick`, so the sequence is deterministic under test.

use glam::Vec3;

use super::{FxSystem, Particle};

/// The walls meet the ground this long after the collapse begins.
pub const SETTLE_BEAT_S: f32 = 0.35;
/// The hanging haze sets in this long after.
pub const HAZE_BEAT_S: f32 = 0.9;

/// Curtain puffs per square metre of footprint, and their clamp. The curtain fills the box's
/// VOLUME from the first frame — the baked walls are already rubble when the phase steps, and
/// a curtain that started at the roof line hung eleven metres over an empty footprint at
/// 0.45 s (the first probe frame); the walls have to vanish inside the dust, not above it.
const CURTAIN_PER_M2: f32 = 1.6;
const CURTAIN_RANGE: (usize, usize) = (12, 220);
/// Chunk cards per square metre of footprint (times a height factor), and their clamp.
const CHUNKS_PER_M2: f32 = 0.6;
const CHUNKS_RANGE: (usize, usize) = (6, 90);
/// Settle puffs per metre of perimeter, and their clamp.
const SETTLE_PER_M: f32 = 1.5;
const SETTLE_RANGE: (usize, usize) = (10, 120);
/// Haze puffs per square metre of footprint, and their clamp.
const HAZE_PER_M2: f32 = 0.25;
const HAZE_RANGE: (usize, usize) = (4, 40);

const MASONRY: [f32; 3] = [0.52, 0.50, 0.46];
const MASONRY_DARK: [f32; 3] = [0.30, 0.27, 0.24];
const SETTLE_TONE: [f32; 3] = [0.56, 0.53, 0.48];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum StagedKind {
    Settle,
    Haze,
}

/// A beat of the sequence waiting on the FX clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StagedEmission {
    pub due_s: f32,
    pub kind: StagedKind,
    pub center: Vec3,
    pub half: Vec3,
}

/// A count that grows with the box and stays inside its clamp.
fn scaled(measure: f32, per: f32, range: (usize, usize)) -> usize {
    ((measure * per).round().max(0.0) as usize).clamp(range.0, range.1)
}

impl FxSystem {
    /// Begin the collapse of a cover box: the first beat now, the others scheduled.
    pub fn cover_collapse(&mut self, center: Vec3, half: Vec3) {
        self.dust_curtain(center, half);
        self.falling_chunks(center, half);
        let now = self.stage_clock_s;
        self.staged.push(StagedEmission {
            due_s: now + SETTLE_BEAT_S,
            kind: StagedKind::Settle,
            center,
            half,
        });
        self.staged.push(StagedEmission {
            due_s: now + HAZE_BEAT_S,
            kind: StagedKind::Haze,
            center,
            half,
        });
    }

    /// Advance the FX clock and fire every beat that has come due.
    pub(crate) fn fire_staged(&mut self, dt: f32) {
        self.stage_clock_s += dt;
        let now = self.stage_clock_s;
        let due: Vec<StagedEmission> =
            self.staged.iter().copied().filter(|beat| beat.due_s <= now).collect();
        self.staged.retain(|beat| beat.due_s > now);
        for beat in due {
            match beat.kind {
                StagedKind::Settle => self.settle_wave(beat.center, beat.half),
                StagedKind::Haze => self.hanging_haze(beat.center, beat.half),
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn staged_beats(&self) -> usize {
        self.staged.len()
    }

    /// Beat 1a: the dust mass — the whole box's volume from a third of its height to the roof
    /// line, falling with the walls and rolling outward.
    fn dust_curtain(&mut self, center: Vec3, half: Vec3) {
        let area = 4.0 * half.x * half.z;
        let count = scaled(area, CURTAIN_PER_M2, CURTAIN_RANGE);
        let tall = 1.0 + (half.y / 12.0).min(1.0);
        for _ in 0..count {
            let dx = self.rand_signed() * half.x;
            let dz = self.rand_signed() * half.z;
            let outward =
                Vec3::new(dx / half.x.max(0.1), 0.0, dz / half.z.max(0.1)).normalize_or_zero();
            let speed = 1.5 + self.rand_unit() * 2.5;
            let fall = 2.5 + self.rand_unit() * 3.0;
            let ttl = 1.6 + self.rand_unit() * 1.2;
            let height = half.y * (0.6 + self.rand_unit() * 1.4);
            let alpha = 0.55;
            self.spawn(Particle {
                position: center + Vec3::new(dx, height - half.y, dz),
                velocity_mps: outward * speed - Vec3::Y * fall,
                gravity_factor: 0.5,
                drag_per_s: 1.0,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 2.0 * tall,
                size_end_m: 5.0 * tall,
                color_begin: [MASONRY[0] * alpha, MASONRY[1] * alpha, MASONRY[2] * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
                seat: None,
            });
        }
    }

    /// Beat 1b: masonry chunks off the top face — dark, small, ballistic.
    fn falling_chunks(&mut self, center: Vec3, half: Vec3) {
        let area = 4.0 * half.x * half.z;
        let height_factor = (half.y / 6.0).clamp(0.5, 2.0);
        let count = scaled(area * height_factor, CHUNKS_PER_M2, CHUNKS_RANGE);
        for _ in 0..count {
            let dx = self.rand_signed() * half.x;
            let dz = self.rand_signed() * half.z;
            let outward =
                Vec3::new(dx / half.x.max(0.1), 0.0, dz / half.z.max(0.1)).normalize_or_zero();
            let speed = 1.5 + self.rand_unit() * 2.5;
            // Off the roof line and DOWN: a few tumble up, most just let go.
            let up = -0.5 + self.rand_unit() * 2.0;
            let ttl = 0.9 + self.rand_unit() * 1.0;
            let size = 0.4 + self.rand_unit() * 0.5;
            let alpha = 0.9;
            self.spawn(Particle {
                position: center + Vec3::new(dx, half.y, dz),
                velocity_mps: outward * speed + Vec3::Y * up,
                gravity_factor: 1.0,
                drag_per_s: 0.3,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: size,
                size_end_m: size * 0.6,
                color_begin: [
                    MASONRY_DARK[0] * alpha,
                    MASONRY_DARK[1] * alpha,
                    MASONRY_DARK[2] * alpha,
                    alpha,
                ],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.03,
                seat: None,
            });
        }
    }

    /// Beat 2: the walls meet the ground — a low, wide wave of dust along the perimeter.
    fn settle_wave(&mut self, center: Vec3, half: Vec3) {
        let perimeter = 4.0 * (half.x + half.z);
        let count = scaled(perimeter, SETTLE_PER_M, SETTLE_RANGE);
        let ground_y = center.y - half.y;
        for _ in 0..count {
            // A point on the perimeter band: pick a side, walk along it.
            let along = self.rand_signed();
            let (dx, dz) = if self.rand_unit() < half.x / (half.x + half.z).max(0.1) {
                (along * half.x, if self.rand_unit() < 0.5 { -half.z } else { half.z })
            } else {
                (if self.rand_unit() < 0.5 { -half.x } else { half.x }, along * half.z)
            };
            let outward =
                Vec3::new(dx / half.x.max(0.1), 0.0, dz / half.z.max(0.1)).normalize_or_zero();
            let speed = 2.0 + self.rand_unit() * 2.0;
            let ttl = 2.0 + self.rand_unit() * 1.5;
            let alpha = 0.45;
            self.spawn(Particle {
                position: Vec3::new(center.x + dx, ground_y + 0.3, center.z + dz),
                velocity_mps: outward * speed + Vec3::Y * 0.3,
                gravity_factor: 0.05,
                drag_per_s: 1.2,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 1.5,
                size_end_m: 5.5,
                color_begin: [
                    SETTLE_TONE[0] * alpha,
                    SETTLE_TONE[1] * alpha,
                    SETTLE_TONE[2] * alpha,
                    alpha,
                ],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
                seat: None,
            });
        }
    }

    /// Beat 3: thin haze hanging in the footprint for seconds.
    fn hanging_haze(&mut self, center: Vec3, half: Vec3) {
        let area = 4.0 * half.x * half.z;
        let count = scaled(area, HAZE_PER_M2, HAZE_RANGE);
        let ground_y = center.y - half.y;
        for _ in 0..count {
            let dx = self.rand_signed() * half.x;
            let dz = self.rand_signed() * half.z;
            let lift = half.y * (0.4 + self.rand_unit() * 1.2);
            let drift = Vec3::new(self.rand_signed(), 0.0, self.rand_signed()) * 0.35;
            let ttl = 4.0 + self.rand_unit() * 2.0;
            let alpha = 0.22;
            self.spawn(Particle {
                position: Vec3::new(center.x + dx, ground_y + lift, center.z + dz),
                velocity_mps: drift + Vec3::Y * 0.15,
                gravity_factor: 0.0,
                drag_per_s: 0.6,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 3.0,
                size_end_m: 8.0,
                color_begin: [MASONRY[0] * alpha, MASONRY[1] * alpha, MASONRY[2] * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
                seat: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::MAX_PARTICLES;

    fn shed() -> (Vec3, Vec3) {
        (Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 1.0))
    }

    fn tenement() -> (Vec3, Vec3) {
        (Vec3::new(0.0, 9.0, 0.0), Vec3::new(9.0, 9.0, 6.0))
    }

    fn particle_positions(fx: &FxSystem) -> Vec<Vec3> {
        fx.particles.iter().map(|particle| particle.position).collect()
    }

    /// `FxSystem::tick` clamps a frame to 0.1 s (a hitch never fast-forwards the smoke), so
    /// the clock is advanced in frame-sized steps.
    fn advance(fx: &mut FxSystem, seconds: f32) {
        let steps = (seconds / 0.05).round().max(1.0) as usize;
        for _ in 0..steps {
            fx.tick(seconds / steps as f32);
        }
    }

    /// The whole point of Z1: the theatre is sized by what fell. A tenement throws many times
    /// the particles of a shed, and it throws them across its own footprint — not at its centre.
    #[test]
    fn the_theatre_scales_with_the_footprint_and_covers_it() {
        let mut small = FxSystem::default();
        let (center, half) = shed();
        small.cover_collapse(center, half);
        let mut big = FxSystem::default();
        let (center, half) = tenement();
        big.cover_collapse(center, half);
        assert!(
            big.live_particles() >= small.live_particles() * 4,
            "tenement {} vs shed {} particles",
            big.live_particles(),
            small.live_particles()
        );
        let reach_x =
            particle_positions(&big).iter().map(|p| (p.x - center.x).abs()).fold(0.0_f32, f32::max);
        assert!(
            reach_x >= half.x * 0.7 && reach_x <= half.x + 1.5,
            "the dust spans the footprint: reach {reach_x:.1} m on a {:.1} m half-width",
            half.x
        );
    }

    /// Three beats on the FX clock: the curtain now, the settle wave at 0.35 s, the haze at
    /// 0.9 s — each fires once, each adds particles, and nothing fires early.
    #[test]
    fn the_collapse_has_three_beats() {
        let mut fx = FxSystem::default();
        let (center, half) = tenement();
        fx.cover_collapse(center, half);
        assert_eq!(fx.staged_beats(), 2, "two beats wait after the first");
        let after_curtain = fx.live_particles();

        advance(&mut fx, 0.2);
        assert_eq!(fx.staged_beats(), 2, "nothing fires before the settle beat");
        assert!(fx.live_particles() <= after_curtain, "only deaths until the settle beat");

        advance(&mut fx, 0.2); // 0.4 s: the settle wave has fired
        assert_eq!(fx.staged_beats(), 1);
        let after_settle = fx.live_particles();
        assert!(
            after_settle > after_curtain,
            "the settle wave adds dust: {after_curtain} -> {after_settle}"
        );

        advance(&mut fx, 0.6); // 1.0 s: the haze has fired
        assert_eq!(fx.staged_beats(), 0);
        assert!(fx.live_particles() > after_settle - 60, "the haze adds puffs over the deaths");
    }

    /// The biggest collapse on the maps leaves the pool more than half free for the battle
    /// around it — a barrage still gets its smoke.
    #[test]
    fn a_tenement_collapse_leaves_the_pool_room() {
        let mut fx = FxSystem::default();
        let (center, half) = tenement();
        fx.cover_collapse(center, half);
        let mut peak = fx.live_particles();
        for _ in 0..12 {
            fx.tick(0.1);
            peak = peak.max(fx.live_particles());
        }
        assert!(peak <= MAX_PARTICLES / 2, "peak {peak} of {MAX_PARTICLES}");
    }

    /// Same box, same theatre: the sequence is deterministic on the FX clock.
    #[test]
    fn the_same_box_collapses_the_same_way() {
        let run = || {
            let mut fx = FxSystem::default();
            let (center, half) = tenement();
            fx.cover_collapse(center, half);
            advance(&mut fx, 0.4);
            advance(&mut fx, 0.6);
            particle_positions(&fx)
        };
        assert_eq!(run(), run());
    }
}
