//! Deterministic narrow-phase intersections for authored damage primitives.

use glam::{Mat3, Vec3};

use super::{DamagePlane, DamageShape};

impl DamageShape {
    pub(super) fn contains(&self, point: Vec3) -> bool {
        self.segment_interval(point, point).is_some()
    }

    pub(super) fn segment_interval(&self, start: Vec3, end: Vec3) -> Option<(f32, f32)> {
        match self {
            Self::Obb { center, half_extents, yaw_rad } => {
                let inverse = Mat3::from_rotation_y(-yaw_rad);
                segment_aabb(
                    inverse * (start - *center),
                    inverse * (end - *center),
                    -*half_extents,
                    *half_extents,
                )
            }
            Self::Cylinder { center, axis, half_length, radius } => {
                segment_cylinder(start - *center, end - *center, *axis, *half_length, *radius)
            }
            Self::Capsule { a, b, radius } => segment_capsule(start, end, *a, *b, *radius),
            Self::Convex { planes, .. } => segment_convex(start, end, planes),
        }
    }

    pub(super) fn bounds(&self) -> (Vec3, Vec3) {
        match self {
            Self::Obb { center, half_extents, yaw_rad } => {
                let r = Mat3::from_rotation_y(*yaw_rad);
                let extent = r.x_axis.abs() * half_extents.x
                    + r.y_axis.abs() * half_extents.y
                    + r.z_axis.abs() * half_extents.z;
                (*center - extent, *center + extent)
            }
            Self::Cylinder { center, axis, half_length, radius } => {
                let extent = axis.normalize_or_zero().abs() * *half_length + Vec3::splat(*radius);
                (*center - extent, *center + extent)
            }
            Self::Capsule { a, b, radius } => {
                (a.min(*b) - Vec3::splat(*radius), a.max(*b) + Vec3::splat(*radius))
            }
            Self::Convex { bounds_min, bounds_max, .. } => (*bounds_min, *bounds_max),
        }
    }
}

fn segment_aabb(start: Vec3, end: Vec3, min: Vec3, max: Vec3) -> Option<(f32, f32)> {
    let delta = end - start;
    let (mut enter, mut exit) = (0.0_f32, 1.0_f32);
    for axis in 0..3 {
        let (origin, direction) = (start[axis], delta[axis]);
        if direction.abs() < 1.0e-7 {
            if origin < min[axis] || origin > max[axis] {
                return None;
            }
            continue;
        }
        let a = (min[axis] - origin) / direction;
        let b = (max[axis] - origin) / direction;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return None;
        }
    }
    Some((enter, exit))
}

fn segment_convex(start: Vec3, end: Vec3, planes: &[DamagePlane]) -> Option<(f32, f32)> {
    let delta = end - start;
    let (mut enter, mut exit) = (0.0_f32, 1.0_f32);
    for plane in planes {
        let origin_distance = plane.normal.dot(start) - plane.offset;
        let speed = plane.normal.dot(delta);
        if speed.abs() < 1.0e-7 {
            if origin_distance > 0.0 {
                return None;
            }
            continue;
        }
        let t = -origin_distance / speed;
        if speed < 0.0 {
            enter = enter.max(t);
        } else {
            exit = exit.min(t);
        }
        if enter > exit {
            return None;
        }
    }
    (exit >= 0.0 && enter <= 1.0).then_some((enter.max(0.0), exit.min(1.0)))
}

fn segment_cylinder(
    start: Vec3,
    end: Vec3,
    axis: Vec3,
    half_length: f32,
    radius: f32,
) -> Option<(f32, f32)> {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() < 0.5 {
        return None;
    }
    let delta = end - start;
    let axial_start = start.dot(axis);
    let axial_delta = delta.dot(axis);
    let (mut enter, mut exit) = if axial_delta.abs() < 1.0e-7 {
        if axial_start.abs() > half_length {
            return None;
        }
        (0.0, 1.0)
    } else {
        let a = (-half_length - axial_start) / axial_delta;
        let b = (half_length - axial_start) / axial_delta;
        (a.min(b).max(0.0), a.max(b).min(1.0))
    };
    let radial_start = start - axis * axial_start;
    let radial_delta = delta - axis * axial_delta;
    let (a, b, c) = (
        radial_delta.length_squared(),
        2.0 * radial_start.dot(radial_delta),
        radial_start.length_squared() - radius * radius,
    );
    if a < 1.0e-9 {
        if c > 0.0 {
            return None;
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }
        let root = discriminant.sqrt();
        let (r0, r1) = ((-b - root) / (2.0 * a), (-b + root) / (2.0 * a));
        enter = enter.max(r0.min(r1));
        exit = exit.min(r0.max(r1));
    }
    (enter <= exit && exit >= 0.0 && enter <= 1.0).then_some((enter.max(0.0), exit.min(1.0)))
}

fn segment_capsule(start: Vec3, end: Vec3, a: Vec3, b: Vec3, radius: f32) -> Option<(f32, f32)> {
    let inside = |t: f32| {
        let p = start.lerp(end, t);
        let ab = b - a;
        let u =
            if ab.length_squared() > 1.0e-9 { (p - a).dot(ab) / ab.length_squared() } else { 0.0 };
        p.distance_squared(a + ab * u.clamp(0.0, 1.0)) <= radius * radius
    };
    let (mut first, mut last, mut previous_t) = (None, None, 0.0);
    let mut previous_inside = inside(0.0);
    if previous_inside {
        first = Some(0.0);
    }
    for step in 1..=64 {
        let t = step as f32 / 64.0;
        let current_inside = inside(t);
        if current_inside != previous_inside {
            let (mut lo, mut hi) = (previous_t, t);
            for _ in 0..12 {
                let mid = (lo + hi) * 0.5;
                if inside(mid) == previous_inside { lo = mid } else { hi = mid }
            }
            let crossing = (lo + hi) * 0.5;
            if current_inside {
                first.get_or_insert(crossing);
            } else {
                last = Some(crossing);
            }
        }
        (previous_t, previous_inside) = (t, current_inside);
    }
    if previous_inside {
        last = Some(1.0);
    }
    first.zip(last)
}
