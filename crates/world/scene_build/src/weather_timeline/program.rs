use game_core::{MatchWeather, WeatherVariant, math::splitmix64};
use terrain::MapId;

use crate::weather::{WeatherLook, weather_look};

pub(super) fn bystra_program(weather: MatchWeather, time_s: f32) -> (WeatherLook, f32) {
    let clear = weather_look(MapId::BystraValley, WeatherVariant::ClearAfternoon);
    let rain = weather_look(MapId::BystraValley, WeatherVariant::RainSqualls);
    let dawn = weather_look(MapId::BystraValley, WeatherVariant::DawnFog);
    match weather.variant {
        WeatherVariant::ClearAfternoon => {
            let onset = program_seed_range(weather.seed, 21, 90.0, 180.0);
            let amount = phase_envelope(time_s, onset, 90.0, 120.0, 120.0)
                * program_seed_range(weather.seed, 22, 0.45, 0.65);
            (blend_look(clear, rain, amount * 0.55, 0.0), 0.0)
        }
        WeatherVariant::RainSqualls => {
            let onset = program_seed_range(weather.seed, 31, 90.0, 150.0);
            let hold = program_seed_range(weather.seed, 32, 150.0, 210.0);
            let rain_amount = phase_envelope(time_s, onset, 60.0, hold, 90.0)
                * program_seed_range(weather.seed, 33, 0.85, 1.0);
            let storm = phase_envelope(time_s, onset - 60.0, 120.0, hold + 30.0, 120.0);
            (blend_look(clear, rain, storm, rain_amount), rain_amount)
        }
        WeatherVariant::DawnFog => {
            let onset = program_seed_range(weather.seed, 41, 120.0, 180.0);
            let duration = program_seed_range(weather.seed, 42, 240.0, 300.0);
            let lift = program_smoothstep(onset, onset + duration, time_s);
            (blend_look(dawn, clear, lift * 0.45, 0.0), 0.0)
        }
        _ => (weather_look(MapId::BystraValley, weather.variant), 0.0),
    }
}

pub(super) fn initial_wetness(variant: WeatherVariant, static_program: bool) -> f32 {
    if static_program {
        match variant {
            WeatherVariant::RainSqualls => 1.0,
            WeatherVariant::DawnFog => 0.2,
            _ => 0.0,
        }
    } else if variant == WeatherVariant::DawnFog {
        0.2
    } else if variant == WeatherVariant::RainSqualls {
        0.03
    } else {
        0.0
    }
}

pub(super) fn initial_puddle_fill(variant: WeatherVariant, static_program: bool) -> f32 {
    if static_program && variant == WeatherVariant::RainSqualls {
        1.0
    } else if variant == WeatherVariant::DawnFog {
        0.08
    } else {
        0.0
    }
}

fn blend_look(from: WeatherLook, to: WeatherLook, amount: f32, rain: f32) -> WeatherLook {
    let t = amount.clamp(0.0, 1.0);
    WeatherLook {
        lighting: from.lighting.lerp(to.lighting, t),
        sky: (
            program_lerp64(from.sky.0, to.sky.0, t),
            program_lerp64(from.sky.1, to.sky.1, t),
            program_lerp64(from.sky.2, to.sky.2, t),
        ),
        rain_intensity: rain,
        wetness: 0.0,
    }
}

fn phase_envelope(time: f32, onset: f32, rise: f32, hold: f32, fall: f32) -> f32 {
    if time < onset {
        0.0
    } else if time < onset + rise {
        program_smoothstep(onset, onset + rise, time)
    } else if time < onset + rise + hold {
        1.0
    } else {
        1.0 - program_smoothstep(onset + rise + hold, onset + rise + hold + fall, time)
    }
}

fn program_smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn program_seed_range(seed: u64, salt: u64, low: f32, high: f32) -> f32 {
    let bits = splitmix64(seed.wrapping_add(salt)) >> 40;
    low + (high - low) * bits as f32 / ((1_u64 << 24) - 1) as f32
}

fn program_lerp64(a: f64, b: f64, t: f32) -> f64 {
    a + (b - a) * f64::from(t)
}
