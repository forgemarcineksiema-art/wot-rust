use game_core::{MatchWeather, WeatherVariant};
use terrain::MapId;

use crate::battle::BattleSeed;

/// Salt for the weather roll, so the pick decorrelates from every other seeded decision
/// (spawn jitter, bot vehicles, routes) made from the same battle seed.
const WEATHER_SALT: u64 = 0x57EA_7AB1_E000_0001;
const WEATHER_PROGRAM_SALT: u64 = 0x57EA_7AB1_E000_0002;

/// The weather a map can be dressed in, straight from its BLUEPRINT (`environment` looks,
/// in authored order — the order feeds the seeded roll). A variant appears there once its
/// full presentation (lighting, fog, particles, ambience) ships for that map, never before.
/// Weather is presentation-only by contract; the sim never reads it. A map without an
/// authored environment plays its single hazy-noon default.
pub fn supported_weather(map: MapId) -> Vec<WeatherVariant> {
    match &map_forge::cached_blueprint(map).environment {
        Some(environment) => environment.looks.iter().map(|look| look.variant).collect(),
        None => vec![WeatherVariant::ClearAfternoon],
    }
}

/// Roll the match weather from the battle seed: deterministic, so replaying a seed replays
/// the sky, and both ends of the wire agree without negotiation.
pub fn pick_weather(map: MapId, seed: BattleSeed) -> MatchWeather {
    let options = supported_weather(map);
    MatchWeather::new(
        options[seed.random_battle_index(WEATHER_SALT, options.len())],
        seed.random_battle_u64(WEATHER_PROGRAM_SALT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_supports_at_least_one_weather_variant() {
        for &map in MapId::SHIPPED {
            assert!(
                !supported_weather(map).is_empty(),
                "map {map:?} has no weather to dress the battle in"
            );
        }
    }

    /// The migration lock: the blueprint-authored rotations reproduce the hand-tuned table
    /// IN ORDER — the list order feeds the seeded roll, so reordering a blueprint's looks
    /// would silently reshuffle which sky every replayed seed gets.
    #[test]
    fn the_blueprint_rotation_reproduces_the_hand_tuned_table() {
        assert_eq!(
            supported_weather(MapId::ProkhorovkaHill252_2),
            vec![
                WeatherVariant::ClearAfternoon,
                WeatherVariant::GoldenEvening,
                WeatherVariant::Overcast
            ]
        );
        assert_eq!(
            supported_weather(MapId::BystraValley),
            vec![
                WeatherVariant::ClearAfternoon,
                WeatherVariant::RainSqualls,
                WeatherVariant::DawnFog
            ]
        );
    }

    #[test]
    fn weather_pick_is_deterministic_per_seed() {
        for &map in MapId::SHIPPED {
            let first = pick_weather(map, BattleSeed::fixed(42));
            assert_eq!(first, pick_weather(map, BattleSeed::fixed(42)));
            assert!(supported_weather(map).contains(&first.variant));
            assert_ne!(first.seed, 0);
        }
        assert_ne!(
            pick_weather(MapId::BystraValley, BattleSeed::fixed(42)).seed,
            pick_weather(MapId::BystraValley, BattleSeed::fixed(43)).seed,
        );
    }
}
