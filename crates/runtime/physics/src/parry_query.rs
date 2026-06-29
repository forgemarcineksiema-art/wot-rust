use parry3d::math::{Pose, Rotation, Vector};
use parry3d::query;
use parry3d::shape::Cuboid;

use crate::TankObstacle;

pub fn tank_footprints_intersect_query(a: &TankObstacle, b: &TankObstacle, slop_m: f32) -> bool {
    let inflated = footprint_shape(a, slop_m.max(0.0));
    let other = footprint_shape(b, 0.0);
    query::intersection_test(&footprint_pose(a), &inflated, &footprint_pose(b), &other)
        .unwrap_or(false)
}

fn footprint_shape(obstacle: &TankObstacle, inflate_m: f32) -> Cuboid {
    Cuboid::new(Vector::new(
        obstacle.footprint.half_width_m + inflate_m,
        0.5,
        obstacle.footprint.half_length_m + inflate_m,
    ))
}

fn footprint_pose(obstacle: &TankObstacle) -> Pose {
    Pose::from_parts(
        Vector::new(obstacle.center.x, obstacle.center.y, obstacle.center.z),
        Rotation::from_rotation_y(obstacle.yaw_rad),
    )
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::TankFootprint;

    #[test]
    fn parry_query_uses_project_footprint_types() {
        let footprint = TankFootprint { half_width_m: 1.0, half_length_m: 2.0 };
        let a = TankObstacle::new(Vec3::ZERO, 0.0, footprint);
        let b = TankObstacle::new(Vec3::new(1.5, 0.0, 0.0), 0.0, footprint);
        let c = TankObstacle::new(Vec3::new(5.0, 0.0, 0.0), 0.0, footprint);

        assert!(tank_footprints_intersect_query(&a, &b, 0.0));
        assert!(!tank_footprints_intersect_query(&a, &c, 0.0));
    }
}
