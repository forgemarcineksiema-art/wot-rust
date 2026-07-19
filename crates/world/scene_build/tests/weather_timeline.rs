use game_core::{MatchWeather, WeatherVariant};
use scene_build::weather_timeline::{WEATHER_DURATION_S, WeatherTimeline};
use terrain::MapId;

#[test]
fn every_dynamic_frame_is_bounded_continuous_and_fair() {
    for variant in
        [WeatherVariant::ClearAfternoon, WeatherVariant::RainSqualls, WeatherVariant::DawnFog]
    {
        for seed in [1, 7, 99, u64::MAX - 1] {
            let timeline =
                WeatherTimeline::new(MapId::BystraValley, MatchWeather::new(variant, seed));
            let mut previous = timeline.sample(0.0);
            for second in 0..=WEATHER_DURATION_S {
                let frame = timeline.sample(second as f32);
                for value in [frame.rain_intensity, frame.surface_wetness, frame.puddle_fill] {
                    assert!((0.0..=1.0).contains(&value), "{variant:?}/{seed}: {value}");
                }
                if second > 0 {
                    assert!((frame.rain_intensity - previous.rain_intensity).abs() < 0.06);
                    assert!((frame.surface_wetness - previous.surface_wetness).abs() < 0.03);
                    assert!((frame.puddle_fill - previous.puddle_fill).abs() < 0.02);
                }
                if second % 10 == 0 {
                    for height in (0..=40).step_by(2) {
                        let fog = frame.lighting.fog_factor(sim::VIEW_RANGE_M, height as f32);
                        assert!(fog <= 0.35, "{variant:?}/{seed} fog={fog} at {second}s/{height}m");
                    }
                }
                previous = frame;
            }
        }
    }
}

#[test]
fn late_joiner_reconstructs_the_same_weather_phase() {
    let weather = MatchWeather::new(WeatherVariant::RainSqualls, 0xDEAD_BEEF);
    let ongoing = WeatherTimeline::new(MapId::BystraValley, weather);
    let late_joiner = WeatherTimeline::new(MapId::BystraValley, weather);
    assert_eq!(ongoing.sample(327.4), late_joiner.sample(327.4));
}

#[test]
fn static_maps_and_practice_weather_do_not_gain_a_timeline() {
    let steppe = WeatherTimeline::new(
        MapId::ProkhorovkaHill252_2,
        MatchWeather::new(WeatherVariant::GoldenEvening, 123),
    );
    assert_eq!(steppe.sample(0.0), steppe.sample(600.0));

    let practice = WeatherTimeline::new(MapId::BystraValley, MatchWeather::default());
    assert_eq!(practice.sample(0.0), practice.sample(600.0));
}
