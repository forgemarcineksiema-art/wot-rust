//! The weather look table: what a `(map, weather variant)` pair means for the client's eyes
//! and ears. The SERVER picks the variant (deterministically from the battle seed) and the
//! sim never reads it — weather is presentation only, and the fog-fairness test below is the
//! contract that keeps it that way: no variant may visually erase a legitimately spotted
//! target at the 400 m view range.

use renderer_api::SceneLighting;
use terrain::MapId;

use game_core::WeatherVariant;

#[derive(Debug, Clone, Copy)]
pub struct WeatherLook {
    pub lighting: SceneLighting,
    /// The renderer's fallback clear colour behind the gradient sky (r, g, b).
    pub sky: (f64, f64, f64),
    /// Rain streak density, 0.0 disables the rain pass entirely.
    pub rain_intensity: f32,
    /// World wetness 0..1: darkens albedo, sharpens finishes, pools sheen on flat ground.
    pub wetness: f32,
}

/// Total over every (map, variant) pair: an unauthored combination falls back to the map's
/// clear look instead of panicking — the server-side `supported_weather` table is what keeps
/// unauthored variants out of real battles.
pub fn weather_look(map: MapId, variant: WeatherVariant) -> WeatherLook {
    match (map, variant) {
        (MapId::ProkhorovkaHill252_2, WeatherVariant::GoldenEvening) => WeatherLook {
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
            rain_intensity: 0.0,
            wetness: 0.0,
        },
        (MapId::ProkhorovkaHill252_2, WeatherVariant::Overcast) => WeatherLook {
            lighting: SceneLighting::prokhorovka_overcast(),
            sky: (0.48, 0.51, 0.55),
            rain_intensity: 0.0,
            wetness: 0.0,
        },
        // The hazy-noon default, and the fallback for variants the steppe does not author.
        (MapId::ProkhorovkaHill252_2, _) => WeatherLook {
            lighting: SceneLighting::battlefield_default(),
            sky: (0.55, 0.69, 0.87),
            rain_intensity: 0.0,
            wetness: 0.0,
        },
        (MapId::BystraValley, WeatherVariant::RainSqualls) => WeatherLook {
            lighting: SceneLighting::bystra_rain(),
            sky: (0.42, 0.46, 0.50),
            rain_intensity: 1.0,
            wetness: 1.0,
        },
        (MapId::BystraValley, WeatherVariant::DawnFog) => WeatherLook {
            lighting: SceneLighting::bystra_dawn_fog(),
            sky: (0.66, 0.64, 0.64),
            rain_intensity: 0.0,
            wetness: 0.2,
        },
        // The clear afternoon, and the fallback for variants the valley does not author
        // (the server-side supported_weather table keeps those out of real battles).
        (MapId::BystraValley, _) => WeatherLook {
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: (0.62, 0.66, 0.72),
            rain_intensity: 0.0,
            wetness: 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE fairness lock of the weather system: in every authored look, a target at the sim's
    /// 400 m view range keeps at least 65% of its contrast at every fighting height — weather
    /// is presentation, never a gameplay modifier. (`uczciwy czołg`: nobody fights the sky.)
    const MAX_FOG_AT_VIEW_RANGE: f32 = 0.35;

    #[test]
    fn no_weather_look_can_hide_a_spotted_target_at_view_range() {
        for &map in terrain::MapId::ALL {
            for variant in game_core::WeatherVariant::ALL {
                let look = weather_look(map, variant);
                let mut height = 0.0_f32;
                while height <= 40.0 {
                    let fog = look.lighting.fog_factor(sim::VIEW_RANGE_M, height);
                    assert!(
                        fog <= MAX_FOG_AT_VIEW_RANGE,
                        "{map:?}/{variant:?} fogs {fog} at {}m/{height}m — hides spotted targets",
                        sim::VIEW_RANGE_M
                    );
                    height += 2.0;
                }
            }
        }
    }

    /// The table is total and each authored Bystra variant is a genuinely different sky.
    #[test]
    fn every_pair_has_a_look_and_bystra_variants_differ() {
        let clear = weather_look(MapId::BystraValley, WeatherVariant::ClearAfternoon);
        let rain = weather_look(MapId::BystraValley, WeatherVariant::RainSqualls);
        let dawn = weather_look(MapId::BystraValley, WeatherVariant::DawnFog);
        assert!(rain.rain_intensity > 0.0 && clear.rain_intensity == 0.0);
        // Rain soaks the world; dawn leaves only river-mist damp; a clear afternoon is dry.
        assert!(rain.wetness == 1.0 && clear.wetness == 0.0);
        assert!(dawn.wetness > 0.0 && dawn.wetness < rain.wetness);
        assert_ne!(clear.lighting.key_rgb, rain.lighting.key_rgb);
        assert_ne!(clear.lighting.fog_density, dawn.lighting.fog_density);
        assert!(
            dawn.lighting.fog_height_falloff > clear.lighting.fog_height_falloff,
            "dawn mist fills the valley floor"
        );
    }
}
