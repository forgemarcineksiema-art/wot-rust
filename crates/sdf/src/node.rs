//! The CSG node tree and its signed-distance evaluation.
//!
//! Hard ops (`Union`/`Intersect`/`Subtract`) keep edges crisp — the basis for sharp, angled armour
//! plates. Smooth ops blend over a radius for cast surfaces. `Rigid` transforms are rotate+translate
//! only, so they preserve the distance metric and the field stays a true SDF (no scale skew).

use glam::{Quat, Vec2, Vec3};

/// A constructive signed-distance node. Build these with the constructors/combinators in `shape.rs`.
#[derive(Debug, Clone, PartialEq)]
pub enum Sdf {
    Sphere {
        radius: f32,
    },
    Cuboid {
        half: Vec3,
    },
    /// A flat half-space: everything on the `-normal` side is inside. `normal` is unit; the surface
    /// is the exact plane `dot(p, normal) = offset`, so a plate's slope is exact and recoverable.
    HalfSpace {
        normal: Vec3,
        offset: f32,
    },
    /// A capped cylinder about the local +Y axis.
    Cylinder {
        radius: f32,
        half_height: f32,
    },
    Rigid {
        rotation: Quat,
        translation: Vec3,
        node: Box<Sdf>,
    },
    Union(Box<Sdf>, Box<Sdf>),
    Intersect(Box<Sdf>, Box<Sdf>),
    Subtract(Box<Sdf>, Box<Sdf>),
    SmoothUnion {
        a: Box<Sdf>,
        b: Box<Sdf>,
        radius: f32,
    },
    SmoothSubtract {
        a: Box<Sdf>,
        b: Box<Sdf>,
        radius: f32,
    },
}

/// A field that answers both "how far to the surface" and "which way is out". The gradient points
/// away from the surface (the outward normal direction) and is unit length where the field is a true
/// distance metric.
pub trait SignedDistance {
    fn eval(&self, point: Vec3) -> f32;
    fn gradient(&self, point: Vec3) -> Vec3;
}

impl SignedDistance for Sdf {
    fn eval(&self, point: Vec3) -> f32 {
        Sdf::eval(self, point)
    }

    fn gradient(&self, point: Vec3) -> Vec3 {
        Sdf::gradient(self, point)
    }
}

impl Sdf {
    /// Signed distance from `p` to the surface: negative inside, positive outside, zero on it.
    pub fn eval(&self, p: Vec3) -> f32 {
        match self {
            Sdf::Sphere { radius } => p.length() - radius,
            Sdf::Cuboid { half } => cuboid(p, *half),
            Sdf::HalfSpace { normal, offset } => p.dot(*normal) - offset,
            Sdf::Cylinder { radius, half_height } => cylinder(p, *radius, *half_height),
            // A rigid transform places the child: undo it on the query point (conjugate == inverse
            // for a unit quaternion) before evaluating the child in its own local space.
            Sdf::Rigid { rotation, translation, node } => {
                node.eval(rotation.conjugate() * (p - *translation))
            }
            Sdf::Union(a, b) => a.eval(p).min(b.eval(p)),
            Sdf::Intersect(a, b) => a.eval(p).max(b.eval(p)),
            Sdf::Subtract(a, b) => a.eval(p).max(-b.eval(p)),
            Sdf::SmoothUnion { a, b, radius } => smin(a.eval(p), b.eval(p), *radius),
            // Smooth subtract = smooth max(a, -b) = -smin(-a, b, k).
            Sdf::SmoothSubtract { a, b, radius } => -smin(-a.eval(p), b.eval(p), *radius),
        }
    }

    /// The outward gradient at `p`. Analytic for the primitives, rigid transforms and the hard
    /// booleans (which select a branch deterministically); smooth blends use the same `smin` weight
    /// to mix the child gradients. A central finite-difference fallback covers any point where the
    /// analytic direction collapses (e.g. an exact corner or the very centre of a sphere).
    pub fn gradient(&self, p: Vec3) -> Vec3 {
        let analytic = match self {
            Sdf::Sphere { .. } => p,
            Sdf::HalfSpace { normal, .. } => *normal,
            Sdf::Cuboid { half } => cuboid_gradient(p, *half),
            Sdf::Cylinder { radius, half_height } => cylinder_gradient(p, *radius, *half_height),
            Sdf::Rigid { rotation, translation, node } => {
                *rotation * node.gradient(rotation.conjugate() * (p - *translation))
            }
            // Hard booleans take the gradient of whichever branch is the active surface.
            Sdf::Union(a, b) => active_branch(a, b, p, a.eval(p) <= b.eval(p)),
            Sdf::Intersect(a, b) => active_branch(a, b, p, a.eval(p) >= b.eval(p)),
            Sdf::Subtract(a, b) => {
                if a.eval(p) >= -b.eval(p) {
                    a.gradient(p)
                } else {
                    -b.gradient(p)
                }
            }
            Sdf::SmoothUnion { a, b, radius } => smin_gradient(a, b, *radius, p),
            Sdf::SmoothSubtract { a, b, radius } => {
                // d/dp[-smin(-a, b)] mixes the gradient of -a and of b by the same weight.
                let h = smin_weight(-a.eval(p), b.eval(p), *radius);
                (-a.gradient(p)) * h + b.gradient(p) * (1.0 - h)
            }
        };
        let g = analytic.normalize_or_zero();
        if g == Vec3::ZERO { finite_difference_gradient(self, p) } else { g }
    }
}

fn active_branch(a: &Sdf, b: &Sdf, p: Vec3, take_a: bool) -> Vec3 {
    if take_a { a.gradient(p) } else { b.gradient(p) }
}

/// Outward gradient of an axis-aligned box: the direction out of the nearest face (or corner).
fn cuboid_gradient(p: Vec3, half: Vec3) -> Vec3 {
    let q = p.abs() - half;
    if q.max_element() > 0.0 {
        // Outside: the gradient is the direction of the positive overshoot, signed back to `p`.
        q.max(Vec3::ZERO) * p.signum()
    } else {
        // Inside: out through the closest face — the axis with the least-negative slab distance.
        let axis = if q.x >= q.y && q.x >= q.z {
            Vec3::X
        } else if q.y >= q.z {
            Vec3::Y
        } else {
            Vec3::Z
        };
        axis * p.signum()
    }
}

/// Outward gradient of a Y-axis capped cylinder, blended in the corner region.
fn cylinder_gradient(p: Vec3, radius: f32, half_height: f32) -> Vec3 {
    let radial_dir = Vec3::new(p.x, 0.0, p.z).normalize_or_zero();
    let radial = Vec2::new(p.x, p.z).length() - radius;
    let axial = p.y.abs() - half_height;
    let axial_dir = Vec3::new(0.0, p.y.signum(), 0.0);
    if radial > 0.0 && axial > 0.0 {
        (radial_dir * radial + axial_dir * axial).normalize_or_zero()
    } else if radial > axial {
        radial_dir
    } else {
        axial_dir
    }
}

/// The `smin` blend weight: 1 means branch `a` dominates, 0 means branch `b`.
fn smin_weight(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return if a <= b { 1.0 } else { 0.0 };
    }
    (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0)
}

fn smin_gradient(a: &Sdf, b: &Sdf, k: f32, p: Vec3) -> Vec3 {
    let h = smin_weight(a.eval(p), b.eval(p), k);
    a.gradient(p) * h + b.gradient(p) * (1.0 - h)
}

/// Central-difference gradient, the robust fallback where the analytic direction is undefined.
fn finite_difference_gradient(sdf: &Sdf, p: Vec3) -> Vec3 {
    const H: f32 = 1.0e-3;
    let dx = sdf.eval(p + Vec3::X * H) - sdf.eval(p - Vec3::X * H);
    let dy = sdf.eval(p + Vec3::Y * H) - sdf.eval(p - Vec3::Y * H);
    let dz = sdf.eval(p + Vec3::Z * H) - sdf.eval(p - Vec3::Z * H);
    Vec3::new(dx, dy, dz).normalize_or_zero()
}

/// Exact distance to an axis-aligned box of the given half-extents (Inigo Quilez's form).
fn cuboid(p: Vec3, half: Vec3) -> f32 {
    let q = p.abs() - half;
    q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
}

/// Exact distance to a Y-axis capped cylinder.
fn cylinder(p: Vec3, radius: f32, half_height: f32) -> f32 {
    let radial = Vec2::new(p.x, p.z).length() - radius;
    let axial = p.y.abs() - half_height;
    let outside = Vec2::new(radial.max(0.0), axial.max(0.0)).length();
    radial.max(axial).min(0.0) + outside
}

/// Polynomial smooth-minimum (Inigo Quilez). `k <= 0` degenerates to a hard `min`, so a smooth
/// blend with zero radius is exactly the hard union — the property the tests lock.
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    (b * (1.0 - h) + a * h) - k * h * (1.0 - h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1.0e-5;

    #[test]
    fn sphere_is_signed_about_its_radius() {
        let s = Sdf::Sphere { radius: 2.0 };
        assert!((s.eval(Vec3::ZERO) + 2.0).abs() < EPS, "centre is -radius");
        assert!(s.eval(Vec3::new(2.0, 0.0, 0.0)).abs() < EPS, "surface is zero");
        assert!((s.eval(Vec3::new(5.0, 0.0, 0.0)) - 3.0).abs() < EPS, "outside is distance");
    }

    #[test]
    fn half_space_slope_is_exact_and_recoverable() {
        // A 60-degree glacis: the plate normal encodes the armour slope directly.
        let normal = Vec3::new(0.0, (60.0_f32).to_radians().sin(), (60.0_f32).to_radians().cos());
        let plate = Sdf::HalfSpace { normal: normal.normalize(), offset: 0.0 };
        // A point one metre along the normal is exactly one metre outside.
        assert!((plate.eval(normal.normalize()) - 1.0).abs() < EPS);
        assert!((plate.eval(-normal.normalize()) + 1.0).abs() < EPS);
    }

    #[test]
    fn cuboid_is_negative_inside_zero_on_face_positive_outside() {
        let b = Sdf::Cuboid { half: Vec3::new(1.0, 1.0, 1.0) };
        assert!((b.eval(Vec3::ZERO) + 1.0).abs() < EPS, "centre is -half");
        assert!(b.eval(Vec3::new(1.0, 0.0, 0.0)).abs() < EPS, "face is zero");
        assert!((b.eval(Vec3::new(2.0, 0.0, 0.0)) - 1.0).abs() < EPS, "outside is distance");
    }

    #[test]
    fn cylinder_is_capped_and_round() {
        let c = Sdf::Cylinder { radius: 1.0, half_height: 2.0 };
        assert!(c.eval(Vec3::ZERO) < 0.0, "axis is inside");
        assert!(c.eval(Vec3::new(1.0, 0.0, 0.0)).abs() < EPS, "radial surface is zero");
        assert!(c.eval(Vec3::new(0.0, 2.0, 0.0)).abs() < EPS, "cap surface is zero");
        assert!(c.eval(Vec3::new(0.0, 3.0, 0.0)) > 0.0, "above the cap is outside");
    }

    #[test]
    fn hard_boolean_ops_are_min_max() {
        let a = Sdf::Sphere { radius: 1.0 };
        let b = Sdf::Cuboid { half: Vec3::splat(0.5) };
        let p = Vec3::new(0.4, 0.0, 0.0);
        let union = Sdf::Union(Box::new(a.clone()), Box::new(b.clone()));
        let inter = Sdf::Intersect(Box::new(a.clone()), Box::new(b.clone()));
        let sub = Sdf::Subtract(Box::new(a.clone()), Box::new(b.clone()));
        assert!((union.eval(p) - a.eval(p).min(b.eval(p))).abs() < EPS);
        assert!((inter.eval(p) - a.eval(p).max(b.eval(p))).abs() < EPS);
        assert!((sub.eval(p) - a.eval(p).max(-b.eval(p))).abs() < EPS);
    }

    #[test]
    fn smooth_union_at_zero_radius_equals_hard_union() {
        let a = Sdf::Sphere { radius: 1.0 };
        let b = Sdf::Sphere { radius: 1.0 };
        let hard = Sdf::Union(Box::new(a.clone()), Box::new(b.clone()));
        let smooth = Sdf::SmoothUnion { a: Box::new(a), b: Box::new(b), radius: 0.0 };
        for x in [-1.0_f32, -0.3, 0.0, 0.7, 1.5] {
            let p = Vec3::new(x, 0.2, 0.0);
            assert!((hard.eval(p) - smooth.eval(p)).abs() < EPS, "x={x}");
        }
    }

    #[test]
    fn smooth_union_rounds_the_seam_inward() {
        // Two spheres straddling the origin: a smooth blend fills the seam, so the field at the
        // join is more negative (more solid) than the hard min.
        let a = Sdf::Sphere { radius: 1.0 }.into_rigid(Vec3::new(-0.8, 0.0, 0.0));
        let b = Sdf::Sphere { radius: 1.0 }.into_rigid(Vec3::new(0.8, 0.0, 0.0));
        let hard = a.clone().min_eval(&b);
        let smooth = Sdf::SmoothUnion { a: Box::new(a), b: Box::new(b), radius: 0.5 };
        let p = Vec3::ZERO;
        assert!(smooth.eval(p) < hard, "smooth seam is more solid than the hard min");
    }

    impl Sdf {
        fn into_rigid(self, t: Vec3) -> Sdf {
            Sdf::Rigid { rotation: Quat::IDENTITY, translation: t, node: Box::new(self) }
        }
        fn min_eval(&self, other: &Sdf) -> f32 {
            self.eval(Vec3::ZERO).min(other.eval(Vec3::ZERO))
        }
    }
}
