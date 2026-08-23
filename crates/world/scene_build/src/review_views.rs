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
    /// Normalized `[x0, y0, x1, y1]` crop framing the parked hero, on the ROOM view only.
    ///
    /// The battlefield has had subject crops since the backlit-flank work; the garage did not,
    /// which is the sharper omission of the two. `docs/art-direction-policy.md` says of this
    /// room that "the hero is the brightest, most contrasted, most detailed thing in frame" and
    /// that "if the room out-reads the vehicle, the shot has failed no matter how well lit the
    /// room is" — and nothing measured it. The frame-wide statistics cannot: the hero is about a
    /// sixth of the picture, so the hall could swallow it whole and the value planes would not
    /// notice.
    pub subject_box: Option<[f32; 4]>,
    /// A CLOSE-UP is a subject photograph, not a room photograph (F3): a 4.5 m frame of road
    /// wheels contains no skylight and no lamp, so the room's value-plane bounds — three
    /// planes, a bright source — describe nothing about it (the first recording measured
    /// bright 0.1% against the room's 2% floor with a picture that was perfectly readable).
    /// A close-up answers to its SUBJECT bounds instead; the plane locks skip it.
    pub close_up: bool,
    /// The armor inspector overlay (I1): the client renders the vehicle's gameplay armor
    /// volumes as translucent zone-colored FX faces over the hero. One view locks it.
    pub inspector: bool,
    /// Which garage screen this view locks (see [`GarageScreen`]).
    pub screen: GarageScreen,
}

/// The garage screens under image lock.
///
/// [`GarageScreen::Room`] reviews the ROOM — light, materials, framing — and the value-structure
/// locks read that frame. The rest review SCREENS the player actually meets, which are a
/// different object: panels, plates, glyphs and hit-target geometry, none of which is a
/// photograph and none of which the grading rules apply to.
///
/// The garage UI had no picture lock of ANY kind until the hero pair landed, and even then only
/// the hangar screen was covered: the tech tree and the module option list — the two screens a
/// player reaches by pressing `T` or clicking a slot — shipped with unit tests over rect
/// arithmetic and nothing else. Rect arithmetic cannot see a node drawn past its panel (which
/// the tech tree did, for five vehicles), a glyph that went missing, or text that lost contrast
/// against what it sits on. Every screen the garage can show is listed here, and
/// `every_garage_screen_is_under_an_image_lock` fails if one is added without a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GarageScreen {
    /// No overlay: the hangar as a photograph.
    Room,
    /// The hangar screen — carousel, loadout strip, crew and vehicle columns.
    Hangar,
    /// The browse-only tech tree (`T`).
    TechTree,
    /// A module slot's option list, open over the hangar screen.
    OptionList,
}

impl GarageScreen {
    /// Every screen, so the review set and its coverage lock cannot disagree about the list.
    pub const ALL: [GarageScreen; 4] = [
        GarageScreen::Room,
        GarageScreen::Hangar,
        GarageScreen::TechTree,
        GarageScreen::OptionList,
    ];

    /// The golden's name for this screen.
    pub fn view_name(self) -> &'static str {
        match self {
            GarageScreen::Room => "garage_hero",
            GarageScreen::Hangar => "garage_screen",
            GarageScreen::TechTree => "garage_tech_tree",
            GarageScreen::OptionList => "garage_option_list",
        }
    }
}

/// The hero-frame crop the subject statistics measure, DERIVED from the framing constants
/// rather than hand-computed (the old literal was the same arithmetic done in a comment, and
/// it silently measured the wrong pixels the moment the boom moved): the pivot projects to
/// frame centre, the lens puts `540 / (2·D·tan(fov/2))` pixels on a metre at the hero boom,
/// and the parked T-54 subtends ~6.5 m across and ~2.4 m up at its three-quarter. Bottom cut
/// at ~0.5 m above the deck so the box measures the VEHICLE and not the contact shadow under
/// it — the same reason the Prokhorovka boxes stay off the ground.
fn hero_subject_box() -> [f32; 4] {
    subject_box_at(6.5, 2.4, crate::hangar::HERO_ORBIT_DISTANCE)
}

/// The same crop arithmetic for any silhouette span, height and boom — [`hero_subject_box`]
/// keeps the T-54's empirical 6.5 x 2.4 m; the heavy-fleet views (F3) derive theirs from the
/// spec through [`heavy_subject_box`].
fn subject_box_at(span_m: f32, height_m: f32, boom_m: f32) -> [f32; 4] {
    let fov = crate::hangar::HERO_FOV_DEGREES.to_radians();
    let px_per_m = 540.0 / (2.0 * boom_m * (fov / 2.0).tan());
    let pivot_y = crate::hangar::hangar_camera_pivot().y;
    let deck = crate::hangar::TURNTABLE_TOP_M;
    let half_w = 0.5 * span_m * px_per_m / 960.0;
    let top = 0.5 - (deck + height_m - pivot_y) * px_per_m / 540.0;
    let bottom = 0.5 + (pivot_y - (deck + 0.5)) * px_per_m / 540.0;
    [0.5 - half_w, top, 0.5 + half_w, bottom]
}

/// A heavy vehicle's subject crop, derived from its SPEC at its own hero boom (F3): the
/// silhouette at the parked three-quarter (0.65 rad off the bearing) projects hull length by
/// its cosine and hull width by its sine, plus a quarter of the stock barrel for the muzzle
/// spilling past the bow; height is the hitbox plus 0.3 m of cupola/MG furniture.
fn heavy_subject_box(kind: VehicleKind) -> [f32; 4] {
    let hitbox = kind.spec().hitbox;
    let offset = crate::hangar::HERO_PARK_YAW - crate::hangar::HERO_ORBIT_YAW;
    let span = (2.0 * hitbox.half_length_m + 0.25 * kind.stock_barrel_length_m()) * offset.cos()
        + 2.0 * hitbox.half_width_m * offset.sin();
    subject_box_at(span, 2.0 * hitbox.half_height_m + 0.3, crate::hangar::hero_orbit_boom_for(kind))
}

/// The garage review set: the hero shot the garage actually opens with. The framing comes from
/// `hangar::HERO_ORBIT_*` — the same constants the live orbit camera rests at — so a reframing
/// moves the played picture and the locked picture together.
pub fn hangar_review_views() -> Vec<HangarReviewView> {
    let hero = |screen: GarageScreen| HangarReviewView {
        name: screen.view_name().to_string(),
        eye: crate::hangar::hero_orbit_eye().to_array(),
        target: crate::hangar::hangar_camera_pivot().to_array(),
        lighting: SceneLighting::garage_hero(),
        // READ from the one place the live client reads it, not copied from it: a literal here
        // is how the goldens ended up locking a near-black sky through roof openings the game
        // fills with daylight.
        background: crate::hangar::INTERIOR_BACKGROUND,
        vehicle: ReviewVehicle {
            kind: VehicleKind::T54_1951,
            position: [0.0, crate::hangar::TURNTABLE_TOP_M, 0.0],
            // Three-quarter to the camera — and now actually three-quarter, read from the same
            // constant `garage_preview_snapshot` parks the live hero at.
            yaw_rad: crate::hangar::HERO_PARK_YAW,
            turret_yaw_rad: 0.0,
            // The garage's own showroom tint, not the battle green.
            hull_color: [0.72, 0.76, 0.62],
        },
        // The hull and turret mass, on the ROOM view only — the overlay views are half
        // instrument panel and answer to their own locks.
        subject_box: (screen == GarageScreen::Room).then_some(hero_subject_box()),
        close_up: false,
        inspector: false,
        screen,
    };
    // The room, then every screen drawn over it. Same framing, same light, same hero: the views
    // differ by exactly the overlay, so a diff between any two of them is the UI and nothing else.
    let mut views: Vec<HangarReviewView> = GarageScreen::ALL.into_iter().map(hero).collect();

    // F3: the close orbit and the heavy fleet. The hero framing was designed on a T-54 and
    // reviewed on nothing else; these lock the shots that broke first — the suspension pass
    // the live camera flies for the running-gear slot, and the two longest vehicles in the
    // roster at their own spec-derived boom (at 15 m the Jagdtiger ran its barrel off the
    // frame's right edge).
    let (susp_eye, susp_target) = crate::hangar::slot_eye(crate::hangar::FRAMING_SUSPENSION);
    views.push(HangarReviewView {
        name: "garage_susp_close".to_string(),
        eye: susp_eye.to_array(),
        target: susp_target.to_array(),
        lighting: SceneLighting::garage_hero(),
        background: crate::hangar::INTERIOR_BACKGROUND,
        vehicle: ReviewVehicle {
            kind: VehicleKind::T54_1951,
            position: [0.0, crate::hangar::TURNTABLE_TOP_M, 0.0],
            yaw_rad: crate::hangar::HERO_PARK_YAW,
            turret_yaw_rad: 0.0,
            hull_color: [0.72, 0.76, 0.62],
        },
        // The close pass IS the subject: the crop trims only the frame's edges (the wall
        // sliver top-right, the floor at the bottom corners) and the subject bounds do the
        // locking — the room planes skip a close-up entirely.
        subject_box: Some([0.06, 0.06, 0.94, 0.94]),
        close_up: true,
        inspector: false,
        screen: GarageScreen::Room,
    });
    for (name, kind) in [
        ("garage_hero_tiger2", VehicleKind::TigerII),
        ("garage_hero_jagdtiger", VehicleKind::Jagdtiger),
    ] {
        views.push(HangarReviewView {
            name: name.to_string(),
            eye: crate::hangar::hero_orbit_eye_for(kind).to_array(),
            target: crate::hangar::hangar_camera_pivot().to_array(),
            lighting: SceneLighting::garage_hero(),
            background: crate::hangar::INTERIOR_BACKGROUND,
            vehicle: ReviewVehicle {
                kind,
                position: [0.0, crate::hangar::TURNTABLE_TOP_M, 0.0],
                yaw_rad: crate::hangar::HERO_PARK_YAW,
                turret_yaw_rad: 0.0,
                hull_color: [0.72, 0.76, 0.62],
            },
            subject_box: Some(heavy_subject_box(kind)),
            close_up: false,
            inspector: false,
            screen: GarageScreen::Room,
        });
    }

    // I1: the armor inspector — the hero shot with the gameplay armor volumes drawn over the
    // vehicle. One golden locks that the inspector still SHOWS (zones colored, discs stamped,
    // vehicle readable underneath); the numbers it draws are locked at their source.
    views.push(HangarReviewView {
        name: "garage_inspector".to_string(),
        eye: crate::hangar::hero_orbit_eye().to_array(),
        target: crate::hangar::hangar_camera_pivot().to_array(),
        lighting: SceneLighting::garage_hero(),
        background: crate::hangar::INTERIOR_BACKGROUND,
        vehicle: ReviewVehicle {
            kind: VehicleKind::T54_1951,
            position: [0.0, crate::hangar::TURNTABLE_TOP_M, 0.0],
            yaw_rad: crate::hangar::HERO_PARK_YAW,
            turret_yaw_rad: 0.0,
            hull_color: [0.72, 0.76, 0.62],
        },
        // The subject is deliberately UNDER an overlay here; the hero view already measures
        // the bare vehicle, and a crop of translucent color would measure the overlay.
        subject_box: None,
        close_up: false,
        inspector: true,
        screen: GarageScreen::Room,
    });
    views
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

/// The short per-map key the review render paths and the look goldens agree on.
pub fn map_key(map: MapId) -> &'static str {
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
            // THE COUNTERWEIGHT. The backlit frame measures the side the sun never reaches, so
            // every lever that helps it is a lever that could wash out the side the sun DOES
            // reach — and this is the frame the program calls golden, the one that must not be
            // spent paying for the other. Framed on the hull and running gear at the distance a
            // player actually sits, so "the fix did not flatten the good frame" is a number.
            subject_box: Some([0.37, 0.62, 0.62, 0.83]),
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
        for map in MapId::SHIPPED {
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

    /// A screen the garage can show and the review set cannot is a screen that ships
    /// unreviewed — the same rule `REVIEWED_MAPS` enforces for maps, applied to the UI. The
    /// tech tree and the option list lived outside it until this lock existed, which is how a
    /// tech-tree node could print past its panel for five vehicles without a picture noticing.
    #[test]
    fn every_garage_screen_is_under_an_image_lock() {
        let views = hangar_review_views();
        for screen in GarageScreen::ALL {
            assert!(
                views.iter().any(|view| view.screen == screen),
                "{screen:?} has no review view — it would ship with no picture lock"
            );
        }
        // Every OVERLAY screen keeps exactly one view (a second copy of a UI lock is drift
        // waiting to happen); the ROOM legitimately carries more than one since F3 — the hero
        // shot plus the close orbit and the heavy fleet, each a different photograph of the
        // same room.
        for screen in GarageScreen::ALL {
            let count = views.iter().filter(|view| view.screen == screen).count();
            if screen == GarageScreen::Room {
                assert!(count >= 1, "the room keeps at least its hero view");
            } else {
                assert_eq!(count, 1, "{screen:?}: one view per overlay screen, no strays");
            }
        }
        // The room view the value-structure locks and the UI-footprint diffs read FIRST must
        // stay the hero shot, and every review view keeps a unique golden name.
        let room = views.iter().find(|view| view.screen == GarageScreen::Room).expect("room view");
        assert_eq!(room.name, "garage_hero");
        let mut names: Vec<&str> = views.iter().map(|view| view.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), views.len(), "duplicate golden names in the review set");
    }

    /// The world layer names no vehicle (`vehicle_dispatch` ratchet), so the span the hero
    /// framing was designed on lives in `hangar.rs` as a LITERAL — and this lock, in the one
    /// file allowlisted to name the fleet, pins that literal to the benchmark's own
    /// spec-derived span. If the T-54's hull or stock gun ever changes, this fails and the
    /// framing constant follows the data — never the other way round.
    #[test]
    fn the_hero_framed_span_is_the_benchmarks_own() {
        let benchmark = crate::hangar::hero_span_m(VehicleKind::T54_1951);
        assert!(
            (crate::hangar::HERO_FRAMED_SPAN_M - benchmark).abs() < 0.05,
            "HERO_FRAMED_SPAN_M {} has drifted from the benchmark's spec span {benchmark}",
            crate::hangar::HERO_FRAMED_SPAN_M
        );
    }

    /// The garage's sky is single-sourced: what shows through the roof openings in a review
    /// frame is what the live client paints there. The two used to be separate literals, and
    /// when the roof gained real openings the goldens kept locking a near-black sky the game
    /// had already replaced with daylight.
    #[test]
    fn the_review_background_is_the_clients_own_interior_background() {
        for view in hangar_review_views() {
            assert_eq!(
                view.background,
                crate::hangar::INTERIOR_BACKGROUND,
                "{} locks a different sky than the game paints",
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
