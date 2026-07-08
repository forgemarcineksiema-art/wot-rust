use game_core::TankId;
use net::{Snapshot, TankSnapshot};

use super::ClientApp;

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
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
    }
}
