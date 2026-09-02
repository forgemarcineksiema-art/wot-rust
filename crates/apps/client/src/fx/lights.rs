//! The shot and the hit as LIGHT (Inny Poziom S1). The renderer had the sun and nothing
//! else: a 100 mm gun at dusk lit neither its own glacis nor the ground under the muzzle, and
//! a shell's death lit nothing — the single largest reason a shot read as a decal. The
//! profile's local light slots (`LocalLight`, six of them, all-off on every outdoor profile)
//! are the pool the flash pours into: a pulse is born on the FX clock with the flash and dies
//! with it, and every frame the six strongest live pulses ride the slots into the terrain,
//! scene and vehicle shading. The profile itself stays all-off — the light is the frame's,
//! not the weather's.

use game_core::ImpactSurface;
use glam::Vec3;
use renderer_api::{LocalLight, MAX_LOCAL_LIGHTS, NO_LOCAL_LIGHTS};

use super::FxSystem;

/// How many pulses the pool holds before the weakest is dropped: a barrage degrades by losing
/// its dimmest spark, never by growing.
pub(crate) const MAX_LIGHT_PULSES: usize = 24;

/// One transient light: what it is at full energy, when it was born on the FX clock, and how
/// long it lives. Energy falls as `(1 - u)²` over the life — a flash is gone almost as soon as
/// it is seen, and the eye keeps the impression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LightPulse {
    pub light: LocalLight,
    pub born_s: f32,
    pub ttl_s: f32,
}

impl LightPulse {
    pub fn energy_at(&self, now_s: f32) -> f32 {
        let u = ((now_s - self.born_s) / self.ttl_s.max(1.0e-3)).clamp(0.0, 1.0);
        (1.0 - u) * (1.0 - u)
    }

    pub fn alive_at(&self, now_s: f32) -> bool {
        now_s - self.born_s < self.ttl_s
    }
}

impl FxSystem {
    /// The muzzle flash as light: an amber-white pool just ahead of the muzzle, its reach and
    /// energy by the round's recoil scale (S3), gone with the flash.
    pub fn muzzle_light(&mut self, muzzle: Vec3, direction: Vec3, recoil_scale: f32) {
        let position = muzzle + direction.normalize_or_zero() * 0.9;
        self.push_light(LocalLight::muzzle_flash(position.to_array(), recoil_scale), 0.09);
    }

    /// A shell absorbed by the world as light: a kinetic round's spark fan on steel or masonry,
    /// the dull spark of one into soil, an HE detonation's burst. Water swallows the light
    /// with the shell.
    pub fn impact_light(&mut self, position: Vec3, surface: ImpactSurface, high_explosive: bool) {
        let (strength, ttl_s) = if high_explosive {
            (2.0, 0.14)
        } else {
            match surface {
                ImpactSurface::Terrain => (0.3, 0.06),
                ImpactSurface::Cover | ImpactSurface::Hull => (1.0, 0.07),
                ImpactSurface::Water => return,
            }
        };
        self.push_light(LocalLight::impact_flash(position.to_array(), strength), ttl_s);
    }

    /// A strike on armour as light: the spark fan, brighter for a penetration (the flash-and-
    /// black-smoke signature has a flash in it), an HE charge's burst.
    pub fn armor_hit_light(&mut self, position: Vec3, penetrated: bool, high_explosive: bool) {
        let (strength, ttl_s) = if high_explosive {
            (2.0, 0.14)
        } else if penetrated {
            (1.4, 0.08)
        } else {
            (1.0, 0.07)
        };
        self.push_light(LocalLight::impact_flash(position.to_array(), strength), ttl_s);
    }

    fn push_light(&mut self, light: LocalLight, ttl_s: f32) {
        let now = self.stage_clock_s;
        self.lights.retain(|pulse| pulse.alive_at(now));
        if self.lights.len() >= MAX_LIGHT_PULSES {
            let weakest = self
                .lights
                .iter()
                .enumerate()
                .map(|(index, pulse)| (index, pulse.energy_at(now) * pulse.light.intensity))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index);
            if let Some(index) = weakest {
                self.lights.swap_remove(index);
            }
        }
        self.lights.push(LightPulse { light, born_s: now, ttl_s });
    }

    /// Drop the pulses that have burned out — called from the FX tick after the clock moved.
    pub(crate) fn tick_lights(&mut self) {
        let now = self.stage_clock_s;
        self.lights.retain(|pulse| pulse.alive_at(now));
    }

    /// This frame's six slots: the strongest live pulses, each at its current energy, strongest
    /// first; the all-off array when nothing is burning, byte-identical to the profile's own.
    pub fn local_lights(&self) -> [LocalLight; MAX_LOCAL_LIGHTS] {
        let now = self.stage_clock_s;
        let mut live: Vec<(f32, LocalLight)> = self
            .lights
            .iter()
            .filter(|pulse| pulse.alive_at(now))
            .map(|pulse| {
                let energy = pulse.energy_at(now);
                let light = pulse.light.at_energy(energy);
                (light.intensity * light.radius_m, light)
            })
            .collect();
        live.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut slots = NO_LOCAL_LIGHTS;
        for (slot, (_, light)) in slots.iter_mut().zip(live) {
            *slot = light;
        }
        slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_slots(fx: &FxSystem) -> usize {
        fx.local_lights().iter().filter(|light| light.radius_m > 0.0).count()
    }

    /// A shot lights the frame it flashes in, dims as the flash dies, and is gone with it —
    /// after which the slots are byte-identical to the profile's all-off array.
    #[test]
    fn a_shot_lights_its_frame_and_the_light_dies_with_the_flash() {
        let mut fx = FxSystem::default();
        fx.muzzle_light(Vec3::new(0.0, 1.8, 0.0), Vec3::Z, 1.0);
        let born = fx.local_lights()[0];
        assert!(born.radius_m > 0.0 && born.intensity > 0.0, "the shot lights: {born:?}");
        assert!(born.position[2] > 0.0, "the pool sits ahead of the muzzle: {:?}", born.position);
        fx.tick(0.04);
        let dying = fx.local_lights()[0];
        assert!(dying.intensity < born.intensity && dying.intensity > 0.0, "{dying:?}");
        assert_eq!(dying.radius_m, born.radius_m, "the pool shrinks in energy, not in reach");
        fx.tick(0.10);
        assert_eq!(fx.local_lights(), NO_LOCAL_LIGHTS, "the light is gone with the flash");
        assert!(fx.lights.is_empty(), "burned-out pulses are dropped");
    }

    /// A 128 mm lights further and harder than a 75 mm: the light scales through the same
    /// recoil momentum every other channel of the shot does (S3).
    #[test]
    fn a_heavier_round_lights_further_and_harder() {
        let mut light = FxSystem::default();
        light.muzzle_light(Vec3::ZERO, Vec3::Z, 0.67);
        let mut heavy = FxSystem::default();
        heavy.muzzle_light(Vec3::ZERO, Vec3::Z, 1.36);
        let (small, big) = (light.local_lights()[0], heavy.local_lights()[0]);
        assert!(big.radius_m > small.radius_m, "reach {} vs {}", big.radius_m, small.radius_m);
        assert!(big.intensity > small.intensity, "energy {} vs {}", big.intensity, small.intensity);
    }

    /// Eight sparks at once: the six strongest ride the slots, strongest first, and the two
    /// dimmest wait in the pool — nothing overflows the uniform.
    #[test]
    fn the_six_strongest_pulses_ride_the_slots_strongest_first() {
        let mut fx = FxSystem::default();
        for index in 0..8 {
            let high_explosive = index % 4 == 0;
            fx.impact_light(Vec3::X * index as f32, ImpactSurface::Cover, high_explosive);
        }
        assert_eq!(fx.lights.len(), 8);
        let slots = fx.local_lights();
        assert_eq!(live_slots(&fx), MAX_LOCAL_LIGHTS);
        for pair in slots.windows(2) {
            assert!(
                pair[0].intensity * pair[0].radius_m >= pair[1].intensity * pair[1].radius_m,
                "strongest first: {slots:?}"
            );
        }
        // Both HE bursts are among the six; the pool itself stays bounded under a barrage.
        assert!(slots[0].radius_m > slots[5].radius_m, "the HE bursts lead: {slots:?}");
        for index in 0..40 {
            fx.impact_light(Vec3::Z * index as f32, ImpactSurface::Hull, false);
        }
        assert!(fx.lights.len() <= MAX_LIGHT_PULSES, "the pool is bounded: {}", fx.lights.len());
    }

    /// The world's answers differ in light as they do in dust: an HE burst outreaches a spark
    /// fan, a spark fan outreaches the dull spark of a round into soil, and a shell dying in
    /// the river lights nothing.
    #[test]
    fn what_the_shell_died_against_sets_the_light() {
        let mut fx = FxSystem::default();
        fx.impact_light(Vec3::ZERO, ImpactSurface::Water, false);
        assert_eq!(live_slots(&fx), 0, "water swallows the light with the shell");
        let reach = |surface: ImpactSurface, high_explosive: bool| {
            let mut fx = FxSystem::default();
            fx.impact_light(Vec3::ZERO, surface, high_explosive);
            fx.local_lights()[0].radius_m
        };
        let soil = reach(ImpactSurface::Terrain, false);
        let steel = reach(ImpactSurface::Hull, false);
        let burst = reach(ImpactSurface::Terrain, true);
        assert!(burst > steel && steel > soil && soil > 0.0, "{burst} > {steel} > {soil}");
        let mut pen = FxSystem::default();
        pen.armor_hit_light(Vec3::ZERO, true, false);
        let mut bounce = FxSystem::default();
        bounce.armor_hit_light(Vec3::ZERO, false, false);
        assert!(
            pen.local_lights()[0].intensity > bounce.local_lights()[0].intensity,
            "a penetration flashes harder than a bounce"
        );
    }
}
