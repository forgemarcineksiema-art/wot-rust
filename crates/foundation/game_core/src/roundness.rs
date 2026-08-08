//! THE ROUNDNESS LAW: how many segments a round part needs is a function of its RADIUS, not of
//! what somebody typed.
//!
//! An n-sided prism inscribed in a circle of radius `r` misses that circle by `r(1 - cos(pi/n))`
//! at each edge midpoint. That deviation is the whole of what "faceted" means, it is measurable,
//! and it depends on the radius — so a segment count chosen for one part says nothing about
//! another.
//!
//! Measured across the fleet before this module existed — **through the resolution path, not the
//! blueprint field**. That distinction cost a wrong finding first time round: `TrackShape.segments`
//! reads 14 on every vehicle and is dead data for the running gear, because
//! `RunningGearKinematics::segments_for` already applies a per-part floor above it.
//!
//! | part | radius | segments it really gets | silhouette error |
//! |---|---:|---:|---:|
//! | road wheel | 0.405 m | 22 (floor) | 4.1 mm |
//! | idler | 0.300 m | 20 (floor) | 3.7 mm |
//! | drive sprocket | 0.300 m | 16 (floor) | 5.8 mm |
//! | **commander's cupola** | 0.312 m | **12** (hand-written) | **10.6 mm** |
//! | **IS-3 fuel drum** | 0.145 m | **10** (hand-written) | **7.1 mm** |
//! | gun barrel | 0.092 m | 12 | 3.1 mm |
//!
//! At 1080p and a ~50° horizontal field of view a pixel subtends about 0.45 mrad, so at 25 m it
//! covers ~11 mm.
//!
//! The lesson the table carries is not "the fleet is faceted" — the running gear's per-part floors
//! largely work. It is that **the parts with a system are fine and the parts without one are not**:
//! a cupola is the same radius as an idler and gets 12 where the idler gets 20, because one went
//! through `segments_for` and the other was typed at the call site. This module is that system,
//! generalised, so a part's radius answers the question wherever it is built.
//!
//! This is a floor, in the sense `docs/vehicle-geometry-policy.md` gives the word: it says how
//! round a thing must be, never how round it may be.

/// How far a round part's silhouette may fall inside its true circle, in metres.
///
/// Calibrated as **a quarter of a pixel at 25 m** on the min-spec presentation (1080p, ~50° hFOV,
/// ~11 mm per pixel at that range): a deviation nobody can resolve at the distance where vehicles
/// are actually fought. It is one number for the whole fleet on purpose — the point of the law is
/// that the radius decides, not the author.
pub const SILHOUETTE_TOLERANCE_M: f32 = 0.0028;

/// Segments below which a revolve stops reading as round at all, whatever its radius. A 6-sided
/// bolt head is a bolt head; a 6-sided road wheel is a hexagon.
pub const MIN_SEGMENTS: usize = 8;

/// The ceiling. Past this the silhouette gain is under a tenth of a pixel and the triangles are
/// spent on nothing — and on instanced running gear they are spent 200 times over.
pub const MAX_SEGMENTS: usize = 48;

/// The segment count a part of `radius_m` needs to hold its silhouette within `tolerance_m`.
///
/// Exact rather than the small-angle approximation, because the approximation drifts at the low
/// counts (8–12) where most of this fleet's round parts actually live.
pub fn segments_for_radius(radius_m: f32, tolerance_m: f32) -> usize {
    let radius = radius_m.abs();
    let tolerance = tolerance_m.abs();
    if !radius.is_finite() || !tolerance.is_finite() || radius <= tolerance || tolerance <= 0.0 {
        return MIN_SEGMENTS;
    }
    // r(1 - cos(pi/n)) <= tol  =>  n >= pi / acos(1 - tol/r)
    let cosine = (1.0 - tolerance / radius).clamp(-1.0, 1.0);
    let angle = cosine.acos();
    if angle <= f32::EPSILON {
        return MAX_SEGMENTS;
    }
    let needed = (std::f32::consts::PI / angle).ceil();
    (needed as usize).clamp(MIN_SEGMENTS, MAX_SEGMENTS)
}

/// The fleet-standard count for a part of this radius.
pub fn round_segments(radius_m: f32) -> usize {
    segments_for_radius(radius_m, SILHOUETTE_TOLERANCE_M)
}

/// How far a prism of `segments` sides misses the circle of `radius_m`, in metres — the number
/// the law is written against, exposed so tests and reports can quote it rather than restate it.
pub fn silhouette_error_m(radius_m: f32, segments: usize) -> f32 {
    if segments < 3 {
        return radius_m.abs();
    }
    radius_m.abs() * (1.0 - (std::f32::consts::PI / segments as f32).cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_law_holds_its_own_tolerance_at_every_radius_the_fleet_uses() {
        // 8 mm (a bolt head) to 1.2 m (a turret ring), the range this fleet actually spans.
        let mut radius = 0.008_f32;
        while radius <= 1.2 {
            let n = round_segments(radius);
            let error = silhouette_error_m(radius, n);
            assert!(
                error <= SILHOUETTE_TOLERANCE_M || n == MAX_SEGMENTS,
                "r={radius:.3}: {n} segments leaves {:.4} m of error",
                error
            );
            radius *= 1.15;
        }
    }

    #[test]
    fn a_bigger_part_never_asks_for_fewer_segments() {
        let mut previous = 0;
        let mut radius = 0.01_f32;
        while radius <= 1.5 {
            let n = round_segments(radius);
            assert!(n >= previous, "r={radius:.3} asked for {n} after {previous}");
            previous = n;
            radius *= 1.1;
        }
    }

    /// The cases that motivated the law, quoted from the fleet as it actually resolves.
    #[test]
    fn parts_with_no_system_are_the_ones_that_facet() {
        // A cupola and an idler are the same size. One went through `segments_for` (20), the other
        // was typed at its call site (12), and only the typed one facets.
        let idler_error = silhouette_error_m(0.300, 20);
        let cupola_error = silhouette_error_m(0.312, 12);
        assert!(idler_error < 0.004, "the idler's floor holds it: {idler_error:.4} m");
        assert!(cupola_error > 0.010, "the hand-written cupola does not: {cupola_error:.4} m");
        assert!(cupola_error > idler_error * 2.5, "and the gap is not marginal");

        // The law asks the bigger part for more, which is the whole content of it.
        assert!(round_segments(0.415) > round_segments(0.145));
        assert!(round_segments(0.145) > round_segments(0.07));
    }

    #[test]
    fn degenerate_inputs_fall_back_to_the_floor_instead_of_dividing_by_zero() {
        for radius in [0.0_f32, -1.0, f32::NAN, f32::INFINITY] {
            let n = round_segments(radius);
            assert!((MIN_SEGMENTS..=MAX_SEGMENTS).contains(&n), "r={radius} gave {n}");
        }
        assert_eq!(segments_for_radius(0.5, 0.0), MIN_SEGMENTS);
    }
}
