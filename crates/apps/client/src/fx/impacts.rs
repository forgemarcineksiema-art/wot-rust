//! Impact effect recipes: what a shell's death actually looks like, by what it died against.
//! Terrain swallows the shot in a dirt fountain, cover chips into pale dust and sparks, armor
//! answers with a spark fan — and a penetration adds the flash-and-black-smoke signature that
//! reads as "that one went in" from 400 m away. Colors premultiplied like every FX recipe.

use game_core::ImpactSurface;
use glam::Vec3;

use super::{FxSystem, Particle};

impl FxSystem {
    /// A shell absorbed without damage: dispatch on the surface the server reported.
    pub fn impact_burst(&mut self, position: Vec3, surface: ImpactSurface) {
        match surface {
            ImpactSurface::Terrain => self.dirt_fountain(position),
            ImpactSurface::Cover => self.masonry_burst(position),
            ImpactSurface::Hull => self.spark_fan(position, 8),
            ImpactSurface::Water => self.water_splash(position),
        }
    }

    /// A shell dying in the river: a bright water column thrown straight up (heavier and
    /// taller than dirt clods — water holds together), a ring of low flat spray, and a brief
    /// pale mist. No sparks, no dust — water answers differently than ground.
    fn water_splash(&mut self, position: Vec3) {
        // The column: tall, narrow, falls back hard.
        for _ in 0..10 {
            let lateral = Vec3::new(self.rand_signed(), 0.0, self.rand_signed()) * 1.2;
            let up = 7.0 + self.rand_unit() * 5.0;
            let ttl = 0.8 + self.rand_unit() * 0.5;
            let alpha = 0.8;
            self.spawn(Particle {
                position: position + Vec3::Y * 0.1,
                velocity_mps: lateral + Vec3::Y * up,
                gravity_factor: 1.0,
                drag_per_s: 0.6,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.30,
                size_end_m: 0.75,
                color_begin: [0.72 * alpha, 0.78 * alpha, 0.82 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.08,
            });
        }
        // The ring: fast, flat, short-lived spray skating outward on the surface.
        for _ in 0..8 {
            let direction =
                Vec3::new(self.rand_signed(), 0.12, self.rand_signed()).normalize_or_zero();
            let speed = 6.0 + self.rand_unit() * 4.0;
            let ttl = 0.35 + self.rand_unit() * 0.25;
            let alpha = 0.55;
            self.spawn(Particle {
                position: position + Vec3::Y * 0.15,
                velocity_mps: direction * speed,
                gravity_factor: 0.8,
                drag_per_s: 1.4,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.25,
                size_end_m: 0.5,
                color_begin: [0.60 * alpha, 0.68 * alpha, 0.72 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.05,
            });
        }
        // The mist: a soft pale veil hanging where the column collapsed.
        self.dust_pall(position, [0.66, 0.72, 0.75], 4);
    }

    /// A hit on a live tank. Every armor strike throws sparks; a ricochet throws them longer and
    /// shallower (the shell skips away), and a penetration adds the entry flash and a column of
    /// dark smoke out of the hole.
    pub fn armor_hit(&mut self, position: Vec3, penetrated: bool, ricocheted: bool) {
        self.spark_fan(position, if ricocheted { 14 } else { 10 });
        if penetrated {
            self.penetration_signature(position);
        }
    }

    /// Dirt fountain: heavy clods thrown mostly upward that arc back down under gravity, inside
    /// a slower pall of hanging dust.
    fn dirt_fountain(&mut self, position: Vec3) {
        for _ in 0..12 {
            let lateral = Vec3::new(self.rand_signed(), 0.0, self.rand_signed()) * 3.5;
            let up = 4.5 + self.rand_unit() * 4.0;
            let ttl = 0.7 + self.rand_unit() * 0.7;
            let alpha = 0.85;
            self.spawn(Particle {
                position: position + Vec3::Y * 0.2,
                velocity_mps: lateral + Vec3::Y * up,
                gravity_factor: 0.9,
                drag_per_s: 0.8,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.35,
                size_end_m: 0.9,
                color_begin: [0.30 * alpha, 0.24 * alpha, 0.16 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.05,
            });
        }
        self.dust_pall(position, [0.40, 0.34, 0.24], 6);
    }

    /// Stone/cover strike: paler dust than soil, plus a few hot chips.
    fn masonry_burst(&mut self, position: Vec3) {
        self.spark_fan(position, 5);
        self.dust_pall(position, [0.52, 0.50, 0.46], 6);
    }

    /// The hanging cloud any hard impact leaves: slow, buoyancy-neutral puffs that grow and thin.
    fn dust_pall(&mut self, position: Vec3, tone: [f32; 3], count: u32) {
        for _ in 0..count {
            let drift = Vec3::new(
                self.rand_signed() * 1.6,
                0.6 + self.rand_unit(),
                self.rand_signed() * 1.6,
            );
            let ttl = 1.4 + self.rand_unit() * 1.4;
            let alpha = 0.5;
            self.spawn(Particle {
                position: position + Vec3::Y * 0.3,
                velocity_mps: drift,
                gravity_factor: 0.02,
                drag_per_s: 1.6,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.9,
                size_end_m: 3.0,
                color_begin: [tone[0] * alpha, tone[1] * alpha, tone[2] * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }

    /// Hot metal sparks off armor or rock: fast additive streaks that die in a blink and drop as
    /// they fly. This is the non-pen answer — bright, dry, and clearly not an explosion.
    fn spark_fan(&mut self, position: Vec3, count: u32) {
        for _ in 0..count {
            let direction =
                Vec3::new(self.rand_signed(), 0.3 + self.rand_unit() * 0.7, self.rand_signed())
                    .normalize_or_zero();
            let speed = 9.0 + self.rand_unit() * 9.0;
            let ttl = 0.15 + self.rand_unit() * 0.25;
            self.spawn(Particle {
                position,
                velocity_mps: direction * speed,
                gravity_factor: 0.5,
                drag_per_s: 2.0,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.10,
                size_end_m: 0.05,
                color_begin: [1.0, 0.75, 0.35, 0.0],
                color_end: [0.4, 0.12, 0.02, 0.0],
                stretch_s: 0.04,
            });
        }
    }

    /// The penetration signature: one hard entry flash plus dark propellant/fuel smoke rolling
    /// out of the hole — the read-at-range cue that the shell went inside.
    fn penetration_signature(&mut self, position: Vec3) {
        self.spawn(Particle {
            position,
            velocity_mps: Vec3::ZERO,
            gravity_factor: 0.0,
            drag_per_s: 0.0,
            age_s: 0.0,
            ttl_s: 0.09,
            size_begin_m: 1.4,
            size_end_m: 2.4,
            color_begin: [1.0, 0.72, 0.30, 0.0],
            color_end: [0.3, 0.08, 0.02, 0.0],
            stretch_s: 0.0,
        });
        for _ in 0..7 {
            let drift = Vec3::new(
                self.rand_signed() * 1.2,
                1.2 + self.rand_unit() * 1.4,
                self.rand_signed() * 1.2,
            );
            let ttl = 1.6 + self.rand_unit() * 1.6;
            let shade = 0.05 + self.rand_unit() * 0.04;
            let alpha = 0.6;
            self.spawn(Particle {
                position: position + Vec3::Y * 0.1,
                velocity_mps: drift,
                gravity_factor: -0.03,
                drag_per_s: 1.2,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.6,
                size_end_m: 2.6,
                color_begin: [shade * alpha, shade * alpha, shade * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_surface_speaks_its_own_burst() {
        let mut terrain = FxSystem::default();
        terrain.impact_burst(Vec3::ZERO, ImpactSurface::Terrain);
        assert_eq!(terrain.live_particles(), 18, "12 clods + 6 dust");

        let mut cover = FxSystem::default();
        cover.impact_burst(Vec3::ZERO, ImpactSurface::Cover);
        assert_eq!(cover.live_particles(), 11, "5 sparks + 6 dust");

        let mut hull = FxSystem::default();
        hull.impact_burst(Vec3::ZERO, ImpactSurface::Hull);
        assert_eq!(hull.live_particles(), 8, "sparks only — a wreck rings, it does not crater");

        let mut water = FxSystem::default();
        water.impact_burst(Vec3::ZERO, ImpactSurface::Water);
        assert_eq!(water.live_particles(), 22, "10 column + 8 ring + 4 mist — the splash");
    }

    #[test]
    fn a_penetration_adds_flash_and_dark_smoke_over_a_bounce() {
        let mut bounce = FxSystem::default();
        bounce.armor_hit(Vec3::ZERO, false, false);
        let mut pen = FxSystem::default();
        pen.armor_hit(Vec3::ZERO, true, false);

        assert!(pen.live_particles() > bounce.live_particles());
        // The bounce is all additive sparks; the penetration carries occluding smoke rows.
        let bounce_vertices = bounce.vertices(Vec3::new(0.0, 1.0, -8.0), Vec3::ZERO);
        assert!(bounce_vertices.iter().all(|vertex| vertex.color[3] == 0.0));
        let pen_vertices = pen.vertices(Vec3::new(0.0, 1.0, -8.0), Vec3::ZERO);
        assert!(pen_vertices.iter().any(|vertex| vertex.color[3] > 0.0));
    }

    #[test]
    fn a_ricochet_throws_more_sparks_than_a_flat_bounce() {
        let mut bounce = FxSystem::default();
        bounce.armor_hit(Vec3::ZERO, false, false);
        let mut ricochet = FxSystem::default();
        ricochet.armor_hit(Vec3::ZERO, false, true);
        assert!(ricochet.live_particles() > bounce.live_particles());
    }

    #[test]
    fn dirt_clods_fall_back_down_while_dust_hangs() {
        let mut fx = FxSystem::default();
        fx.impact_burst(Vec3::ZERO, ImpactSurface::Terrain);
        for _ in 0..30 {
            fx.tick(1.0 / 30.0); // one second
        }
        // The heavy clods (ttl <= 1.4 s) are mostly gone; the pall (up to 2.8 s) lingers.
        let left = fx.live_particles();
        assert!(left > 0, "the dust pall outlives the clods");
        assert!(left < 18, "the clods themselves die within the second, {left} left");
    }
}
