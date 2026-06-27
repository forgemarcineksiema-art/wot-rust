//! Negative contact tests for tank-vs-cover collision, per the engineering rule that every
//! contact-shape approximation needs a locked scenario that must NOT happen. The pre-fix
//! blocker tested the hull as a 1.6 m point-radius, so a T-55A buried its nose 1.6 m (a
//! Jagdtiger 2.5 m) inside buildings, and hulls wider than 1.6 m clipped through wall corners.

use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, resolve_cover_collision, step_tank_on_world_with_tanks,
};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

fn cover_box(center: [f32; 3], half_extents_m: [f32; 3]) -> StaticCoverObject {
    StaticCoverObject {
        id: "wall".to_string(),
        name: "wall".to_string(),
        kind: StaticCoverKind::FarmBuilding,
        center,
        half_extents_m,
    }
}

/// Drive a hull with the given footprint straight at a wall and return its final center z.
fn drive_into_wall(footprint: TankFootprint) -> (f32, f32) {
    let heightmap = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain");
    let wall_face_z = 56.0;
    let wall = cover_box([40.0, 1.5, 60.0], [5.0, 2.5, 4.0]);
    let settings = TankControllerSettings::arcade_default();
    let mut state = TankKinematicState {
        position: Vec3::new(40.0, 0.0, 40.0),
        ..TankKinematicState::default()
    };
    for _ in 0..240 {
        step_tank_on_world_with_tanks(
            &mut state,
            TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 },
            &settings,
            Some(&heightmap),
            TankWorldObstacles::new(std::slice::from_ref(&wall), footprint, &[]),
            1.0 / 60.0,
        );
    }
    (state.position.z, wall_face_z)
}

#[test]
fn hull_front_never_interpenetrates_cover_head_on() {
    for (name, footprint) in [
        ("T-55A", TankFootprint { half_width_m: 1.75, half_length_m: 3.20 }),
        ("Jagdtiger", TankFootprint { half_width_m: 2.00, half_length_m: 4.10 }),
    ] {
        let (center_z, wall_face_z) = drive_into_wall(footprint);
        let hull_front_z = center_z + footprint.half_length_m;
        assert!(
            hull_front_z <= wall_face_z + 1.0e-2,
            "{name} nose must stop at the wall face, got front z {hull_front_z} vs face {wall_face_z}"
        );
        assert!(
            hull_front_z > wall_face_z - 1.0,
            "{name} should still drive up to the wall, got front z {hull_front_z}"
        );
    }
}

#[test]
fn wide_hull_is_blocked_where_the_old_point_radius_let_it_pass() {
    // Wall flank at x = 1.8: a 2.0 m half-width hull overlaps it, but the old 1.6 m point-radius
    // test did not (1.8 - 1.6 = 0.2 m of imagined clearance).
    let cover = vec![cover_box([3.8, 1.0, 17.0], [2.0, 2.0, 2.0])];
    let previous = Vec3::new(0.0, 0.0, 10.0);
    let attempted = Vec3::new(0.0, 0.0, 11.0);

    let wide = TankFootprint { half_width_m: 2.0, half_length_m: 4.1 };
    let resolved = resolve_cover_collision(previous, attempted, 0.0, wide, &cover);
    assert!(
        (resolved.z - previous.z).abs() < 1.0e-6,
        "a 2.0 m half-width hull must be blocked by the wall corner, got z {}",
        resolved.z
    );

    // The same move with a hull narrow enough to clear the corner must stay unblocked, proving
    // the block above comes from the real footprint and not from an inflated test.
    let narrow = TankFootprint { half_width_m: 1.6, half_length_m: 4.1 };
    let resolved = resolve_cover_collision(previous, attempted, 0.0, narrow, &cover);
    assert_eq!(resolved, attempted, "a narrow hull clears the corner");
}

#[test]
fn near_miss_along_a_wall_is_never_blocked() {
    // Driving parallel to a wall with 0.3 m of real clearance must keep the full move: the
    // footprint must be the hull, not the hull plus an invisible margin.
    let cover = vec![cover_box([4.05, 1.0, 30.0], [2.0, 2.0, 10.0])];
    let footprint = TankFootprint { half_width_m: 1.75, half_length_m: 3.20 };

    let mut position = Vec3::new(0.0, 0.0, 10.0);
    for _ in 0..100 {
        let attempted = position + Vec3::new(0.0, 0.0, 0.25);
        let resolved = resolve_cover_collision(position, attempted, 0.0, footprint, &cover);
        assert_eq!(resolved, attempted, "a clean pass along the wall must not be blocked");
        position = resolved;
    }
    assert!(position.z > 30.0, "the hull should have passed the whole wall");
}
