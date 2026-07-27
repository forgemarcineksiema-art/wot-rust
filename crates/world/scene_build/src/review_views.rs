//! The canonical look-review views: the exact camera + lighting setups the art-direction
//! policy is judged against (`docs/art-direction-policy.md`). One source of truth shared by the
//! `*_views` examples (human eyes) and the `look_goldens` harness (locked regression) — both
//! render them through `client::render_review_views`, so "the picture we reviewed" and "the
//! picture we locked" cannot drift apart.
//!
//! The look views are **derived from each map's blueprint**, not hand-listed. `pick_weather`
//! rolls a variant at random for every battle, so a look this set skips is a look the player
//! meets unreviewed — and a hand-written table is exactly the thing that skips one. Deriving
//! them also picks up the blueprint's per-map `LightingOverrides`, which a hardcoded
//! `SceneLighting::…()` call would silently miss: locking a profile the game never ships is the
//! sin this whole program exists to correct.

use game_core::{VehicleKind, WeatherVariant};
use renderer_api::SceneLighting;
use terrain::{BattlefieldMap, MapId};

/// The vehicle a review view parks in frame, described in the terms a *picture* needs rather
/// than the terms the wire needs. The client turns this into a `net::TankSnapshot` — keeping the
/// protocol crate out of the world layer, and keeping the review set honest about what it is: a
/// statement of what the camera should see.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReviewVehicle {
    pub kind: VehicleKind,
    /// Hull origin, already sitting on the ground.
    pub position: [f32; 3],
    pub yaw_rad: f32,
    pub turret_yaw_rad: f32,
    /// Paint. The battle tints friend and foe apart, so a review frame must state which it is.
    pub hull_color: [f32; 3],
}

/// One canonical review view: a named camera on a battlefield under one authored lighting
/// profile, with the renderer's fallback clear colour behind the gradient sky, and optionally a
/// vehicle parked in shot.
pub struct ReviewView {
    /// Golden filename and log identity. Owned, because the look views are derived from each
    /// blueprint's declared variants rather than hand-listed.
    pub name: String,
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub lighting: SceneLighting,
    pub sky: (f64, f64, f64),
    /// The subject, when this view has one. A landscape-only review set cannot catch a hero that
    /// fails to separate from its ground, and the subject of this game is a tank.
    pub vehicle: Option<ReviewVehicle>,
    /// Normalized `[x0, y0, x1, y1]` crop framing the subject, when the view exists to judge it.
    /// The frame-wide value statistics cannot see a vehicle crushed to a black silhouette — a
    /// tank is a small share of a wide frame, so a picture can lose its whole subject and still
    /// read as three healthy value planes. Measuring INSIDE this box is what makes "you cannot
    /// see half the tank" a failing test instead of a remark on a screenshot.
    pub subject_box: Option<[f32; 4]>,
}

/// The player's own eye, derived from the battle chase camera rather than guessed: 12 m back at
/// 0.42 rad (`client::camera::types` defaults) puts the eye ~4.9 m above the hull it follows.
/// The review panoramas used to sit at 14 m — a vantage the game never gives anyone, which is
/// how a picture could pass review and still look wrong in play.
pub const CHASE_EYE_HEIGHT_M: f32 = 4.9;
const CHASE_DISTANCE_M: f32 = 12.0;

/// One canonical review of the GARAGE. The hangar is an interior studio, not a battlefield: no
/// sky dome, no fog, its own light rig and its own long lens — so it needs its own view type
/// rather than a battlefield view with the outdoor half left empty.
///
/// It belongs under the same locks all the same. The garage is the first thirty seconds of
/// contact with the game, it grades through the same display transform (policy rule 7), and it
/// had **no golden and no review view at all** before this.
pub struct HangarReviewView {
    pub name: String,
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub lighting: SceneLighting,
    /// The flat interior clear colour that stands in for a sky the hangar does not have.
    pub background: (f64, f64, f64),
    /// The hero on the turntable. A garage review with no vehicle reviews an empty room.
    pub vehicle: ReviewVehicle,
}

/// The garage review set: the hero shot the garage actually opens with. The framing comes from
/// `hangar::HERO_ORBIT_*` — the same constants the live orbit camera rests at — so a reframing
/// moves the played picture and the locked picture together.
pub fn hangar_review_views() -> Vec<HangarReviewView> {
    vec![HangarReviewView {
        name: "garage_hero".to_string(),
        eye: crate::hangar::hero_orbit_eye().to_array(),
        target: crate::hangar::hangar_camera_pivot().to_array(),
        lighting: SceneLighting::garage_hero(),
        // Matches `garage_render::ensure_scene`'s interior background.
        background: (0.05, 0.05, 0.06),
        vehicle: ReviewVehicle {
            kind: VehicleKind::T54_1951,
            position: [0.0, crate::hangar::TURNTABLE_TOP_M, 0.0],
            // Three-quarter to the camera, as `garage_preview_snapshot` parks it.
            yaw_rad: 0.6,
            turret_yaw_rad: 0.0,
            // The garage's own showroom tint, not the battle green.
            hull_color: [0.72, 0.76, 0.62],
        },
    }]
}

/// The maps whose looks are locked. A map missing from here ships unreviewed, so the coverage
/// test reads this list rather than trusting anyone to remember.
pub const REVIEWED_MAPS: [MapId; 4] =
    [MapId::ProkhorovkaHill252_2, MapId::BystraValley, MapId::OrlinyPereval, MapId::Ostrogorsk];

/// Every review view for one map: its shared look vantage under **every** variant the blueprint
/// declares, followed by the identity views that say what the map IS.
pub fn review_views_for(map: MapId, battlefield: &BattlefieldMap) -> Vec<ReviewView> {
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(5.0);
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];

    let (look_eye, look_target) = look_vantage(map, &at);
    let mut views: Vec<ReviewView> = declared_looks(map)
        .into_iter()
        .map(|variant| {
            let look = crate::weather::weather_look(map, variant);
            ReviewView {
                name: format!("{}_{}", map_key(map), variant_key(variant)),
                eye: look_eye,
                target: look_target,
                lighting: look.lighting,
                sky: look.sky,
                vehicle: None,
                subject_box: None,
            }
        })
        .collect();
    views.extend(identity_views(map, battlefield, &at));
    views
}

/// The variants a map's blueprint declares, in authored order — the same list the server's
/// `supported_weather` rolls over. A map without an environment section plays hazy noon.
fn declared_looks(map: MapId) -> Vec<WeatherVariant> {
    match &map_forge::cached_blueprint(map).environment {
        Some(environment) => environment.looks.iter().map(|look| look.variant).collect(),
        None => vec![WeatherVariant::ClearAfternoon],
    }
}

fn map_key(map: MapId) -> &'static str {
    match map {
        MapId::ProkhorovkaHill252_2 => "prokhorovka",
        MapId::BystraValley => "bystra",
        MapId::OrlinyPereval => "orliny",
        MapId::Ostrogorsk => "ostrogorsk",
        _ => "scratch",
    }
}

fn variant_key(variant: WeatherVariant) -> &'static str {
    match variant {
        WeatherVariant::ClearAfternoon => "clear_afternoon",
        WeatherVariant::RainSqualls => "rain",
        WeatherVariant::DawnFog => "dawn_fog",
        WeatherVariant::GoldenEvening => "golden_evening",
        WeatherVariant::Overcast => "overcast",
    }
}

/// The one vantage each map shows all of its looks from — chosen so a single frame carries the
/// map's shape, its ground, and enough sky to judge the air. All at the player's own eye.
fn look_vantage(map: MapId, at: &dyn Fn(f32, f32, f32) -> [f32; 3]) -> ([f32; 3], [f32; 3]) {
    match map {
        // Off the road crown deliberately: on the axis at eye height a third of the frame is
        // embankment. A review vantage is an art-direction decision, not a convenience.
        MapId::ProkhorovkaHill252_2 => {
            (at(250.0, CHASE_EYE_HEIGHT_M, 452.0), at(700.0, 2.0, 505.0))
        }
        // The bridge: valley, town bench and water in one frame.
        MapId::BystraValley => (at(486.0, CHASE_EYE_HEIGHT_M, 432.0), at(610.0, 2.0, 500.0)),
        // Straight up the defile — the pass IS the map.
        MapId::OrlinyPereval => (at(500.0, 4.0, 220.0), at(500.0, 8.0, 500.0)),
        // The boulevard runs the length of the map into the city.
        MapId::Ostrogorsk => (at(460.0, 3.0, 230.0), at(460.0, 2.5, 500.0)),
        _ => (at(500.0, CHASE_EYE_HEIGHT_M, 250.0), at(500.0, 2.0, 500.0)),
    }
}

/// The frames that say what a map is, rendered under its first declared look.
fn identity_views(
    map: MapId,
    battlefield: &BattlefieldMap,
    at: &dyn Fn(f32, f32, f32) -> [f32; 3],
) -> Vec<ReviewView> {
    let default_look = crate::weather::weather_look(
        map,
        declared_looks(map).first().copied().unwrap_or(WeatherVariant::ClearAfternoon),
    );
    let named = |name: &str, eye: [f32; 3], target: [f32; 3]| ReviewView {
        name: format!("{}_{name}", map_key(map)),
        eye,
        target,
        lighting: default_look.lighting,
        sky: default_look.sky,
        vehicle: None,
        subject_box: None,
    };

    match map {
        MapId::ProkhorovkaHill252_2 => prokhorovka_identity_views(battlefield, at),
        MapId::BystraValley => {
            vec![named("town_lane", at(680.0, CHASE_EYE_HEIGHT_M, 470.0), at(760.0, 3.0, 510.0))]
        }
        // The pine belt, not the crest walk: the crest vantage points at the near side of a
        // grass mound — 60% bare hillside, 40% sky, nothing to judge. A review frame has to show
        // the map, and for a mountain pass that means the belt climbing toward the ridge.
        MapId::OrlinyPereval => {
            vec![named("pine_belt", at(430.0, 3.0, 395.0), at(560.0, 12.0, 445.0))]
        }
        // The street canyon at tank-eye height: tenement walls both sides — the frame that judges
        // whether the city reads as masonry or as boxes.
        MapId::Ostrogorsk => {
            vec![named("canyon", at(150.0, 2.4, 446.0), at(400.0, 2.0, 446.0))]
        }
        _ => Vec::new(),
    }
}

/// Prokhorovka's identity views, including the one that judges the SUBJECT: a T-54 at chase
/// distance under the reference look. Deliberately pinned to the golden evening rather than to
/// the map's first declared look — this is the frame the whole art direction aims at.
fn prokhorovka_identity_views(
    battlefield: &BattlefieldMap,
    at: &dyn Fn(f32, f32, f32) -> [f32; 3],
) -> Vec<ReviewView> {
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(5.0);
    let evening =
        crate::weather::weather_look(MapId::ProkhorovkaHill252_2, WeatherVariant::GoldenEvening);
    let noon =
        crate::weather::weather_look(MapId::ProkhorovkaHill252_2, WeatherVariant::ClearAfternoon);

    let tank_x = 520.0_f32;
    let tank_z = 470.0_f32;
    let tank_ground = ground(tank_x, tank_z);

    vec![
        ReviewView {
            name: "prokhorovka_evening_midfield".to_string(),
            eye: at(500.0, 6.0, 460.0),
            target: at(620.0, 2.0, 520.0),
            lighting: evening.lighting,
            sky: evening.sky,
            vehicle: None,
            subject_box: None,
        },
        // Żywy Step P2: a hull-height eye across 300 m of open field — the view that judges the
        // card meadow's band (blades -> cards -> ground) and its seams.
        ReviewView {
            name: "prokhorovka_grass_midfield".to_string(),
            eye: at(430.0, 2.0, 380.0),
            target: at(620.0, 1.0, 520.0),
            lighting: noon.lighting,
            sky: noon.sky,
            vehicle: None,
            subject_box: None,
        },
        // THE READABILITY FRAME. `battlefield_default`'s key points toward +X/+Z, so a camera
        // placed on the -X/-Z side sees the faces the sun never touches — the exact situation a
        // player meets constantly and the one that was failing: with `dot(n, key) <= 0` the key
        // contributes nothing, and the hemispheric ambient alone left the hull, tracks and road
        // wheels as one black silhouette. Nothing about the light may harm reading the vehicle,
        // so the worst case gets a locked frame of its own rather than a note on a screenshot.
        ReviewView {
            name: "prokhorovka_contact_backlit".to_string(),
            // Close, like `closeup_probe`: this frame judges a SURFACE, and at chase distance
            // the hull is too few pixels for its median to mean anything.
            eye: [tank_x - 7.0, tank_ground + 2.3, tank_z - 4.6],
            target: [tank_x + 0.2, tank_ground + 1.05, tank_z + 0.2],
            lighting: noon.lighting,
            sky: noon.sky,
            vehicle: Some(ReviewVehicle {
                kind: VehicleKind::T54_1951,
                position: [tank_x, tank_ground, tank_z],
                yaw_rad: 0.45,
                turret_yaw_rad: 0.0,
                hull_color: [0.30, 0.40, 0.28],
            }),
            // Frames the hull flank and running gear ONLY. Authored against the rendered frame
            // and deliberately kept off the ground: the void UNDER a tank is black correctly, and
            // a box that swallowed it would measure the cast shadow while claiming to measure the
            // vehicle.
            subject_box: Some([0.28, 0.46, 0.78, 0.72]),
        },
        // THE SHADE FRAME. The backlit view above judges a hull the key misses by ANGLE; this one
        // judges a hull the terrain itself puts in shadow — parked in a balka at golden evening,
        // when the key is low enough for a rise to take it away entirely. Cast shadow is the one
        // condition that removes the key without removing the sky, so it is the frame where the
        // ambient term alone has to keep the tank readable. A vehicle that dissolves here is
        // unreadable in every hollow on the map.
        ReviewView {
            name: "prokhorovka_evening_in_shadow".to_string(),
            eye: [tank_x - 8.0, tank_ground + 2.6, tank_z + 5.2],
            target: [tank_x + 0.2, tank_ground + 1.05, tank_z + 0.2],
            lighting: evening.lighting,
            sky: evening.sky,
            vehicle: Some(ReviewVehicle {
                kind: VehicleKind::T54_1951,
                position: [tank_x, tank_ground, tank_z],
                yaw_rad: 2.35,
                turret_yaw_rad: 0.0,
                hull_color: [0.30, 0.40, 0.28],
            }),
            // Hull flank and running gear only, off the ground for the same reason as the
            // backlit frame: the void under a tank is black correctly.
            subject_box: Some([0.28, 0.46, 0.78, 0.72]),
        },
        // The subject, from the seat the player actually occupies.
        ReviewView {
            name: "prokhorovka_evening_contact".to_string(),
            eye: [
                tank_x - CHASE_DISTANCE_M,
                tank_ground + CHASE_EYE_HEIGHT_M,
                tank_z - CHASE_DISTANCE_M * 0.35,
            ],
            target: [tank_x + 40.0, tank_ground + 1.0, tank_z + 14.0],
            lighting: evening.lighting,
            sky: evening.sky,
            vehicle: Some(ReviewVehicle {
                kind: VehicleKind::T54_1951,
                position: [tank_x, tank_ground, tank_z],
                yaw_rad: 0.45,
                turret_yaw_rad: 0.0,
                // The player's own green (`vehicle::render_frame`) — the hull a player stares at
                // for a whole battle.
                hull_color: [0.30, 0.40, 0.28],
            }),
            subject_box: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decision this locks: the weather roll stays, so every look comes up. A variant the
    /// review set skips is a variant the player meets unreviewed.
    #[test]
    fn every_declared_look_on_every_map_has_a_review_view() {
        for map in REVIEWED_MAPS {
            let battlefield = map_forge::battlefield(map);
            let views = review_views_for(map, &battlefield);
            for variant in declared_looks(map) {
                let wanted = format!("{}_{}", map_key(map), variant_key(variant));
                assert!(
                    views.iter().any(|view| view.name == wanted),
                    "{map:?} declares {variant:?} but the review set has no {wanted}"
                );
            }
        }
    }

    /// Every shipped map is reviewed. A map that forgets to register here would ship a look
    /// nobody ever looked at.
    #[test]
    fn every_shipped_map_is_in_the_review_set() {
        for map in MapId::ALL {
            assert!(
                REVIEWED_MAPS.contains(map),
                "{map:?} ships but is not in REVIEWED_MAPS — its look is never reviewed"
            );
            let battlefield = map_forge::battlefield(*map);
            assert!(
                !review_views_for(*map, &battlefield).is_empty(),
                "{map:?} is listed as reviewed but contributes no view"
            );
        }
    }

    /// The review set exists to judge the frame the PLAYER meets. A panorama shot from above the
    /// chase camera judges a vantage the game never gives anyone — which is how the old 14 m
    /// panoramas passed review while the played picture was wrong.
    #[test]
    fn no_review_camera_sits_above_the_players_own_eye() {
        for map in REVIEWED_MAPS {
            let battlefield = map_forge::battlefield(map);
            for view in review_views_for(map, &battlefield) {
                let ground =
                    battlefield.heightmap.sample_height(view.eye[0], view.eye[2]).unwrap_or(0.0);
                let above = view.eye[1] - ground;
                assert!(
                    above <= CHASE_EYE_HEIGHT_M + 1.5,
                    "{}: eye sits {above:.1} m above ground, past the player's own \
                     {CHASE_EYE_HEIGHT_M:.1} m",
                    view.name
                );
            }
        }
    }

    /// The subject of this game is a tank, so at least one locked frame must contain one — a
    /// landscape-only set cannot catch a hero that fails to ground or separate from its field.
    #[test]
    fn the_review_set_puts_a_vehicle_in_front_of_the_camera() {
        let battlefield = map_forge::battlefield(MapId::ProkhorovkaHill252_2);
        let views = review_views_for(MapId::ProkhorovkaHill252_2, &battlefield);
        let with_vehicle: Vec<_> = views.iter().filter(|view| view.vehicle.is_some()).collect();
        assert!(!with_vehicle.is_empty(), "no review view carries a vehicle");
        for view in with_vehicle {
            let vehicle = view.vehicle.expect("filtered to Some");
            let range =
                glam::Vec3::from_array(view.eye).distance(glam::Vec3::from_array(vehicle.position));
            assert!(
                (4.0..=200.0).contains(&range),
                "{}: the vehicle is {range:.1} m from the eye — too far to judge its surface",
                view.name
            );
        }
    }

    /// Golden filenames are the review set's identity; a collision would silently overwrite one
    /// frame with another and lock half of what we think we locked.
    #[test]
    fn review_view_names_are_unique_across_every_map() {
        let mut seen = std::collections::BTreeSet::new();
        for map in REVIEWED_MAPS {
            let battlefield = map_forge::battlefield(map);
            for view in review_views_for(map, &battlefield) {
                assert!(seen.insert(view.name.clone()), "duplicate review view name {}", view.name);
            }
        }
    }
}
