//! Shared 2D polygon helpers used by every section/profile kernel (panel, sweep, the loft/extrude
//! builders). Hoisted here so the same shoelace area and convexity test are not copy-pasted per
//! crate — the de-duplication the workspace's anti-duplication gate enforces.

use glam::Vec2;

/// Signed area of a closed polygon (shoelace); positive when wound counter-clockwise, ~0 when
/// degenerate. Callers that only need the winding read its sign.
pub fn signed_area(points: &[Vec2]) -> f32 {
    let n = points.len();
    0.5 * (0..n).map(|i| points[i].perp_dot(points[(i + 1) % n])).sum::<f32>()
}

/// A simple polygon is convex if every consecutive turn keeps the same sign (collinear runs allowed).
pub fn is_convex(points: &[Vec2]) -> bool {
    let n = points.len();
    let mut sign = 0.0_f32;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        let c = points[(i + 2) % n];
        let cross = (b - a).perp_dot(c - b);
        if cross.abs() > 1.0e-6 {
            if sign != 0.0 && sign * cross < 0.0 {
                return false;
            }
            sign = cross;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ccw_square_is_convex_with_positive_area() {
        let square = [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
        assert!(is_convex(&square));
        assert!((signed_area(&square) - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn a_reflex_polygon_is_not_convex() {
        let arrow =
            [Vec2::new(0.0, 0.0), Vec2::new(2.0, 1.0), Vec2::new(0.0, 2.0), Vec2::new(0.5, 1.0)];
        assert!(!is_convex(&arrow));
    }

    #[test]
    fn clockwise_winding_flips_the_area_sign() {
        let cw = [Vec2::ZERO, Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 0.0)];
        assert!(signed_area(&cw) < 0.0);
    }
}
