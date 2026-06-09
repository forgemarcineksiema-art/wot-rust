use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState, TankObstacle,
    make_tank_hull_collider, make_terrain_heightfield_collider, resolve_tank_collision,
    step_custom_tank_controller, step_tank_on_heightmap, step_tank_on_world,
};
use terrain::prokhorovka_hill_252_2;

#[test]
fn custom_tank_controller_accelerates_and_turns() {
    let settings = TankControllerSettings::arcade_default();
    let mut state = TankKinematicState::default();

    step_custom_tank_controller(
        &mut state,
        TankControlInput { throttle: 1.0, steer: 0.5, brake: 0.0 },
        &settings,
        0.1,
    );

    assert!(state.forward_speed_mps > 0.0);
    assert!(state.yaw_rad > 0.0);
}

#[test]
fn rapier_hull_collider_can_be_created_for_tank_dimensions() {
    let _collider = make_tank_hull_collider([2.0, 1.0, 4.0]);
}

#[test]
fn historical_map_heightfield_collider_spans_full_world_extent() {
    let map = prokhorovka_hill_252_2();
    let [extent_x, extent_z] = map.heightmap.extent_m();
    assert!(extent_x > 900.0 && extent_z > 900.0, "sanity: Prokhorovka is ~1000m");

    let collider = make_terrain_heightfield_collider(&map.heightmap);
    let aabb = collider.compute_aabb();
    let span_x = aabb.maxs.x - aabb.mins.x;
    let span_z = aabb.maxs.z - aabb.mins.z;

    // Must cover the whole map, not collapse to a per-cell (~5m) patch.
    assert!((span_x - extent_x).abs() < 1.0, "x span {span_x} should match extent {extent_x}");
    assert!((span_z - extent_z).abs() < 1.0, "z span {span_z} should match extent {extent_z}");
    // And align to the corner-origin [0, extent] frame the sampler uses.
    assert!(aabb.mins.x > -1.0 && aabb.mins.z > -1.0, "collider should start near origin");
    assert!(aabb.maxs.x > extent_x - 1.0, "collider should reach the far edge");
}

#[test]
fn tank_controller_can_drive_over_historical_heightmap() {
    let map = prokhorovka_hill_252_2();
    let settings = TankControllerSettings::arcade_default();
    let mut state = TankKinematicState {
        position: glam::Vec3::new(115.0, 0.0, 120.0),
        ..TankKinematicState::default()
    };

    step_tank_on_heightmap(
        &mut state,
        TankControlInput { throttle: 1.0, steer: 0.15, brake: 0.0 },
        &settings,
        &map.heightmap,
        0.2,
    );

    let terrain_y = map
        .heightmap
        .sample_height(state.position.x, state.position.z)
        .expect("tank should remain inside playable map");
    assert!((state.position.y - terrain_y).abs() < 0.01);
}

#[test]
fn embankment_blocks_movement_except_at_crossings() {
    let map = prokhorovka_hill_252_2();
    let settings = TankControllerSettings::arcade_default();

    // Drive north from south of the central axis, straight at the embankment. Off a crossing
    // the solid railbed must stop the tank short of the axis; at the central crossing (x=500)
    // the railbed opens and the tank passes through.
    let blocked_z = drive_north_into_embankment(&map.heightmap, &settings, 375.0);
    let crossing_z = drive_north_into_embankment(&map.heightmap, &settings, 500.0);

    assert!(
        blocked_z < 498.0,
        "solid embankment must stop the tank south of the axis (reached z={blocked_z})"
    );
    assert!(
        crossing_z > 510.0,
        "the crossing must let the tank pass the axis (reached z={crossing_z})"
    );
}

fn drive_north_into_embankment(
    heightmap: &terrain::HeightMap,
    settings: &TankControllerSettings,
    x: f32,
) -> f32 {
    let mut state = TankKinematicState {
        position: glam::Vec3::new(x, 0.0, 455.0),
        ..TankKinematicState::default()
    };
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    for _ in 0..600 {
        step_tank_on_heightmap(&mut state, input, settings, heightmap, 1.0 / 60.0);
    }
    state.position.z
}

#[test]
fn cover_collision_blocks_head_on_and_keeps_the_unblocked_axis() {
    let cover = vec![cover_box([0.0, 1.0, 10.0], [6.0, 2.0, 1.0])];
    let footprint = TankFootprint { half_width_m: 1.6, half_length_m: 1.6 };
    let previous = glam::Vec3::new(0.0, 0.0, 1.0);

    // A clear move that never reaches the cover is unchanged.
    let clear = physics::resolve_cover_collision(
        previous,
        glam::Vec3::new(0.0, 0.0, 3.0),
        0.0,
        footprint,
        &cover,
    );
    assert_eq!(clear, glam::Vec3::new(0.0, 0.0, 3.0));

    // Driving straight into the cover is fully blocked: the hull holds its previous position.
    let head_on = physics::resolve_cover_collision(
        previous,
        glam::Vec3::new(0.0, 0.0, 9.5),
        0.0,
        footprint,
        &cover,
    );
    assert_eq!(head_on, previous);

    // Moving sideways (x) while pressing into the cover (z): x is kept, z is dropped (slide).
    let slide = physics::resolve_cover_collision(
        previous,
        glam::Vec3::new(5.0, 0.0, 9.5),
        0.0,
        footprint,
        &cover,
    );
    assert!((slide.x - 5.0).abs() < 1.0e-6, "x slide preserved, got {}", slide.x);
    assert!((slide.z - previous.z).abs() < 1.0e-6, "z into cover blocked, got {}", slide.z);
}

#[test]
fn cover_collision_cancels_forward_speed_when_hull_is_fully_blocked() {
    let heightmap = terrain::HeightMap::flat(32, 32, 1.0, 0.0).expect("flat terrain");
    let settings = TankControllerSettings::arcade_default();
    let cover = vec![cover_box([0.0, 1.0, 8.0], [5.0, 2.0, 1.0])];
    let mut state = TankKinematicState {
        position: glam::Vec3::new(0.0, 0.0, 5.3),
        yaw_rad: 0.0,
        forward_speed_mps: 12.0,
    };

    step_tank_on_world(
        &mut state,
        TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 },
        &settings,
        &heightmap,
        &cover,
        1.0 / 60.0,
    );

    assert_eq!(state.position, glam::Vec3::new(0.0, 0.0, 5.3));
    assert!(
        state.forward_speed_mps.abs() < 0.01,
        "blocked hull must not keep phantom forward speed, got {}",
        state.forward_speed_mps
    );
}

#[test]
fn tank_collision_blocks_head_on_and_keeps_the_unblocked_axis() {
    let footprint = TankFootprint { half_width_m: 1.6, half_length_m: 3.2 };
    let obstacles = [TankObstacle::new(glam::Vec3::new(0.0, 0.0, 8.0), 0.0, footprint)];
    let previous = glam::Vec3::new(0.0, 0.0, 1.0);

    let clear = resolve_tank_collision(
        previous,
        glam::Vec3::new(0.0, 0.0, 1.2),
        0.0,
        footprint,
        &obstacles,
    );
    assert_eq!(clear, glam::Vec3::new(0.0, 0.0, 1.2));

    let head_on = resolve_tank_collision(
        previous,
        glam::Vec3::new(0.0, 0.0, 4.9),
        0.0,
        footprint,
        &obstacles,
    );
    assert_eq!(head_on, previous);

    let slide = resolve_tank_collision(
        previous,
        glam::Vec3::new(2.0, 0.0, 4.9),
        0.0,
        footprint,
        &obstacles,
    );
    assert!((slide.x - 2.0).abs() < 1.0e-6, "x slide preserved, got {}", slide.x);
    assert!((slide.z - previous.z).abs() < 1.0e-6, "z into tank blocked, got {}", slide.z);
}

fn cover_box(center: [f32; 3], half_extents_m: [f32; 3]) -> terrain::StaticCoverObject {
    terrain::StaticCoverObject {
        id: "wall".to_string(),
        name: "wall".to_string(),
        kind: terrain::StaticCoverKind::FarmBuilding,
        center,
        half_extents_m,
    }
}
