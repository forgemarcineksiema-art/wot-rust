//! One battle-FX particle: a soft world-space quad with simple ballistic motion (fractional
//! gravity + exponential drag) and size/color ramps over its lifetime. Premultiplied colors,
//! so a particle can be authored anywhere between additive glow (alpha 0) and opaque smoke.

use game_core::math::GRAVITY_MPS2;
use glam::Vec3;

/// Hard cap on live particles. At the cap new spawns recycle the particle closest to death, so
/// a barrage degrades by dropping the oldest smoke — never by unbounded growth.
pub(crate) const MAX_PARTICLES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Particle {
    pub position: Vec3,
    pub velocity_mps: Vec3,
    /// Fraction of gravity applied: 0 floats (smoke), 1 falls ballistically (dirt clods).
    pub gravity_factor: f32,
    /// Exponential drag rate (1/s): velocity decays by `exp(-drag * dt)` each tick.
    pub drag_per_s: f32,
    pub age_s: f32,
    pub ttl_s: f32,
    /// Quad size (full width, meters) at birth and death; lerped over life.
    pub size_begin_m: f32,
    pub size_end_m: f32,
    /// Premultiplied RGBA at birth and death; lerped over life.
    pub color_begin: [f32; 4],
    pub color_end: [f32; 4],
    /// Velocity stretch (seconds of motion the quad elongates over): 0 stays round, larger
    /// values streak the quad along its velocity — sparks and embers.
    pub stretch_s: f32,
}

impl Particle {
    /// Advance one frame; returns `false` once the particle's life is over.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.velocity_mps *= (-self.drag_per_s * dt).exp();
        self.velocity_mps.y -= GRAVITY_MPS2 * self.gravity_factor * dt;
        self.position += self.velocity_mps * dt;
        self.age_s += dt;
        self.age_s < self.ttl_s
    }

    /// Life progress in `[0, 1]`.
    pub fn life(&self) -> f32 {
        (self.age_s / self.ttl_s.max(1.0e-6)).clamp(0.0, 1.0)
    }

    pub fn size_m(&self) -> f32 {
        let t = self.life();
        self.size_begin_m + (self.size_end_m - self.size_begin_m) * t
    }

    pub fn color(&self) -> [f32; 4] {
        let t = self.life();
        let mut color = [0.0; 4];
        for (channel, slot) in color.iter_mut().enumerate() {
            *slot = self.color_begin[channel]
                + (self.color_end[channel] - self.color_begin[channel]) * t;
        }
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn still_smoke() -> Particle {
        Particle {
            position: Vec3::ZERO,
            velocity_mps: Vec3::ZERO,
            gravity_factor: 0.0,
            drag_per_s: 0.0,
            age_s: 0.0,
            ttl_s: 1.0,
            size_begin_m: 1.0,
            size_end_m: 3.0,
            color_begin: [1.0, 1.0, 1.0, 1.0],
            color_end: [0.0, 0.0, 0.0, 0.0],
            stretch_s: 0.0,
        }
    }

    #[test]
    fn particle_dies_exactly_when_its_ttl_elapses() {
        let mut particle = still_smoke();
        assert!(particle.tick(0.5), "alive at half life");
        assert!(!particle.tick(0.6), "dead past ttl");
    }

    #[test]
    fn size_and_color_ramp_linearly_over_life() {
        let mut particle = still_smoke();
        particle.tick(0.5);
        assert!((particle.size_m() - 2.0).abs() < 1.0e-4);
        let color = particle.color();
        for channel in color {
            assert!((channel - 0.5).abs() < 1.0e-4, "premultiplied lerp at half life: {color:?}");
        }
    }

    #[test]
    fn gravity_factor_scales_the_fall_and_drag_bleeds_speed() {
        let mut clod = still_smoke();
        clod.gravity_factor = 1.0;
        clod.velocity_mps = Vec3::new(10.0, 0.0, 0.0);
        clod.drag_per_s = 2.0;
        clod.tick(0.1);
        assert!(clod.velocity_mps.y < -0.5, "full gravity pulls down");
        assert!(clod.velocity_mps.x < 10.0 * 0.85, "drag bleeds horizontal speed");

        let mut smoke = still_smoke();
        smoke.tick(0.1);
        assert_eq!(smoke.velocity_mps.y, 0.0, "zero gravity factor floats");
    }
}
