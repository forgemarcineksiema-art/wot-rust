//! The canonical look-review views: the exact camera + lighting setups the art-direction
//! policy is judged against (`docs/art-direction-policy.md`). One source of truth shared by
//! the `prokhorovka_views` example (human eyes) and the `look_goldens` harness (locked
//! regression) — both render them through `client::render_review_views`, so "the picture we
//! reviewed" and "the picture we locked" cannot drift apart.

use game_core::VehicleKind;
use renderer_api::SceneLighting;
use terrain::BattlefieldMap;

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
    pub name: &'static str,
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub lighting: SceneLighting,
    pub sky: (f64, f64, f64),
    /// The subject, when this view has one. A landscape-only review set cannot catch a hero that
    /// fails to separate from its ground, and the subject of this game is a tank.
    pub vehicle: Option<ReviewVehicle>,
}

/// The player's own eye, derived from the battle chase camera rather than guessed: 12 m back at
/// 0.42 rad (`client::camera::types` defaults) puts the eye ~4.9 m above the hull it follows.
/// The review panoramas used to sit at 14 m — a vantage the game never gives anyone, which is
/// how a picture can pass review and still look wrong in play.
pub const CHASE_EYE_HEIGHT_M: f32 = 4.9;
const CHASE_DISTANCE_M: f32 = 12.0;

/// The Prokhorovka review set: the hill panorama in its three authored times of day, a mid-field
/// vantage, the grass band, and a tank at fighting range — all from heights the player occupies.
pub fn prokhorovka_review_views(battlefield: &BattlefieldMap) -> [ReviewView; 6] {
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(5.0);
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];

    // The contact view: a T-54 held at chase distance, looking down the killzone. This is the
    // frame that judges whether the hero grounds, separates from the field, and holds its paint.
    let tank_x = 520.0_f32;
    let tank_z = 470.0_f32;
    let tank_ground = ground(tank_x, tank_z);
    let contact_vehicle = ReviewVehicle {
        kind: VehicleKind::T54_1951,
        position: [tank_x, tank_ground, tank_z],
        yaw_rad: 0.45,
        turret_yaw_rad: 0.0,
        // The player's own green (`vehicle::render_frame`), because this is the hull a player
        // stares at for a whole battle.
        hull_color: [0.30, 0.40, 0.28],
    };

    [
        ReviewView {
            name: "prokhorovka_noon",
            eye: at(250.0, CHASE_EYE_HEIGHT_M, 452.0),
            target: at(700.0, 2.0, 505.0),
            lighting: SceneLighting::battlefield_default(),
            sky: (0.55, 0.69, 0.87),
            vehicle: None,
        },
        ReviewView {
            name: "prokhorovka_golden_evening",
            eye: at(250.0, CHASE_EYE_HEIGHT_M, 452.0),
            target: at(700.0, 2.0, 505.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
            vehicle: None,
        },
        ReviewView {
            name: "prokhorovka_overcast",
            eye: at(250.0, CHASE_EYE_HEIGHT_M, 452.0),
            target: at(700.0, 2.0, 505.0),
            lighting: SceneLighting::prokhorovka_overcast(),
            sky: (0.48, 0.51, 0.55),
            vehicle: None,
        },
        ReviewView {
            name: "prokhorovka_evening_midfield",
            eye: at(500.0, 6.0, 460.0),
            target: at(620.0, 2.0, 520.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
            vehicle: None,
        },
        // Żywy Step P2: a hull-height eye looking across 300 m of open field — the view that
        // judges the card meadow's band (blades -> cards -> ground) and its seams.
        ReviewView {
            name: "prokhorovka_grass_midfield",
            eye: at(430.0, 2.0, 380.0),
            target: at(620.0, 1.0, 520.0),
            lighting: SceneLighting::battlefield_default(),
            sky: (0.62, 0.68, 0.75),
            vehicle: None,
        },
        // The subject, from the seat the player actually occupies: chase distance behind a T-54
        // under the reference look, with the far ridge past it for depth.
        ReviewView {
            name: "prokhorovka_evening_contact",
            eye: [
                tank_x - CHASE_DISTANCE_M,
                tank_ground + CHASE_EYE_HEIGHT_M,
                tank_z - CHASE_DISTANCE_M * 0.35,
            ],
            target: [tank_x + 40.0, tank_ground + 1.0, tank_z + 14.0],
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.80, 0.62, 0.45),
            vehicle: Some(contact_vehicle),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The review set exists to judge the frame the PLAYER meets. A panorama shot from above the
    /// chase camera judges a vantage the game never gives anyone — which is how the 14 m
    /// panoramas passed review while the played picture was wrong.
    #[test]
    fn no_review_camera_sits_above_the_players_own_eye() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        for view in prokhorovka_review_views(&battlefield) {
            let ground =
                battlefield.heightmap.sample_height(view.eye[0], view.eye[2]).unwrap_or(0.0);
            let above = view.eye[1] - ground;
            assert!(
                above <= CHASE_EYE_HEIGHT_M + 1.5,
                "{}: eye sits {above:.1} m above ground, past the player's own {CHASE_EYE_HEIGHT_M:.1} m",
                view.name
            );
        }
    }

    /// The subject of this game is a tank, so at least one locked frame must contain one — a
    /// landscape-only set cannot catch a hero that fails to ground or separate from its field.
    #[test]
    fn the_review_set_puts_a_vehicle_in_front_of_the_camera() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let views = prokhorovka_review_views(&battlefield);
        let with_vehicle: Vec<_> = views.iter().filter(|v| v.vehicle.is_some()).collect();
        assert!(!with_vehicle.is_empty(), "no review view carries a vehicle");
        for view in with_vehicle {
            let vehicle = view.vehicle.expect("filtered to Some");
            let eye = glam::Vec3::from_array(view.eye);
            let hull = glam::Vec3::from_array(vehicle.position);
            let range = eye.distance(hull);
            assert!(
                (4.0..=200.0).contains(&range),
                "{}: the vehicle is {range:.1} m from the eye — too far to judge its surface",
                view.name
            );
        }
    }
}
