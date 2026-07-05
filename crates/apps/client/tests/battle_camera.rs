use std::f32::consts::FRAC_PI_2;

use client::{
    BattleCameraController, BattleCameraEnvironment, BattleCameraInput, BattleCameraMode,
    BattleCameraSettings, CameraObstacle, CameraSubject,
};
use game_core::TankId;
use net::TankSnapshot;
use terrain::{HeightMap, prokhorovka_hill_252_2};

#[test]
fn third_person_camera_follows_tank_and_stays_above_terrain() {
    let heightmap = HeightMap::flat(32, 32, 1.0, 3.0).expect("heightmap");
    let subject = CameraSubject::from_snapshot(tank_snapshot([10.0, 3.0, 10.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);
    let render_camera =
        camera.render_camera(&subject, &BattleCameraEnvironment::with_terrain(&heightmap));

    assert!(render_camera.eye[2] < subject.position[2], "TPP camera should sit behind hull");
    assert!(render_camera.eye[1] >= 3.0 + camera.settings().terrain_clearance_m);
    assert!(render_camera.target[1] > subject.position[1]);
    assert!(
        render_camera.target[2] > subject.position[2] + 3.0,
        "TPP aim camera should look ahead of the hull instead of centering the player's turret"
    );
    assert!(
        (render_camera.eye[0] - subject.position[0]).abs() < 0.05,
        "at battle distance the camera is CENTERED on the vehicle, not beside it"
    );
    assert_eq!(render_camera.vertical_fov_degrees, camera.settings().third_person_fov_degrees);
}

#[test]
fn third_person_camera_shortens_boom_against_static_cover_obstacles() {
    let heightmap = HeightMap::flat(32, 32, 1.0, 0.0).expect("heightmap");
    let mut environment = BattleCameraEnvironment::with_terrain(&heightmap);
    environment.add_obstacle(CameraObstacle::aabb(
        "stone_cover",
        [11.0, 5.5, 6.0],
        [1.0, 2.0, 1.0],
    ));
    let subject = CameraSubject::from_snapshot(tank_snapshot([10.0, 0.0, 10.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);
    let render_camera = camera.render_camera(&subject, &environment);

    assert!(render_camera.eye[2] > 6.0, "camera boom should be pulled in before obstacle");
    assert!(render_camera.eye[2] < render_camera.target[2]);
}

#[test]
fn boom_collision_catches_obstacles_thinner_than_a_sampling_step() {
    let heightmap = HeightMap::flat(32, 32, 1.0, 0.0).expect("heightmap");
    let mut environment = BattleCameraEnvironment::with_terrain(&heightmap);
    // A 4 cm plate: the old 32-point sampling strode ~0.38 m and could step clean over it.
    environment.add_obstacle(CameraObstacle::aabb("fence", [11.0, 4.0, 8.0], [3.0, 3.0, 0.02]));
    let subject = CameraSubject::from_snapshot(tank_snapshot([10.0, 0.0, 10.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);

    let frame = camera.render_camera(&subject, &environment);

    assert!(frame.eye[2] > 8.0, "boom must stop on the tank side of the plate: {}", frame.eye[2]);
}

#[test]
fn boom_shortens_in_front_of_a_terrain_ridge_instead_of_seeing_through_it() {
    // Flat map with a 10 m ridge across z = 13..15; the tank sits at z = 24 looking +Z, so the
    // fully zoomed-out boom would put the eye behind the ridge with the sight line through it.
    let mut samples = vec![0.0; 64 * 64];
    for z in 13..=15 {
        for x in 0..64 {
            samples[z * 64 + x] = 10.0;
        }
    }
    let heightmap = HeightMap::new(64, 64, 1.0, samples).expect("heightmap");
    let subject = CameraSubject::from_snapshot(tank_snapshot([32.0, 0.0, 24.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);
    camera.apply_input(BattleCameraInput {
        orbit_yaw_delta_rad: 0.0,
        pitch_delta_rad: 0.0,
        zoom_delta_m: 100.0, // longest boom
    });

    let frame = camera.render_camera(&subject, &BattleCameraEnvironment::with_terrain(&heightmap));

    assert!(
        frame.eye[2] > 15.0,
        "boom must stop on the tank side of the ridge, eye z = {}",
        frame.eye[2]
    );
}

#[test]
fn third_person_camera_follows_the_hull_rigidly_without_lag() {
    // The eye spring was removed: the camera tracks an already-smooth subject 1:1.
    let environment = BattleCameraEnvironment::empty();
    let subject_a = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, 0.0), 0.0);
    let subject_b = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 40.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);

    let eye_a = camera.render_camera(&subject_a, &environment).eye;
    let eye_b = camera.render_camera(&subject_b, &environment).eye;

    assert!((eye_b[2] - eye_a[2] - 40.0).abs() < 1.0e-4, "eye must shift 1:1 with the hull");
    assert!((eye_b[0] - eye_a[0]).abs() < 1.0e-4);
    assert!((eye_b[1] - eye_a[1]).abs() < 1.0e-4);
}

#[test]
fn the_camera_stays_centered_at_every_boom_length_so_zoom_cannot_slide_the_view() {
    let environment = BattleCameraEnvironment::empty();
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, FRAC_PI_2), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);

    // Scroll the boom through its whole range: the lateral placement must not move â€” a
    // zoom-dependent offset swept the scene sideways on every scroll (the "slides left" bug).
    let far = camera.render_camera(&subject, &environment);
    assert!(far.target[0] > subject.position[0] + 3.0, "TPP target leads the current turret");
    for _ in 0..24 {
        camera.apply_input(BattleCameraInput { zoom_delta_m: -0.8, ..Default::default() });
        let stepped = camera.render_camera(&subject, &environment);
        if camera.mode() != BattleCameraMode::ThirdPerson {
            break;
        }
        assert!(
            (stepped.eye[2] - far.eye[2]).abs() < 1.0e-4,
            "zooming must not slide the view sideways, drifted {}",
            (stepped.eye[2] - far.eye[2]).abs()
        );
        assert!(stepped.target[2].abs() < 1.0e-4, "the sight lane stays centered on the hull");
    }
}

#[test]
fn third_person_view_direction_is_invariant_under_zoom() {
    // The over-shoulder offset shifts the whole sight lane; if it sat on the eye only, changing
    // the boom length rotated the view and dragged the aim point sideways with every scroll.
    let environment = BattleCameraEnvironment::empty();
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.3, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::ThirdPerson);

    let direction = |camera: &BattleCameraController| {
        let frame = camera.render_camera(&subject, &environment);
        let eye = glam::Vec3::from_array(frame.eye);
        (glam::Vec3::from_array(frame.target) - eye).normalize()
    };
    let near = direction(&camera);
    camera.apply_input(BattleCameraInput {
        orbit_yaw_delta_rad: 0.0,
        pitch_delta_rad: 0.0,
        zoom_delta_m: 6.0,
    });
    let far = direction(&camera);

    assert!((near - far).length() < 1.0e-5, "zoom must not rotate the view: {near} vs {far}");
}

#[test]
fn sniper_eye_does_not_translate_while_the_turret_traverses() {
    // The eye sits on the turret-ring axis, so turret catch-up after a mouse flick rotates the
    // view without sliding the world sideways.
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);
    let environment = BattleCameraEnvironment::empty();

    let still = CameraSubject::from_snapshot(tank_snapshot([5.0, 0.0, 5.0], 0.4, 0.0), 0.0)
        .with_desired_aim(1.2, 0.0);
    let slewing = CameraSubject::from_snapshot(tank_snapshot([5.0, 0.0, 5.0], 0.4, 0.9), 0.0)
        .with_desired_aim(1.2, 0.0);

    let eye_a = camera.render_camera(&still, &environment).eye;
    let eye_b = camera.render_camera(&slewing, &environment).eye;

    assert!(
        (glam::Vec3::from_array(eye_a) - glam::Vec3::from_array(eye_b)).length() < 1.0e-5,
        "traverse must not move the sniper eye: {eye_a:?} vs {eye_b:?}"
    );
}

#[test]
fn camera_environment_imports_static_cover_from_battlefield_map() {
    let map = prokhorovka_hill_252_2();
    let environment = BattleCameraEnvironment::for_battlefield(&map);

    assert_eq!(environment.obstacles.len(), map.static_cover.len());
    assert!(environment.terrain.is_some());
}

#[test]
fn camera_environment_can_borrow_prebuilt_obstacles_for_hot_render_paths() {
    let heightmap = HeightMap::flat(32, 32, 1.0, 0.0).expect("heightmap");
    let obstacles = [CameraObstacle::aabb("barn", [8.0, 2.0, 8.0], [2.0, 2.0, 2.0])];

    let environment = BattleCameraEnvironment::with_obstacles(&heightmap, &obstacles);

    assert_eq!(environment.obstacles.len(), 1);
    assert_eq!(environment.obstacles[0].label, "barn");
    assert!(environment.terrain.is_some());
}

#[test]
fn sniper_camera_uses_turret_yaw_and_gun_pitch() {
    let subject =
        CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, FRAC_PI_2), -0.08);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);

    let render_camera = camera.render_camera(&subject, &BattleCameraEnvironment::empty());

    assert!(render_camera.target[0] > render_camera.eye[0] + 100.0);
    assert!(render_camera.target[1] < render_camera.eye[1], "negative gun pitch aims downward");
    assert_eq!(render_camera.vertical_fov_degrees, camera.settings().sniper_fov_degrees);
}

#[test]
fn sniper_camera_uses_desired_aim_instead_of_current_gun_pitch() {
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, 0.0), 0.25)
        .with_desired_aim(0.0, -0.12);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);

    let render_camera = camera.render_camera(&subject, &BattleCameraEnvironment::empty());

    assert!(render_camera.target[1] < render_camera.eye[1], "desired aim pitch points downward");
}

#[test]
fn sniper_eye_stays_on_current_turret_while_view_tracks_desired_aim() {
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, 0.0), 0.0)
        .with_desired_aim(FRAC_PI_2, 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);

    let render_camera = camera.render_camera(&subject, &BattleCameraEnvironment::empty());

    assert!(render_camera.eye[0].abs() < 1.0e-4, "eye should not slide to desired yaw");
    assert!(render_camera.target[0] > render_camera.eye[0] + 100.0);
}

#[test]
fn sniper_scroll_zooms_fov_without_changing_third_person_distance() {
    let subject = CameraSubject::from_snapshot(tank_snapshot([0.0, 0.0, 0.0], 0.0, 0.0), 0.0);
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);
    let distance = camera.distance_m();
    let baseline = camera.render_camera(&subject, &BattleCameraEnvironment::empty());

    camera.apply_input(BattleCameraInput {
        orbit_yaw_delta_rad: 0.0,
        pitch_delta_rad: 0.0,
        zoom_delta_m: -2.0,
    });
    let zoomed = camera.render_camera(&subject, &BattleCameraEnvironment::empty());

    assert!(zoomed.vertical_fov_degrees < baseline.vertical_fov_degrees);
    assert_eq!(camera.distance_m(), distance);
}

#[test]
fn wheel_sweeps_one_zoom_axis_from_long_boom_to_maximum_magnification() {
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    let zoom = |delta: f32| BattleCameraInput {
        orbit_yaw_delta_rad: 0.0,
        pitch_delta_rad: 0.0,
        zoom_delta_m: delta,
    };

    // Scrolling in shortens the boom, then hands over to sniper and steps the magnification.
    for _ in 0..20 {
        camera.apply_input(zoom(-0.8));
    }
    assert_eq!(camera.mode(), BattleCameraMode::Sniper);
    assert_eq!(camera.sniper_fov_degrees(), 3.0, "kept scrolling to the narrowest step");

    // Scrolling out steps back through the ladder and exits to the shortest boom.
    for _ in 0..5 {
        camera.apply_input(zoom(0.8));
    }
    assert_eq!(camera.mode(), BattleCameraMode::ThirdPerson);
    assert_eq!(camera.distance_m(), camera.settings().min_distance_m);
}

#[test]
fn sniper_zoom_uses_discrete_magnification_steps() {
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    camera.set_mode(BattleCameraMode::Sniper);
    assert_eq!(camera.sniper_fov_degrees(), 8.0);

    camera.apply_input(BattleCameraInput {
        orbit_yaw_delta_rad: 0.0,
        pitch_delta_rad: 0.0,
        zoom_delta_m: -0.8,
    });

    assert_eq!(camera.sniper_fov_degrees(), 5.0, "one click moves exactly one ladder step");
}

#[test]
fn look_sensitivity_scales_with_sniper_fov_and_stays_unit_in_third_person() {
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());
    assert_eq!(camera.look_sensitivity_scale(), 1.0);

    camera.set_mode(BattleCameraMode::Sniper);
    let expected = camera.sniper_fov_degrees() / camera.settings().third_person_fov_degrees;
    assert!((camera.look_sensitivity_scale() - expected).abs() < 1.0e-6);
    assert!(camera.look_sensitivity_scale() < 0.2, "8 deg of 62 deg must slow the look hard");
}

#[test]
fn camera_input_clamps_pitch_and_zoom_without_touching_simulation() {
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());

    camera.apply_input(BattleCameraInput {
        orbit_yaw_delta_rad: 0.25,
        pitch_delta_rad: 10.0,
        zoom_delta_m: 100.0,
    });

    assert_eq!(camera.pitch_rad(), camera.settings().max_pitch_rad);
    assert_eq!(camera.distance_m(), camera.settings().max_distance_m);

    camera.set_mode(BattleCameraMode::Sniper);

    assert_eq!(camera.mode(), BattleCameraMode::Sniper);
}

#[test]
fn camera_orbit_yaw_wraps_into_pi_range() {
    use std::f32::consts::PI;
    let mut camera = BattleCameraController::new(BattleCameraSettings::default());

    // Seven half-turns must not accumulate unbounded; the angle stays wrapped.
    for _ in 0..7 {
        camera.apply_input(BattleCameraInput {
            orbit_yaw_delta_rad: PI,
            pitch_delta_rad: 0.0,
            zoom_delta_m: 0.0,
        });
    }

    assert!(camera.orbit_yaw_rad() > -PI - 1e-4 && camera.orbit_yaw_rad() <= PI + 1e-4);
}

fn tank_snapshot(position: [f32; 3], hull_yaw_rad: f32, turret_yaw_rad: f32) -> TankSnapshot {
    let spec = game_core::VehicleKind::PrototypeMedium.spec();
    TankSnapshot {
        tank_id: TankId(1),
        team: game_core::TeamId(1),
        vehicle: spec.kind,
        position,
        yaw_rad: hull_yaw_rad,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
        module_hit_points: spec.module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    }
}
