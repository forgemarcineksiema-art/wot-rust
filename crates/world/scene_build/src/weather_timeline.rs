//! Deterministic presentation weather evaluated from match identity and authoritative battle time.

use game_core::{MatchWeather, WeatherVariant, math::splitmix64};
use renderer_api::SceneLighting;
use terrain::MapId;

use crate::weather::weather_look;

mod program;

use program::{bystra_program, initial_puddle_fill, initial_wetness};

pub const WEATHER_DURATION_S: usize = 600;
const SUN_TRAVEL_RAD: f32 = 2.5_f32.to_radians();

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherFrame {
    pub lighting: SceneLighting,
    pub sky: (f64, f64, f64),
    pub rain_intensity: f32,
    pub surface_wetness: f32,
    pub puddle_fill: f32,
    pub cloud_offset: [f32; 2],
    pub rain_phase_s: f32,
}

impl WeatherFrame {
    fn lerp(self, other: Self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self {
            lighting: self.lighting.lerp(other.lighting, t),
            sky: (
                weather_lerp64(self.sky.0, other.sky.0, t),
                weather_lerp64(self.sky.1, other.sky.1, t),
                weather_lerp64(self.sky.2, other.sky.2, t),
            ),
            rain_intensity: weather_lerp(self.rain_intensity, other.rain_intensity, t),
            surface_wetness: weather_lerp(self.surface_wetness, other.surface_wetness, t),
            puddle_fill: weather_lerp(self.puddle_fill, other.puddle_fill, t),
            cloud_offset: [
                weather_lerp(self.cloud_offset[0], other.cloud_offset[0], t),
                weather_lerp(self.cloud_offset[1], other.cloud_offset[1], t),
            ],
            rain_phase_s: weather_lerp(self.rain_phase_s, other.rain_phase_s, t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WeatherTimeline {
    frames: Vec<WeatherFrame>,
}

impl WeatherTimeline {
    pub fn new(map: MapId, weather: MatchWeather) -> Self {
        let static_program = map != MapId::BystraValley || weather.seed == 0;
        let cloud_offset = if static_program {
            [0.0; 2]
        } else {
            [
                timeline_seed_range(weather.seed, 11, -32.0, 32.0),
                timeline_seed_range(weather.seed, 12, -32.0, 32.0),
            ]
        };
        let rain_phase_s =
            if static_program { 0.0 } else { timeline_seed_range(weather.seed, 13, 0.0, 1.75) };
        let mut wetness = initial_wetness(weather.variant, static_program);
        let mut puddle_fill = initial_puddle_fill(weather.variant, static_program);
        let mut frames = Vec::with_capacity(WEATHER_DURATION_S + 1);

        for second in 0..=WEATHER_DURATION_S {
            let time_s = second as f32;
            let (look, rain) = if static_program {
                let look = weather_look(map, weather.variant);
                (look, look.rain_intensity)
            } else {
                bystra_program(weather, time_s)
            };
            if second > 0 && !static_program {
                wetness = response(wetness, rain, if rain > wetness { 45.0 } else { 360.0 });
                let puddle_target = weather_smoothstep(0.45, 0.9, wetness);
                puddle_fill = response(
                    puddle_fill,
                    puddle_target,
                    if puddle_target > puddle_fill { 120.0 } else { 1200.0 },
                );
            }
            let mut lighting = look.lighting;
            if !static_program {
                let anchor = match weather.variant {
                    WeatherVariant::DawnFog => SceneLighting::bystra_dawn_fog().key_direction,
                    _ => SceneLighting::bystra_clear_afternoon().key_direction,
                };
                lighting.key_direction = rotated_sun(anchor, time_s);
            }
            frames.push(WeatherFrame {
                lighting,
                sky: look.sky,
                rain_intensity: rain.clamp(0.0, 1.0),
                surface_wetness: wetness.clamp(0.0, 1.0),
                puddle_fill: puddle_fill.clamp(0.0, 1.0),
                cloud_offset,
                rain_phase_s,
            });
        }
        Self { frames }
    }

    pub fn sample(&self, elapsed_s: f32) -> WeatherFrame {
        let time = elapsed_s.clamp(0.0, WEATHER_DURATION_S as f32);
        let lo = time.floor() as usize;
        let hi = (lo + 1).min(WEATHER_DURATION_S);
        self.frames[lo].lerp(self.frames[hi], time - lo as f32)
    }
}

fn rotated_sun(direction: [f32; 3], time_s: f32) -> [f32; 3] {
    let angle = SUN_TRAVEL_RAD * (time_s / WEATHER_DURATION_S as f32).clamp(0.0, 1.0);
    let (sin, cos) = angle.sin_cos();
    [
        direction[0] * cos + direction[2] * sin,
        direction[1],
        -direction[0] * sin + direction[2] * cos,
    ]
}

fn response(value: f32, target: f32, time_constant_s: f32) -> f32 {
    target + (value - target) * (-1.0 / time_constant_s).exp()
}

fn weather_smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn timeline_seed_range(seed: u64, salt: u64, low: f32, high: f32) -> f32 {
    let bits = splitmix64(seed.wrapping_add(salt)) >> 40;
    weather_lerp(low, high, bits as f32 / ((1_u64 << 24) - 1) as f32)
}

fn weather_lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn weather_lerp64(a: f64, b: f64, t: f32) -> f64 {
    a + (b - a) * f64::from(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_is_deterministic_seeded_and_clamped() {
        let weather = MatchWeather::new(WeatherVariant::RainSqualls, 77);
        let a = WeatherTimeline::new(MapId::BystraValley, weather);
        let b = WeatherTimeline::new(MapId::BystraValley, weather);
        assert_eq!(a.sample(273.25), b.sample(273.25));
        assert_ne!(
            a.sample(0.0).cloud_offset,
            WeatherTimeline::new(
                MapId::BystraValley,
                MatchWeather::new(WeatherVariant::RainSqualls, 78),
            )
            .sample(0.0)
            .cloud_offset,
        );
        assert_eq!(a.sample(600.0), a.sample(999.0));
    }

    #[test]
    fn programs_keep_their_authored_weather_story() {
        let clear = WeatherTimeline::new(
            MapId::BystraValley,
            MatchWeather::new(WeatherVariant::ClearAfternoon, 11),
        );
        assert!((0..=600).all(|s| clear.sample(s as f32).rain_intensity == 0.0));

        let squall = WeatherTimeline::new(
            MapId::BystraValley,
            MatchWeather::new(WeatherVariant::RainSqualls, 22),
        );
        assert_eq!(squall.sample(0.0).rain_intensity, 0.0);
        assert!((0..=600).any(|s| squall.sample(s as f32).rain_intensity > 0.8));
        assert!(squall.sample(450.0).puddle_fill > squall.sample(100.0).puddle_fill);

        let dawn = WeatherTimeline::new(
            MapId::BystraValley,
            MatchWeather::new(WeatherVariant::DawnFog, 33),
        );
        assert!(dawn.sample(500.0).lighting.fog_density < dawn.sample(0.0).lighting.fog_density);
        assert!((0..=600).all(|s| dawn.sample(s as f32).rain_intensity == 0.0));
    }

    #[test]
    fn sun_moves_only_the_authored_two_and_a_half_degrees() {
        let timeline = WeatherTimeline::new(
            MapId::BystraValley,
            MatchWeather::new(WeatherVariant::ClearAfternoon, 44),
        );
        let a = timeline.sample(0.0).lighting.key_direction;
        let b = timeline.sample(600.0).lighting.key_direction;
        let azimuth = |direction: [f32; 3]| direction[2].atan2(direction[0]);
        assert!(((azimuth(b) - azimuth(a)).abs().to_degrees() - 2.5).abs() < 0.02);
    }
}
