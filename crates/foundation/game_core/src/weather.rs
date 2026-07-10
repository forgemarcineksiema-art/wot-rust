use serde::{Deserialize, Serialize};

/// Per-match weather identity, picked once by the server when a battle is created and told to
/// the client alongside the map. Weather is **presentation only** — lighting, fog, particles,
/// ambience. The simulation never reads it: spotting ranges, ballistics and traction are
/// identical in every variant, so no team ever fights the sky.
///
/// Like [`crate::VehicleKind`], the variant order is wire identity (bincode discriminants) —
/// append, never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WeatherVariant {
    #[default]
    ClearAfternoon,
    RainSqualls,
    DawnFog,
    /// A low western sun raking long shadows across the field — the golden-hour look.
    GoldenEvening,
    /// A dry lead-grey lid: flat light, no sun disc worth the name, no rain.
    Overcast,
}

impl WeatherVariant {
    pub const ALL: [WeatherVariant; 5] = [
        WeatherVariant::ClearAfternoon,
        WeatherVariant::RainSqualls,
        WeatherVariant::DawnFog,
        WeatherVariant::GoldenEvening,
        WeatherVariant::Overcast,
    ];
}
