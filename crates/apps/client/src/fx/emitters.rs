//! Effect recipes: parameterized particle bursts for the battle moments (muzzle blast so far;
//! shell impacts and engine smoke join in later milestones). All colors are premultiplied RGBA —
//! `alpha = 0` rows blend purely additively (flash), non-zero alpha rows are real occluding smoke.

use glam::Vec3;

use super::{FxSystem, Particle};

/// A muzzle low enough over the ground to kick up its signature dust ring.
const MUZZLE_DUST_HEIGHT_M: f32 = 2.2;

impl FxSystem {
    /// The full muzzle event of a main-gun shot: a hot additive flash at the muzzle, a fast jet
    /// of smoke blown out along the bore, and — when the muzzle hangs low over the ground — the
    /// pressure-wave dust ring under it.
    pub fn muzzle_blast(&mut self, muzzle: Vec3, direction: Vec3, ground_y: Option<f32>) {
        let direction = direction.normalize_or_zero();
        self.muzzle_flash(muzzle, direction);
        self.muzzle_smoke(muzzle, direction);
        if let Some(ground_y) = ground_y
            && muzzle.y - ground_y < MUZZLE_DUST_HEIGHT_M
        {
            self.ground_dust_ring(Vec3::new(muzzle.x, ground_y, muzzle.z) + direction * 1.2);
        }
    }

    /// Two-layer additive flash: a small white-hot core and a larger amber bloom, both gone in
    /// under a tenth of a second — the eye keeps the impression, not the sprite.
    fn muzzle_flash(&mut self, muzzle: Vec3, direction: Vec3) {
        self.spawn(Particle {
            position: muzzle + direction * 0.5,
            velocity_mps: direction * 6.0,
            gravity_factor: 0.0,
            drag_per_s: 10.0,
            age_s: 0.0,
            ttl_s: 0.05,
            size_begin_m: 1.0,
            size_end_m: 1.6,
            color_begin: [1.0, 0.95, 0.8, 0.0],
            color_end: [0.4, 0.3, 0.15, 0.0],
            stretch_s: 0.03,
        });
        self.spawn(Particle {
            position: muzzle + direction * 1.1,
            velocity_mps: direction * 9.0,
            gravity_factor: 0.0,
            drag_per_s: 9.0,
            age_s: 0.0,
            ttl_s: 0.09,
            size_begin_m: 1.7,
            size_end_m: 2.6,
            color_begin: [1.0, 0.62, 0.22, 0.0],
            color_end: [0.25, 0.1, 0.03, 0.0],
            stretch_s: 0.04,
        });
    }

    /// Propellant smoke: a fast cone along the bore that drags to a stop and drifts up, thinning
    /// as it grows. Premultiplied gray so it genuinely occludes what's behind it while it lives.
    fn muzzle_smoke(&mut self, muzzle: Vec3, direction: Vec3) {
        for _ in 0..8 {
            let spread = Vec3::new(self.rand_signed(), self.rand_signed(), self.rand_signed());
            let speed = 5.0 + self.rand_unit() * 6.0;
            let ttl = 1.1 + self.rand_unit() * 1.2;
            let shade = 0.30 + self.rand_unit() * 0.10;
            let bore_offset = 0.8 + self.rand_unit() * 0.8;
            let size_end = 2.6 + self.rand_unit();
            let alpha = 0.5;
            self.spawn(Particle {
                position: muzzle + direction * bore_offset,
                velocity_mps: direction * speed + spread * 1.6 + Vec3::Y * 0.7,
                gravity_factor: -0.02, // hot gas: a whisper of buoyancy
                drag_per_s: 2.4,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.7,
                size_end_m: size_end,
                color_begin: [shade * alpha, shade * alpha, (shade + 0.02) * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }

    /// The blast-pressure dust ring under a low muzzle: dirt-colored puffs thrown radially
    /// outward at ground level, heavier than smoke (they drag down, not up).
    fn ground_dust_ring(&mut self, center: Vec3) {
        for index in 0..10 {
            let angle = index as f32 / 10.0 * std::f32::consts::TAU + self.rand_unit() * 0.5;
            let radial = Vec3::new(angle.cos(), 0.0, angle.sin());
            let speed = 3.5 + self.rand_unit() * 3.0;
            let ttl = 0.9 + self.rand_unit() * 0.9;
            let lift = 0.8 + self.rand_unit();
            let alpha = 0.45;
            self.spawn(Particle {
                position: center + radial * 0.5 + Vec3::Y * 0.15,
                velocity_mps: radial * speed + Vec3::Y * lift,
                gravity_factor: 0.12,
                drag_per_s: 2.8,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.8,
                size_end_m: 2.4,
                color_begin: [0.42 * alpha, 0.36 * alpha, 0.26 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }
}

impl FxSystem {
    /// A puff of dust kicked up behind a tank's tracks as it rolls — sandy, low, thrown backward
    /// and outward and settling quickly. Used by the garage drive-in; the caller paces the cadence
    /// by travelled distance. `ground` is the contact point at floor level behind the hull.
    pub fn track_dust(&mut self, ground: Vec3) {
        for _ in 0..4 {
            let spread = Vec3::new(self.rand_signed() * 1.4, 0.0, self.rand_signed() * 0.8);
            let speed = 1.2 + self.rand_unit() * 1.6;
            let ttl = 0.7 + self.rand_unit() * 0.7;
            let lift = 0.4 + self.rand_unit() * 0.6;
            let alpha = 0.40;
            // Sandy dirt, biased backward (-Z) so the trail streams out behind the moving tank.
            self.spawn(Particle {
                position: ground + Vec3::Y * 0.1 + spread * 0.2,
                velocity_mps: spread * speed + Vec3::new(0.0, lift, -speed * 0.8),
                gravity_factor: 0.15,
                drag_per_s: 3.2,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.5,
                size_end_m: 1.8,
                color_begin: [0.44 * alpha, 0.39 * alpha, 0.29 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }

    /// One puff of the dead-engine column: darker and slower than gun smoke, rising off the
    /// deck and thinning as it climbs. The caller owns the emission cadence.
    pub fn engine_smoke_puff(&mut self, deck: Vec3) {
        let drift =
            Vec3::new(self.rand_signed() * 0.7, 1.4 + self.rand_unit(), self.rand_signed() * 0.7);
        let ttl = 2.0 + self.rand_unit() * 1.5;
        let shade = 0.07 + self.rand_unit() * 0.05;
        let scatter = Vec3::new(self.rand_signed() * 0.4, 0.1, self.rand_signed() * 0.4);
        let alpha = 0.55;
        self.spawn(Particle {
            position: deck + scatter,
            velocity_mps: drift,
            gravity_factor: -0.04,
            drag_per_s: 1.0,
            age_s: 0.0,
            ttl_s: ttl,
            size_begin_m: 0.8,
            size_end_m: 3.2,
            color_begin: [shade * alpha, shade * alpha, shade * alpha, alpha],
            color_end: [0.0, 0.0, 0.0, 0.0],
            stretch_s: 0.0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_low_muzzle_blast_spawns_flash_smoke_and_ground_dust() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, Some(0.0));
        // 2 flash + 8 smoke + 10 dust.
        assert_eq!(fx.live_particles(), 20);
    }

    #[test]
    fn a_high_muzzle_skips_the_ground_dust_ring() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 8.0, 0.0), Vec3::Z, Some(0.0));
        assert_eq!(fx.live_particles(), 10, "flash + smoke only");

        let mut no_ground = FxSystem::default();
        no_ground.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, None);
        assert_eq!(no_ground.live_particles(), 10, "no heightmap, no dust");
    }

    #[test]
    fn the_blast_is_over_in_seconds_not_minutes() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, Some(0.0));
        for _ in 0..80 {
            fx.tick(1.0 / 20.0); // 4 s of frames
        }
        assert_eq!(fx.live_particles(), 0, "every muzzle particle expires");
    }

    #[test]
    fn flash_rows_are_additive_and_smoke_rows_occlude() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, Some(0.0));
        let vertices = fx.vertices(Vec3::new(0.0, 2.0, -10.0), Vec3::new(0.0, 1.8, 0.0));
        let additive = vertices.iter().filter(|v| v.color[3] == 0.0).count();
        let occluding = vertices.iter().filter(|v| v.color[3] > 0.0).count();
        assert!(additive > 0, "flash quads blend additively (alpha 0)");
        assert!(occluding > 0, "smoke and dust quads carry real alpha");
    }
}
