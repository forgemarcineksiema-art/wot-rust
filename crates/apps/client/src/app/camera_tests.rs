use game_core::TankId;
use net::{Snapshot, TankSnapshot};

use super::ClientApp;

/// D9's contract: death hands the camera a wide, slowly orbiting spectate — the boom flows
/// past the map's alive ceiling, the view drifts, and the sniper scope is refused. Revival
/// (a new battle from the garage) hands the rig back.
#[test]
fn death_gives_the_wreck_a_wide_slow_orbit_and_refuses_the_scope() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_with_aim(tank_id, 3, 0.4, 0.0));

    // Alive: no spectate, the boom sits at the dialed distance.
    assert!(!app.tick_death_spectate(), "a living player is not spectating");
    let alive = app.presented_camera_for_player(1.0, 0.05).expect("camera");
    let alive_boom = boom_length(&alive);

    // Kill the player; the spectate takes the screen and forces third person.
    let mut dead = snapshot_with_aim(tank_id, 4, 0.4, 0.0);
    dead.tanks[0].hit_points = 0;
    app.accept_and_sync(dead);
    app.camera_controller.set_mode(crate::BattleCameraMode::Sniper);
    assert!(app.tick_death_spectate(), "a dead player spectates");
    assert_eq!(
        app.camera_controller.mode(),
        crate::BattleCameraMode::ThirdPerson,
        "a dead gunner sights nothing"
    );

    // Three presented seconds later the boom has flowed wide and the view has drifted.
    let yaw_at_death = app.camera_controller.orbit_yaw_rad();
    let mut last = alive;
    for _ in 0..60 {
        last = app.presented_camera_for_player(1.0, 0.05).expect("camera");
    }
    let dead_boom = boom_length(&last);
    assert!(
        dead_boom > alive_boom * 1.4,
        "the death boom must flow wide: alive {alive_boom} vs dead {dead_boom}"
    );
    assert!(
        (app.camera_controller.orbit_yaw_rad() - yaw_at_death).abs() > 0.1,
        "the spectate drifts around the wreck"
    );
}

/// D9's other half: the map names its widest boom — the open steppe earns a longer leash
/// than the closed river valley, and both outgrow the shared default.
#[test]
fn each_map_names_its_own_widest_boom() {
    let steppe = ClientApp::map_camera_settings(terrain::MapId::ProkhorovkaHill252_2);
    let valley = ClientApp::map_camera_settings(terrain::MapId::BystraValley);
    assert_eq!(steppe.max_distance_m, 23.0);
    assert_eq!(valley.max_distance_m, 20.0);
    assert!(
        steppe.max_distance_m > valley.max_distance_m,
        "open ground sees farther than a valley"
    );
    let shared_default = crate::BattleCameraSettings::default().max_distance_m;
    assert!(valley.max_distance_m > shared_default, "the per-map leash outgrows the default");
}

fn boom_length(camera: &renderer_api::Camera) -> f32 {
    let eye = glam::Vec3::from_array(camera.eye);
    let target = glam::Vec3::from_array(camera.target);
    eye.distance(target)
}

#[test]
fn third_person_camera_from_tank_uses_desired_yaw_without_waiting_for_turret() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_with_aim(tank_id, 3, std::f32::consts::FRAC_PI_2, 0.0));
    app.desired_aim = crate::aim::DesiredAim::new(0.0, 0.0);
    let tank = app.player_snapshot().expect("player tank").clone();
    let position = tank.position;

    let camera = app.camera_from_tank(tank);

    assert!(
        camera.target[2] > position[2] + 3.0,
        "TPP camera should follow the player sight yaw immediately"
    );
    assert!(camera.eye[2] < position[2], "TPP camera should sit behind the sight lane");
}

#[test]
fn third_person_camera_from_tank_keeps_free_look_orbit() {
    let mut app = ClientApp::new();
    let tank_id = app.player_tank;
    app.accept_and_sync(snapshot_with_aim(tank_id, 3, std::f32::consts::FRAC_PI_2, 0.0));
    app.input.free_look = true;
    app.camera_controller.set_orbit_yaw(0.0);
    let tank = app.player_snapshot().expect("player tank").clone();
    let position = tank.position;

    let camera = app.camera_from_tank(tank);

    assert!(
        camera.target[2] > position[2] + 3.0,
        "free-look should keep using the camera orbit yaw"
    );
    assert!(camera.eye[2] < position[2], "free-look camera should stay behind the orbit lane");
}

fn snapshot_with_aim(
    tank_id: TankId,
    server_tick: u64,
    turret_yaw_rad: f32,
    gun_pitch_rad: f32,
) -> Snapshot {
    Snapshot {
        server_tick,
        tanks: vec![TankSnapshot {
            tank_id,
            team: game_core::TeamId(1),
            vehicle: game_core::VehicleKind::PrototypeMedium,
            position: [10.0, 0.0, 10.0],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 2.5,
            module_hit_points: game_core::VehicleKind::PrototypeMedium
                .spec()
                .module_health
                .hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    }
}
