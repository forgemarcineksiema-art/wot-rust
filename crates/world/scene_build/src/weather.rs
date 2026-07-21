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

/// Total over every (map, variant) pair, straight from the map's BLUEPRINT (`environment`
/// section): an unauthored variant falls back to the map's first (default) look instead of
/// panicking — the server-side `supported_weather` (which reads the same list) is what keeps
/// unauthored variants out of real battles. A map without an environment section wears the
/// hazy-noon default.
pub fn weather_look(map: MapId, variant: WeatherVariant) -> WeatherLook {
    let Some(environment) = &map_forge::cached_blueprint(map).environment else {
        return hazy_noon_fallback();
    };
    match environment
        .looks
        .iter()
        .find(|look| look.variant == variant)
        .or_else(|| environment.looks.first())
    {
        Some(look) => realize_look(look),
        None => hazy_noon_fallback(),
    }
}

/// The look a map without an authored environment wears (and the guard for an empty list,
/// which the map report refuses on shipped maps anyway).
fn hazy_noon_fallback() -> WeatherLook {
    WeatherLook {
        lighting: SceneLighting::battlefield_default(),
        sky: (0.55, 0.69, 0.87),
        rain_intensity: 0.0,
        wetness: 0.0,
    }
}

/// Bind one authored look to the renderer: named preset → `SceneLighting`, then the sparse
/// overrides — the blueprint stays renderer-free, THIS is where names get their meaning.
fn realize_look(look: &map_forge::blueprint::LookSpec) -> WeatherLook {
    let mut lighting = preset_lighting(look.preset);
    let overrides = &look.overrides;
    if let Some(value) = overrides.fog_density {
        lighting.fog_density = value;
    }
    if let Some(value) = overrides.exposure {
        lighting.exposure = value;
    }
    if let Some(value) = overrides.cloud_coverage_bias {
        lighting.cloud_coverage_bias = value;
    }
    if let Some(value) = overrides.cloud_opacity {
        lighting.cloud_opacity = value;
    }
    if let Some(value) = overrides.saturation {
        lighting.saturation = value;
    }
    WeatherLook {
        lighting,
        sky: (look.sky_rgb[0], look.sky_rgb[1], look.sky_rgb[2]),
        rain_intensity: look.rain_intensity,
        wetness: look.wetness,
    }
}

/// What each preset NAME means: the hand-tuned `SceneLighting` profiles stay code (they are
/// the vocabulary, like terrain ops); blueprints only pick them by name.
fn preset_lighting(preset: map_forge::blueprint::LightingPreset) -> SceneLighting {
    use map_forge::blueprint::LightingPreset;
    match preset {
        LightingPreset::HazyNoon => SceneLighting::battlefield_default(),
        LightingPreset::ClearAfternoon => SceneLighting::bystra_clear_afternoon(),
        LightingPreset::GoldenEvening => SceneLighting::prokhorovka_golden_evening(),
        LightingPreset::LeadOvercast => SceneLighting::prokhorovka_overcast(),
        LightingPreset::RainSqualls => SceneLighting::bystra_rain(),
        LightingPreset::DawnFog => SceneLighting::bystra_dawn_fog(),
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

    /// The migration lock: the blueprint-authored looks reproduce the hand-tuned table
    /// exactly — preset + sky + rain + wetness per authored (map, variant) pair, and an
    /// unauthored variant falls back to the map's default look, as it always did.
    #[test]
    fn blueprint_looks_reproduce_the_hand_tuned_table() {
        let steppe = MapId::ProkhorovkaHill252_2;
        assert_eq!(
            weather_look(steppe, WeatherVariant::ClearAfternoon).lighting,
            SceneLighting::battlefield_default()
        );
        assert_eq!(
            weather_look(steppe, WeatherVariant::GoldenEvening).lighting,
            SceneLighting::prokhorovka_golden_evening()
        );
        assert_eq!(
            weather_look(steppe, WeatherVariant::Overcast).lighting,
            SceneLighting::prokhorovka_overcast()
        );
        assert_eq!(weather_look(steppe, WeatherVariant::GoldenEvening).sky, (0.80, 0.62, 0.45));

        let valley = MapId::BystraValley;
        assert_eq!(
            weather_look(valley, WeatherVariant::ClearAfternoon).lighting,
            SceneLighting::bystra_clear_afternoon()
        );
        assert_eq!(
            weather_look(valley, WeatherVariant::RainSqualls).lighting,
            SceneLighting::bystra_rain()
        );
        assert_eq!(
            weather_look(valley, WeatherVariant::DawnFog).lighting,
            SceneLighting::bystra_dawn_fog()
        );
        // Unauthored variants fall back to the map's FIRST (default) look.
        assert_eq!(
            weather_look(valley, WeatherVariant::GoldenEvening).lighting,
            SceneLighting::bystra_clear_afternoon()
        );
        assert_eq!(
            weather_look(steppe, WeatherVariant::RainSqualls).lighting,
            SceneLighting::battlefield_default()
        );
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
