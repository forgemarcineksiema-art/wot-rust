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
        self.armor_hit_directed(position, penetrated, ricocheted, None, 0.4);
    }

    /// A hit with the deflection direction known (from the wire's plate normal + shell
    /// direction): a ricochet's sparks leave ALONG the departure, not in an anonymous dome —
    /// the glance is readable at range from where the steel sprays.
    /// `damage_fraction` is the share of the target's pool the round took (Inny Poziom S9): the
    /// penetration signature grows with it, so a 900-HP hole reads bigger than a 120-HP graze.
    /// A non-penetration has its own answer now — a spall cloud of occluding grey off the
    /// plate — instead of sparks alone.
    pub fn armor_hit_directed(
        &mut self,
        position: Vec3,
        penetrated: bool,
        ricocheted: bool,
        departure: Option<Vec3>,
        damage_fraction: f32,
    ) {
        match departure.map(glam::Vec3::normalize_or_zero).filter(|d| d.length_squared() > 0.5) {
            Some(direction) if ricocheted => self.spark_fan_directed(position, 14, direction),
            _ => self.spark_fan(position, if ricocheted { 14 } else { 10 }),
        }
        if penetrated {
            self.penetration_signature(position, damage_fraction);
        } else {
            self.bounce_spall(position);
        }
    }

    /// An HE round bursting on armour (S9): the charge goes off on the plate — a hot core that
    /// swells and dies inside a tenth of a second, a ring of occluding smoke thrown back along
    /// the shell's approach, and a few sparks. The plate's own clang and the damage event's
    /// sparks come with the strike; this is the blast the AP path never had.
    pub fn he_burst_on_armour(&mut self, position: Vec3, direction: Vec3) {
        let back = -direction.normalize_or_zero();
        self.spawn(Particle {
            position: position + back * 0.4,
            velocity_mps: back * 3.0,
            gravity_factor: 0.0,
            drag_per_s: 6.0,
            age_s: 0.0,
            ttl_s: 0.12,
            size_begin_m: 2.0,
            size_end_m: 4.5,
            color_begin: [1.0, 0.62, 0.22, 0.0],
            color_end: [0.25, 0.06, 0.02, 0.0],
            stretch_s: 0.0,
        });
        // The smoke ring: a hemisphere facing back along the approach, heavier than gun smoke.
        let side = if back.abs().dot(Vec3::Y) > 0.9 {
            Vec3::X
        } else {
            back.cross(Vec3::Y).normalize_or_zero()
        };
        let up = side.cross(back).normalize_or_zero();
        for index in 0..10 {
            let angle = index as f32 / 10.0 * std::f32::consts::TAU + self.rand_unit() * 0.4;
            let radial = side * angle.cos() + up * angle.sin();
            let speed = 5.0 + self.rand_unit() * 3.0;
            let ttl = 1.4 + self.rand_unit() * 1.2;
            let shade = 0.10 + self.rand_unit() * 0.06;
            let alpha = 0.6;
            self.spawn(Particle {
                position: position + back * 0.3 + radial * 0.4,
                velocity_mps: radial * speed + back * 2.0 + Vec3::Y * 0.8,
                gravity_factor: -0.02,
                drag_per_s: 2.4,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.9,
                size_end_m: 3.4,
                color_begin: [shade * alpha, shade * alpha, shade * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
        self.spark_fan(position, 8);
    }

    /// The plate held (S9): what a bounce throws besides sparks — a short cloud of occluding
    /// grey spall and paint off the plate, gone in a second. The world's answer to a non-pen,
    /// readable at range where sparks are pixels.
    fn bounce_spall(&mut self, position: Vec3) {
        for _ in 0..5 {
            let drift = Vec3::new(
                self.rand_signed() * 2.0,
                0.6 + self.rand_unit() * 1.2,
                self.rand_signed() * 2.0,
            );
            let ttl = 0.6 + self.rand_unit() * 0.5;
            let shade = 0.32 + self.rand_unit() * 0.10;
            let alpha = 0.45;
            self.spawn(Particle {
                position: position + Vec3::Y * 0.05,
                velocity_mps: drift,
                gravity_factor: 0.08,
                drag_per_s: 3.0,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.5,
                size_end_m: 1.6,
                color_begin: [shade * alpha, shade * alpha, shade * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
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
            self.spark(position, direction);
        }
    }

    /// A fan biased along a departure direction: most sparks ride the deflection cone, a few
    /// still scatter wide (steel chips every which way, but the ENERGY leaves one way).
    fn spark_fan_directed(&mut self, position: Vec3, count: u32, departure: Vec3) {
        for index in 0..count {
            let scatter =
                Vec3::new(self.rand_signed(), 0.3 + self.rand_unit() * 0.7, self.rand_signed())
                    .normalize_or_zero();
            let direction = if index % 4 == 0 {
                scatter
            } else {
                (departure * 1.6 + scatter * 0.6).normalize_or_zero()
            };
            self.spark(position, direction);
        }
    }

    /// One white-hot chip of steel on its way out.
    fn spark(&mut self, position: Vec3, direction: Vec3) {
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

    /// The penetration signature: one hard entry flash plus dark propellant/fuel smoke rolling
    /// out of the hole — the read-at-range cue that the shell went inside.
    fn penetration_signature(&mut self, position: Vec3, damage_fraction: f32) {
        // 0.6 at a graze, 1.0 at the 40 % a typical pen takes, 1.5 at a near-kill.
        let scale = (0.6 + damage_fraction.clamp(0.0, 1.0)).clamp(0.6, 1.5);
        self.spawn(Particle {
            position,
            velocity_mps: Vec3::ZERO,
            gravity_factor: 0.0,
            drag_per_s: 0.0,
            age_s: 0.0,
            ttl_s: 0.09,
            size_begin_m: 1.4 * scale,
            size_end_m: 2.4 * scale,
            color_begin: [1.0, 0.72, 0.30, 0.0],
            color_end: [0.3, 0.08, 0.02, 0.0],
            stretch_s: 0.0,
        });
        let smoke = (7.0 * scale).round() as usize;
        for _ in 0..smoke {
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
        // Inny Poziom S9: a bounce is no longer sparks alone — the plate answers with at least
        // one occluding row of spall; the penetration still carries its dark smoke rows.
        let bounce_vertices = bounce.vertices(Vec3::new(0.0, 1.0, -8.0), Vec3::ZERO);
        assert!(bounce_vertices.iter().any(|vertex| vertex.color[3] > 0.0), "spall occludes");
        assert!(
            bounce_vertices.iter().any(|vertex| vertex.color[3] == 0.0),
            "sparks stay additive"
        );
        let pen_vertices = pen.vertices(Vec3::new(0.0, 1.0, -8.0), Vec3::ZERO);
        assert!(pen_vertices.iter().any(|vertex| vertex.color[3] > 0.0));
    }

    /// D6's audit lock: with the deflection known (plate normal + shell direction ride the
    /// wire since v19), a ricochet's sparks leave ALONG the departure - the energy has a
    /// direction, not an anonymous dome.
    #[test]
    fn directed_ricochet_sparks_leave_along_the_departure() {
        let departure = Vec3::X;
        let mut fx = FxSystem::default();
        fx.armor_hit_directed(Vec3::ZERO, false, true, Some(departure), 0.0);
        let mean: Vec3 =
            fx.particles.iter().map(|p| p.velocity_mps.normalize_or_zero()).sum::<Vec3>()
                / fx.particles.len() as f32;
        assert!(
            mean.dot(departure) > 0.45,
            "the spark fan must lean along the deflection, mean alignment {}",
            mean.dot(departure)
        );

        // Without the direction the fan stays the old anonymous dome.
        let mut blind = FxSystem::default();
        blind.armor_hit_directed(Vec3::ZERO, false, true, None, 0.0);
        let blind_mean: Vec3 =
            blind.particles.iter().map(|p| p.velocity_mps.normalize_or_zero()).sum::<Vec3>()
                / blind.particles.len() as f32;
        assert!(
            blind_mean.dot(departure).abs() < 0.35,
            "an unknown deflection must not invent a lean, got {}",
            blind_mean.dot(departure)
        );
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
    /// Inny Poziom S9: the penetration signature grows with the share of the pool the round
    /// took, and even the smallest pen out-areas the bounce's spark fan by the register's 4×.
    fn the_penetration_signature_scales_with_damage_and_dwarfs_the_spark_fan() {
        let area = |fx: &FxSystem| -> f32 {
            fx.particles.iter().map(|p| p.size_begin_m * p.size_begin_m).sum()
        };
        let mut sparks = FxSystem::default();
        sparks.spark_fan(Vec3::ZERO, 10);
        let mut graze = FxSystem::default();
        graze.armor_hit_directed(Vec3::ZERO, true, false, None, 0.05);
        let mut kill = FxSystem::default();
        kill.armor_hit_directed(Vec3::ZERO, true, false, None, 0.9);
        assert!(area(&graze) >= 4.0 * area(&sparks), "{} vs {}", area(&graze), area(&sparks));
        assert!(area(&kill) > area(&graze) * 1.5, "a near-kill flashes bigger than a graze");
        assert!(kill.live_particles() > graze.live_particles(), "and rolls more smoke");
    }

    /// Inny Poziom S9: an HE round bursting on armour is a blast — a hot additive core, an
    /// occluding smoke ring thrown back along the approach, sparks — not the AP path's fan.
    #[test]
    fn an_he_round_on_armour_bursts_instead_of_sparking() {
        let mut he = FxSystem::default();
        he.he_burst_on_armour(Vec3::new(0.0, 1.5, 50.0), Vec3::Z);
        let mut ap = FxSystem::default();
        ap.impact_burst(Vec3::new(0.0, 1.5, 50.0), ImpactSurface::Hull);
        assert!(he.live_particles() > 2 * ap.live_particles());
        let core = he.particles.iter().map(|p| p.size_begin_m).fold(0.0_f32, f32::max);
        assert!(core >= 2.0, "the fireball core is metres across: {core}");
        let smoke: Vec<&Particle> =
            he.particles.iter().filter(|p| p.color_begin[3] > 0.0).collect();
        assert!(smoke.len() >= 8, "an occluding smoke ring: {}", smoke.len());
        let back = smoke.iter().map(|p| p.velocity_mps.z).sum::<f32>() / smoke.len() as f32;
        assert!(back < 0.0, "the ring is thrown back along the approach: mean z {back}");
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
