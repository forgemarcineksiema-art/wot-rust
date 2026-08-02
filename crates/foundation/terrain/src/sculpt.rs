//! Terrain sculpting vocabulary beyond summed bumps. A steppe survives on Gaussians; a river
//! valley is made of *structural* features — a channel swept along a curve, a town terrace, a
//! causeway deck — that must hold guarantees by construction ("the ford is always fordable",
//! "the deck is always above the water"). The tools here express those directly:
//!
//! - [`smoothstep01`] / [`band_mask`]: C¹ masks for "inside a feature, fading at its edge";
//! - [`lerp`]: blend terrain toward an explicit target elevation under a mask — carving a
//!   riverbed to `water level − depth` or flattening a town bench to a ramp, instead of
//!   subtracting a bump and hoping the residue lands where gameplay needs it.
//!
//! Everything is pure f32 arithmetic: deterministic, and mirror-symmetry survives as long as
//! the caller's curves and masks are even about the map's symmetry axis.

/// Plain linear blend — the same `lerp` the heightmap's bilinear sampler uses. Masks from
/// [`band_mask`]/[`smoothstep01`] are already in `[0, 1]`, so carving/flattening callers never
/// need a clamp here.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hermite smoothstep of `t` clamped to `[0, 1]`: 0 at 0, 1 at 1, flat at both ends.
pub fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1.0 within `half_width` of the center distance, easing smoothly to 0.0 over `falloff`.
/// `distance` is an absolute lateral distance (e.g. to a river centerline or a road axis).
pub fn band_mask(distance: f32, half_width: f32, falloff: f32) -> f32 {
    smoothstep01((half_width + falloff - distance.abs()) / falloff.max(1.0e-3))
}

/// Distance from `(x, z)` to the nearest point on a polyline walked in order. The shared
/// primitive under road paint and stroke terrain ops — one implementation, so what the eye
/// reads along a line and what the heightfield does along it can never drift apart.
pub fn polyline_distance(points: &[[f32; 2]], x: f32, z: f32) -> f32 {
    let mut best_sq = f32::MAX;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (abx, abz) = (b[0] - a[0], b[1] - a[1]);
        let (apx, apz) = (x - a[0], z - a[1]);
        let len_sq = abx * abx + abz * abz;
        let t = if len_sq <= f32::EPSILON {
            0.0
        } else {
            ((apx * abx + apz * abz) / len_sq).clamp(0.0, 1.0)
        };
        let (dx, dz) = (apx - abx * t, apz - abz * t);
        best_sq = best_sq.min(dx * dx + dz * dz);
    }
    best_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_are_full_inside_zero_outside_and_smooth_between() {
        assert_eq!(band_mask(0.0, 10.0, 5.0), 1.0);
        assert_eq!(band_mask(10.0, 10.0, 5.0), 1.0);
        assert_eq!(band_mask(15.0, 10.0, 5.0), 0.0);
        let mid = band_mask(12.5, 10.0, 5.0);
        assert!(mid > 0.4 && mid < 0.6, "the edge eases, got {mid}");
        // Symmetric in distance sign — a band is a band on both sides of its axis.
        assert_eq!(band_mask(-12.5, 10.0, 5.0), mid);
    }

    #[test]
    fn lerp_carves_exactly_to_target_under_a_full_mask() {
        // The property the river contract stands on: mask 1 means the terrain IS the target.
        assert_eq!(lerp(6.0, 2.5, 1.0), 2.5);
        assert_eq!(lerp(6.0, 2.5, 0.0), 6.0);
        assert_eq!(smoothstep01(0.0), 0.0);
        assert_eq!(smoothstep01(1.0), 1.0);
    }
}
