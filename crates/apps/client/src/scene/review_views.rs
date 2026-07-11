//! The canonical look-review views: the exact camera + lighting setups the art-direction
//! policy is judged against (`docs/art-direction-policy.md`). One source of truth shared by
//! the `prokhorovka_views` example (human eyes) and the `look_goldens` test harness (locked
//! regression), so "the picture we reviewed" and "the picture we locked" can never drift apart.

use renderer_api::SceneLighting;
use terrain::BattlefieldMap;

/// One canonical review view: a named camera on a battlefield under one authored lighting
/// profile, with the renderer's fallback clear colour behind the gradient sky.
pub struct ReviewView {
    pub name: &'static str,
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub lighting: SceneLighting,
    pub sky: (f64, f64, f64),
}

/// The Prokhorovka review set: the hill panorama in its three authored times of day plus a
/// mid-field vantage in the golden evening — the strongest frame the engine produces and the
/// reference look of the whole art direction ("steel under an evening sky").
pub fn prokhorovka_review_views(battlefield: &BattlefieldMap) -> [ReviewView; 4] {
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(5.0);
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];
    [
        ReviewView {
            name: "prokhorovka_noon",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::battlefield_default(),
            sky: (0.55, 0.69, 0.87),
        },
        ReviewView {
            name: "prokhorovka_golden_evening",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
        },
        ReviewView {
            name: "prokhorovka_overcast",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::prokhorovka_overcast(),
            sky: (0.48, 0.51, 0.55),
        },
        ReviewView {
            name: "prokhorovka_evening_midfield",
            eye: at(500.0, 6.0, 460.0),
            target: at(620.0, 2.0, 520.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
        },
    ]
}
