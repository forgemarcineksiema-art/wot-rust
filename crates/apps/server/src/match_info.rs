use game_core::WeatherVariant;
use terrain::MapId;

use crate::battle::BattleSeed;

/// Salt for the weather roll, so the pick decorrelates from every other seeded decision
/// (spawn jitter, bot vehicles, routes) made from the same battle seed.
const WEATHER_SALT: u64 = 0x57EA_7AB1_E000_0001;

/// The weather a map can be dressed in. Hand-tuned looks only — a variant appears here once
/// its full presentation (lighting, fog, particles, ambience) ships for that map, never
/// before. Weather is presentation-only by contract; the sim never reads it.
pub fn supported_weather(map: MapId) -> &'static [WeatherVariant] {
    match map {
        MapId::ProkhorovkaHill252_2 => &[WeatherVariant::ClearAfternoon],
    }
}

/// Roll the match weather from the battle seed: deterministic, so replaying a seed replays
/// the sky, and both ends of the wire agree without negotiation.
pub fn pick_weather(map: MapId, seed: BattleSeed) -> WeatherVariant {
    let options = supported_weather(map);
    options[seed.random_battle_index(WEATHER_SALT, options.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_supports_at_least_one_weather_variant() {
        for &map in MapId::ALL {
            assert!(
                !supported_weather(map).is_empty(),
                "map {map:?} has no weather to dress the battle in"
            );
        }
    }

    #[test]
    fn weather_pick_is_deterministic_per_seed() {
        for &map in MapId::ALL {
            let first = pick_weather(map, BattleSeed::fixed(42));
            assert_eq!(first, pick_weather(map, BattleSeed::fixed(42)));
            assert!(supported_weather(map).contains(&first));
        }
    }
}
