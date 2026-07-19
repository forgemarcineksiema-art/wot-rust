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

    assert!(state.forward_speed() > 0.0);
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
fn the_red_line_holds_a_tank_inside_the_map_and_lets_it_slide_along() {
    let map = prokhorovka_hill_252_2();
    let settings = TankControllerSettings::arcade_default();
    let [_, extent_z] = map.heightmap.extent_m();
    let line = extent_z - physics::MAP_BORDER_MARGIN_M;

    // Full throttle straight at the northern border for 20 s. Before the red line the stepper
    // fell into terrain-free mode past the heightmap and the tank drove off the world onto the
    // render-only backdrop.
    let mut state = TankKinematicState {
        position: glam::Vec3::new(220.0, 0.0, extent_z - 40.0),
        ..TankKinematicState::default()
    };
    let head_on = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    for _ in 0..1200 {
        step_tank_on_heightmap(&mut state, head_on, &settings, &map.heightmap, 1.0 / 60.0);
    }
    assert!(
        state.position.z <= line + 1.0e-3,
        "the hull must hold at the red line, got z={}",
        state.position.z
    );
    assert!(
        map.heightmap.sample_height(state.position.x, state.position.z).is_some(),
        "the hull must never leave the heightmap"
    );
    assert!(
        state.velocity.z.abs() < 0.25,
        "speed into the wall must die at the line, got {} m/s",
        state.velocity.z
    );

    // Held against the wall at a shallow angle (heading mostly ALONG the line, leaning into
    // it), the along-wall drive keeps working: the line is a wall to drive along, not glue.
    // A hull pointed near-square INTO the wall grinds instead — the lateral grip scrubs
    // sideways slip exactly as it does everywhere else, which is the correct track feel.
    let mut slider = TankKinematicState {
        position: glam::Vec3::new(220.0, 0.0, line),
        yaw_rad: 1.2,
        ..TankKinematicState::default()
    };
    let start_x = slider.position.x;
    for _ in 0..600 {
        step_tank_on_heightmap(&mut slider, head_on, &settings, &map.heightmap, 1.0 / 60.0);
    }
    assert!(
        (slider.position.x - start_x).abs() > 30.0,
        "a shallow-angle hull must keep sliding along the line, moved {} m",
        (slider.position.x - start_x).abs()
    );
    assert!(slider.position.z <= line + 1.0e-3, "sliding must not cross the line");
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
    // Staged inside the red-line margin: the scenario tests cover blocking, not the border.
    let cover = vec![cover_box([15.0, 1.0, 12.0], [5.0, 2.0, 1.0])];
    let mut state = TankKinematicState {
        position: glam::Vec3::new(15.0, 0.0, 9.3),
        yaw_rad: 0.0,
        // Charging the cover head-on at 12 m/s (+z is forward at yaw 0).
        velocity: glam::Vec3::new(0.0, 0.0, 12.0),
        ..TankKinematicState::default()
    };

    step_tank_on_world(
        &mut state,
        TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 },
        &settings,
        &heightmap,
        &cover,
        1.0 / 60.0,
    );

    assert_eq!(state.position, glam::Vec3::new(15.0, 0.0, 9.3));
    assert!(
        state.forward_speed().abs() < 0.01,
        "blocked hull must not keep phantom forward speed, got {}",
        state.forward_speed()
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

/// Two hulls can END UP interpenetrated (a pivot swings the unresolved yaw footprint into a
/// neighbor; two hulls step into the same space on one tick). From that state the resolver must
/// let a tank back OUT — rejecting every move deadlocked both tanks for the rest of the battle —
/// while still refusing moves that grind deeper in.
#[test]
fn interpenetrating_hulls_can_back_out_but_not_dig_deeper() {
    let footprint = TankFootprint { half_width_m: 1.6, half_length_m: 3.2 };
    // Overlapped start: centres 4 m apart along z, combined half-lengths 6.4 m.
    let obstacles = [TankObstacle::new(glam::Vec3::new(0.0, 0.0, 4.0), 0.0, footprint)];
    let previous = glam::Vec3::ZERO;

    let back_out = resolve_tank_collision(
        previous,
        glam::Vec3::new(0.0, 0.0, -0.3),
        0.0,
        footprint,
        &obstacles,
    );
    assert_eq!(back_out, glam::Vec3::new(0.0, 0.0, -0.3), "backing out must be allowed");

    let deeper = resolve_tank_collision(
        previous,
        glam::Vec3::new(0.0, 0.0, 0.3),
        0.0,
        footprint,
        &obstacles,
    );
    assert_eq!(deeper, previous, "digging deeper must stay blocked");

    // Repeated back-out ticks fully separate the hulls, after which normal blocking resumes.
    let mut position = previous;
    for _ in 0..40 {
        position = resolve_tank_collision(
            position,
            position - glam::Vec3::Z * 0.3,
            0.0,
            footprint,
            &obstacles,
        );
    }
    assert!(position.z < -2.4, "the hull must escape the overlap, got z {}", position.z);
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
