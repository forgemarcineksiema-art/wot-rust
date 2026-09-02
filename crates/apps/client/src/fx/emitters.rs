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
    /// `recoil_scale` is the round's (`ShellSpec::recoil_scale`, Inny Poziom S3): 1.0 keeps
    /// the numbers every layer was tuned on for the D-10; the flash grows in size, the smoke
    /// and the dust ring in count and speed, so a 128 mm blast is bigger than a 75 mm one in
    /// the same way the sound already is.
    pub fn muzzle_blast(
        &mut self,
        muzzle: Vec3,
        direction: Vec3,
        ground_y: Option<f32>,
        recoil_scale: f32,
    ) {
        let direction = direction.normalize_or_zero();
        let scale = recoil_scale.clamp(0.4, 2.0);
        self.muzzle_flash(muzzle, direction, scale);
        self.muzzle_smoke(muzzle, direction, scale);
        if let Some(ground_y) = ground_y
            && muzzle.y - ground_y < MUZZLE_DUST_HEIGHT_M
        {
            self.ground_dust_ring(Vec3::new(muzzle.x, ground_y, muzzle.z) + direction * 1.2, scale);
        }
    }

    /// Two-layer additive flash: a small white-hot core and a larger amber bloom, both gone in
    /// under a tenth of a second — the eye keeps the impression, not the sprite.
    fn muzzle_flash(&mut self, muzzle: Vec3, direction: Vec3, scale: f32) {
        self.spawn(Particle {
            position: muzzle + direction * 0.5,
            velocity_mps: direction * 6.0 * scale,
            gravity_factor: 0.0,
            drag_per_s: 10.0,
            age_s: 0.0,
            ttl_s: 0.05,
            size_begin_m: 1.0 * scale,
            size_end_m: 1.6 * scale,
            color_begin: [1.0, 0.95, 0.8, 0.0],
            color_end: [0.4, 0.3, 0.15, 0.0],
            stretch_s: 0.03,
        });
        self.spawn(Particle {
            position: muzzle + direction * 1.1,
            velocity_mps: direction * 9.0 * scale,
            gravity_factor: 0.0,
            drag_per_s: 9.0,
            age_s: 0.0,
            ttl_s: 0.09,
            size_begin_m: 1.7 * scale,
            size_end_m: 2.6 * scale,
            color_begin: [1.0, 0.62, 0.22, 0.0],
            color_end: [0.25, 0.1, 0.03, 0.0],
            stretch_s: 0.04,
        });
    }

    /// Propellant smoke: a fast cone along the bore that drags to a stop and drifts up, thinning
    /// as it grows. Premultiplied gray so it genuinely occludes what's behind it while it lives.
    fn muzzle_smoke(&mut self, muzzle: Vec3, direction: Vec3, scale: f32) {
        let count = (8.0 * scale).round().max(2.0) as usize;
        for _ in 0..count {
            let spread = Vec3::new(self.rand_signed(), self.rand_signed(), self.rand_signed());
            let speed = (5.0 + self.rand_unit() * 6.0) * scale.sqrt();
            let ttl = 1.1 + self.rand_unit() * 1.2;
            let shade = 0.30 + self.rand_unit() * 0.10;
            let bore_offset = 0.8 + self.rand_unit() * 0.8;
            let size_end = (2.6 + self.rand_unit()) * scale.sqrt();
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

    /// Dust shaken off the hull by the shot (Inny Poziom S4): the deck around the turret ring
    /// sheds what it carried — a low, brief lift of sandy motes that settle back onto the
    /// plates, more of them for a heavier round. `deck` is the turret ring on the hull roof in
    /// world space; the puffs spread along and across the hull's heading.
    pub fn hull_dust(&mut self, deck: Vec3, hull_yaw_rad: f32, recoil_scale: f32) {
        let scale = recoil_scale.clamp(0.4, 2.0);
        let count = (6.0 * scale).round().max(2.0) as usize;
        let forward = Vec3::new(hull_yaw_rad.sin(), 0.0, hull_yaw_rad.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        for _ in 0..count {
            let spread = forward * (self.rand_signed() * 1.8) + right * (self.rand_signed() * 1.1);
            let lift = (0.5 + self.rand_unit() * 0.7) * scale.sqrt();
            let ttl = 0.5 + self.rand_unit() * 0.5;
            let alpha = 0.30;
            self.spawn(Particle {
                position: deck + spread + Vec3::Y * 0.05,
                velocity_mps: Vec3::Y * lift + spread * 0.2,
                gravity_factor: 0.10,
                drag_per_s: 3.5,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.35,
                size_end_m: 1.1 * scale.sqrt(),
                color_begin: [0.44 * alpha, 0.39 * alpha, 0.29 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }

    /// The blast-pressure dust ring under a low muzzle: dirt-colored puffs thrown radially
    /// outward at ground level, heavier than smoke (they drag down, not up).
    fn ground_dust_ring(&mut self, center: Vec3, scale: f32) {
        let count = (10.0 * scale).round().max(4.0) as usize;
        for index in 0..count {
            let angle =
                index as f32 / count as f32 * std::f32::consts::TAU + self.rand_unit() * 0.5;
            let radial = Vec3::new(angle.cos(), 0.0, angle.sin());
            let speed = (3.5 + self.rand_unit() * 3.0) * scale.sqrt();
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
                size_end_m: 2.4 * scale.sqrt(),
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
    /// Continuous rolling dust (D1): ONE puff per call, streamed behind the moving hull along
    /// its real heading — the caller owns the cadence per track side. Lighter and lower than
    /// the event burst below; a column on the move reads as a drifting ribbon, not explosions.
    pub fn rolling_dust(&mut self, ground: Vec3, heading: Vec3, intensity: f32) {
        let spread = Vec3::new(self.rand_signed() * 0.5, 0.0, self.rand_signed() * 0.4);
        let ttl = 0.9 + self.rand_unit() * 0.8;
        let lift = 0.5 + 0.8 * self.rand_unit();
        let alpha = 0.20 + 0.18 * intensity;
        self.spawn(Particle {
            position: ground + Vec3::Y * 0.08 + spread * 0.3,
            velocity_mps: -heading * (1.0 + 2.4 * intensity) + Vec3::new(spread.x, lift, spread.z),
            gravity_factor: 0.10,
            drag_per_s: 2.6,
            age_s: 0.0,
            ttl_s: ttl,
            size_begin_m: 0.35,
            size_end_m: 1.5 + intensity,
            color_begin: [0.44 * alpha, 0.39 * alpha, 0.29 * alpha, alpha],
            color_end: [0.0, 0.0, 0.0, 0.0],
            stretch_s: 0.0,
        });
    }

    /// One throw of the bow spray (D7): white water shouldered up and forward off the leading
    /// edge, falling back under real gravity, with a low mist that hangs a breath longer.
    pub fn bow_splash(&mut self, waterline: Vec3, heading: Vec3, intensity: f32) {
        let jitter = Vec3::new(self.rand_signed() * 0.6, 0.0, self.rand_signed() * 0.3);
        let lift = 1.6 + 2.2 * self.rand_unit() * intensity;
        let ttl = 0.45 + self.rand_unit() * 0.35;
        let alpha = 0.30 + 0.25 * intensity;
        self.spawn(Particle {
            position: waterline + jitter * 0.4,
            velocity_mps: heading * (1.2 + 2.0 * intensity) + jitter + Vec3::Y * lift,
            gravity_factor: 0.9,
            drag_per_s: 1.4,
            age_s: 0.0,
            ttl_s: ttl,
            size_begin_m: 0.25,
            size_end_m: 0.7,
            color_begin: [0.30 * alpha, 0.34 * alpha, 0.36 * alpha, alpha],
            color_end: [0.0, 0.0, 0.0, 0.0],
            stretch_s: 0.03,
        });
        let mist_ttl = 0.8 + self.rand_unit() * 0.5;
        let mist_alpha = 0.10 + 0.10 * intensity;
        self.spawn(Particle {
            position: waterline + Vec3::Y * 0.15,
            velocity_mps: heading * 0.8 + Vec3::Y * 0.5,
            gravity_factor: 0.05,
            drag_per_s: 2.2,
            age_s: 0.0,
            ttl_s: mist_ttl,
            size_begin_m: 0.5,
            size_end_m: 1.6,
            color_begin: [0.26 * mist_alpha, 0.30 * mist_alpha, 0.32 * mist_alpha, mist_alpha],
            color_end: [0.0, 0.0, 0.0, 0.0],
            stretch_s: 0.0,
        });
    }

    /// One breath of the working exhaust (D1): a small blue-grey puff off the port, denser
    /// under load. Rises briefly, thins fast — an idling column smokes lightly, a charge reads.
    pub fn exhaust_puff(&mut self, port: Vec3, load: f32) {
        let ttl = 0.7 + self.rand_unit() * 0.5;
        let jitter = Vec3::new(self.rand_signed() * 0.1, 0.0, self.rand_signed() * 0.1);
        let sway = self.rand_signed() * 0.3;
        let alpha = 0.16 + 0.22 * load;
        self.spawn(Particle {
            position: port + jitter,
            velocity_mps: Vec3::new(sway, 1.1 + load, -0.4),
            gravity_factor: -0.02,
            drag_per_s: 1.8,
            age_s: 0.0,
            ttl_s: ttl,
            size_begin_m: 0.22,
            size_end_m: 0.9 + 0.5 * load,
            color_begin: [0.16 * alpha, 0.17 * alpha, 0.19 * alpha, alpha],
            color_end: [0.0, 0.0, 0.0, 0.0],
            stretch_s: 0.0,
        });
    }

    /// The hangar's sun-shaft blades as FX quads (Hala 3.0 E1): soft additive beams built
    /// from the hangar's own blade geometry. NEGATIVE sharpness is the shaft tag — the FX
    /// shader runs its drifting-density modulation only on tagged quads, so every battle
    /// particle (always positive) renders bit-identically.
    pub fn hangar_shaft_vertices(blades: &[[[f32; 3]; 4]]) -> Vec<renderer_api::FxVertex> {
        const SHAFT_SHARPNESS: f32 = -0.85;
        // Additive warm glow, alpha 0 (pure light, no coverage). The first candidate (0.045)
        // vanished against the whitewashed walls — additive haze must clear the bright
        // background by a visible step, and the drift modulation still takes it down by a
        // third on average. Trimmed 0.20 -> 0.13 (Światło służy czołgowi): the beams were
        // calibrated against a floor with no shadow lattice; once the bars printed under
        // them the bright blades became the second layer of the same grid. Quieter beams
        // also widen HERO_OVER_ROOM's margin — the E1 pricing ran the other way.
        const GLOW: [f32; 4] = [0.13, 0.118, 0.09, 0.0];
        let mut vertices = Vec::with_capacity(blades.len() * 6);
        for quad in blades {
            let corner = |index: usize, uv: [f32; 2]| renderer_api::FxVertex {
                position: quad[index],
                uv,
                sharpness: SHAFT_SHARPNESS,
                color: GLOW,
            };
            let (a, b, c, d) = (
                corner(0, [-1.0, -1.0]),
                corner(1, [1.0, -1.0]),
                corner(2, [1.0, 1.0]),
                corner(3, [-1.0, 1.0]),
            );
            vertices.extend_from_slice(&[a, b, c, a, c, d]);
        }
        vertices
    }

    /// Dust motes drifting in the hangar's sun shafts (Hala 3.0 E1): a slow trickle of tiny,
    /// long-lived, faintly warm additive specks spawned INSIDE the blade volumes — dust is
    /// only visible where the light is, which is the whole honesty of the effect. The caller
    /// hands the blade quads (from `scene_build::hangar::sun_shaft_quads`) and the frame dt;
    /// the accumulator keeps the trickle rate framerate-independent.
    pub fn hangar_motes(&mut self, blades: &[[[f32; 3]; 4]], dt: f32) {
        const MOTES_PER_SECOND: f32 = 2.5;
        self.mote_budget += MOTES_PER_SECOND * dt;
        while self.mote_budget >= 1.0 {
            self.mote_budget -= 1.0;
            let quad = &blades[(self.rand_unit() * blades.len() as f32) as usize % blades.len()];
            // A point inside the blade: lerp across the top edge, then down the beam.
            let across = self.rand_unit();
            let down = 0.15 + self.rand_unit() * 0.8;
            let top = Vec3::from_array(quad[0]).lerp(Vec3::from_array(quad[1]), across);
            let bottom = Vec3::from_array(quad[3]).lerp(Vec3::from_array(quad[2]), across);
            let position = top.lerp(bottom, down);
            let glow = 0.05 + self.rand_unit() * 0.05;
            let drift = Vec3::new(
                self.rand_signed() * 0.04,
                -0.02 - self.rand_unit() * 0.03,
                self.rand_signed() * 0.04,
            );
            let ttl = 6.0 + self.rand_unit() * 6.0;
            let size = 0.03 + self.rand_unit() * 0.05;
            self.spawn(Particle {
                position,
                velocity_mps: drift,
                gravity_factor: 0.0,
                drag_per_s: 0.0,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: size,
                size_end_m: 0.02,
                // Pure additive glow (alpha 0) — a mote IS light caught, not a body.
                color_begin: [glow, glow * 0.92, glow * 0.75, 0.0],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }

    /// A wisp of steam off the hall's heating duct (Hala 3.0 E2): sparse, soft, premultiplied
    /// puffs that rise, drift with the hall's draft toward the gate, and thin out. Rate-
    /// budgeted like the motes; the caller passes the duct outlet.
    pub fn vent_steam(&mut self, outlet: Vec3, dt: f32) {
        const PUFFS_PER_SECOND: f32 = 1.1;
        self.steam_budget += PUFFS_PER_SECOND * dt;
        while self.steam_budget >= 1.0 {
            self.steam_budget -= 1.0;
            let jitter = Vec3::new(self.rand_signed() * 0.1, 0.0, self.rand_signed() * 0.1);
            let ttl = 1.8 + self.rand_unit() * 1.4;
            let rise = 0.5 + self.rand_unit() * 0.3;
            let drift = -0.25 - self.rand_unit() * 0.2;
            let alpha = 0.10 + self.rand_unit() * 0.04;
            self.spawn(Particle {
                position: outlet + jitter,
                velocity_mps: Vec3::new(0.05, rise, drift),
                gravity_factor: -0.03,
                drag_per_s: 1.1,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.22,
                size_end_m: 0.95,
                color_begin: [0.72 * alpha, 0.73 * alpha, 0.75 * alpha, alpha],
                color_end: [0.0, 0.0, 0.0, 0.0],
                stretch_s: 0.0,
            });
        }
    }

    /// The welding arc behind the second bay's screen (Hala 3.0 K1): while the arc BURNS
    /// (`hangar::welding_burn_at` — deterministic duty cycle, quiet at the goldens' frozen
    /// second), a fountain of white-hot sparks rises over the screen edge and dies falling —
    /// readable light, the sparks ARE the source. Live-only randomness, the same standing
    /// exemption the motes and the steam carry.
    pub fn welding_sparks(&mut self, corner: Vec3, burning: bool, dt: f32) {
        if !burning {
            return;
        }
        const SPARKS_PER_SECOND: f32 = 42.0;
        self.spark_budget += SPARKS_PER_SECOND * dt;
        while self.spark_budget >= 1.0 {
            self.spark_budget -= 1.0;
            let spread = Vec3::new(self.rand_signed() * 0.9, 0.0, self.rand_signed() * 0.9);
            let up = 2.2 + self.rand_unit() * 1.8;
            let ttl = 0.35 + self.rand_unit() * 0.45;
            let jitter = Vec3::new(self.rand_signed() * 0.2, 0.0, self.rand_signed() * 0.2);
            // White-hot dying to ember orange; pure additive.
            self.spawn(Particle {
                position: corner + jitter,
                velocity_mps: spread + Vec3::Y * up,
                gravity_factor: 0.9,
                drag_per_s: 1.6,
                age_s: 0.0,
                ttl_s: ttl,
                size_begin_m: 0.035,
                size_end_m: 0.015,
                color_begin: [1.0, 0.92, 0.72, 0.0],
                color_end: [0.55, 0.16, 0.02, 0.0],
                stretch_s: 0.05,
            });
        }
    }

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
    /// One tongue of open flame off a burning engine deck (the authoritative `engine_fire`
    /// flag). Additive hot core that rises fast and dies young — the fire reads as licks of
    /// flame, while the black column above it stays the smoke emitter's job.
    pub fn engine_fire_flame(&mut self, deck: Vec3) {
        let rise = Vec3::new(
            self.rand_signed() * 0.5,
            2.2 + self.rand_unit() * 1.2,
            self.rand_signed() * 0.5,
        );
        let scatter = Vec3::new(self.rand_signed() * 0.5, 0.05, self.rand_signed() * 0.5);
        let ttl = 0.35 + self.rand_unit() * 0.4;
        self.spawn(Particle {
            position: deck + scatter,
            velocity_mps: rise,
            gravity_factor: -0.10,
            drag_per_s: 1.6,
            age_s: 0.0,
            ttl_s: ttl,
            size_begin_m: 0.55,
            size_end_m: 0.15,
            // Premultiplied, alpha 0: purely additive heat. Orange core cooling to deep red.
            color_begin: [1.0, 0.42, 0.10, 0.0],
            color_end: [0.35, 0.05, 0.01, 0.0],
            stretch_s: 0.04,
        });
    }

    pub fn engine_smoke_puff(&mut self, deck: Vec3) {
        let drift =
            Vec3::new(self.rand_signed() * 0.7, 1.4 + self.rand_unit(), self.rand_signed() * 0.7);
        // F6 fill-rate diet: the burning ENGINE is the player's own deck — these quads sit
        // meters from the eye and stack into near-fullscreen blended fill on the minimum
        // spec. A tighter, shorter column reads the same story for a fraction of the fill.
        let ttl = 1.6 + self.rand_unit() * 1.0;
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
            size_end_m: 2.3,
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
        fx.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, Some(0.0), 1.0);
        // 2 flash + 8 smoke + 10 dust.
        assert_eq!(fx.live_particles(), 20);
    }

    /// Inny Poziom S4: the shot shakes dust off the deck, and a heavier round shakes more —
    /// and every mote lifts off the plates, none falls through them.
    #[test]
    fn a_shot_shakes_dust_off_the_deck_in_proportion_to_its_recoil() {
        let deck = Vec3::new(0.0, 1.6, 0.0);
        let mut light = FxSystem::default();
        light.hull_dust(deck, 0.0, 0.67);
        let mut heavy = FxSystem::default();
        heavy.hull_dust(deck, 0.0, 1.36);
        assert!(light.live_particles() >= 2, "even a 75 mm shakes the deck");
        assert!(
            heavy.live_particles() > light.live_particles(),
            "a 128 mm shakes more than a 75 mm: {} vs {}",
            heavy.live_particles(),
            light.live_particles()
        );
        assert!(
            heavy.particles.iter().all(|p| p.position.y >= deck.y && p.velocity_mps.y > 0.0),
            "dust lifts off the plates"
        );
    }

    #[test]
    fn a_high_muzzle_skips_the_ground_dust_ring() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 8.0, 0.0), Vec3::Z, Some(0.0), 1.0);
        assert_eq!(fx.live_particles(), 10, "flash + smoke only");

        let mut no_ground = FxSystem::default();
        no_ground.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, None, 1.0);
        assert_eq!(no_ground.live_particles(), 10, "no heightmap, no dust");
    }

    #[test]
    fn the_blast_is_over_in_seconds_not_minutes() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, Some(0.0), 1.0);
        for _ in 0..80 {
            fx.tick(1.0 / 20.0); // 4 s of frames
        }
        assert_eq!(fx.live_particles(), 0, "every muzzle particle expires");
    }

    #[test]
    fn flash_rows_are_additive_and_smoke_rows_occlude() {
        let mut fx = FxSystem::default();
        fx.muzzle_blast(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, Some(0.0), 1.0);
        let vertices = fx.vertices(Vec3::new(0.0, 2.0, -10.0), Vec3::new(0.0, 1.8, 0.0));
        let additive = vertices.iter().filter(|v| v.color[3] == 0.0).count();
        let occluding = vertices.iter().filter(|v| v.color[3] > 0.0).count();
        assert!(additive > 0, "flash quads blend additively (alpha 0)");
        assert!(occluding > 0, "smoke and dust quads carry real alpha");
    }
}
