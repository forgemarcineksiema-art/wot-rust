//! Explicit input-failure handling for the cast-loft kernel. The geometry builder assumes a valid
//! spec; this module is the gate that turns malformed authoring data into a typed error instead of
//! silent garbage geometry or a panic deep in the mesher.

use vehicle_geometry::GeometryMesh;

use crate::{CastCap, CastLoftSpec, build_cast_loft};

/// Why a [`CastLoftSpec`] cannot be skinned into a valid cast shell.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CastLoftError {
    #[error("cast loft needs at least two stations")]
    TooFewStations,
    #[error("cast loft segment count must be at least 3")]
    TooFewSegments,
    #[error("station {index} is not finite")]
    NonFiniteStation { index: usize },
    #[error("station heights must strictly increase")]
    NonMonotonicStations,
    #[error("station {index} has non-positive extent")]
    NonPositiveExtent { index: usize },
    #[error("superellipse exponent must be finite and at least 2")]
    InvalidExponent,
    #[error("bump {index} has invalid width or position")]
    InvalidBump { index: usize },
    #[error(
        "bump {index} recesses {amount} m into a casting whose local half-extent is about          {local_radius} m — the surface would pass through its own axis"
    )]
    RecessDeeperThanTheCasting { index: usize, amount: f32, local_radius: f32 },
    #[error(
        "bump {index} is {y_width} m tall but the stations around y={y} are {station_gap} m          apart — the feature falls between samples and would be aliased away"
    )]
    BumpFallsBetweenStations { index: usize, y: f32, y_width: f32, station_gap: f32 },
    #[error("cap is invalid")]
    InvalidCap,
}

/// Validate `spec` and skin it into a watertight cast shell, or report the first input failure.
pub fn try_build_cast_loft(spec: &CastLoftSpec<'_>) -> Result<GeometryMesh, CastLoftError> {
    validate(spec)?;
    Ok(build_cast_loft(spec))
}

fn validate(spec: &CastLoftSpec<'_>) -> Result<(), CastLoftError> {
    if spec.sections.len() < 2 {
        return Err(CastLoftError::TooFewStations);
    }
    if spec.segments < 3 {
        return Err(CastLoftError::TooFewSegments);
    }

    let mut prev_y = f32::NEG_INFINITY;
    for (index, s) in spec.sections.iter().enumerate() {
        let finite = s.y.is_finite()
            && s.half_width.is_finite()
            && s.half_len_front.is_finite()
            && s.half_len_rear.is_finite()
            && s.z_center.is_finite()
            && s.exponent.is_finite();
        if !finite {
            return Err(CastLoftError::NonFiniteStation { index });
        }
        if s.half_width <= 0.0 || s.half_len_front <= 0.0 || s.half_len_rear <= 0.0 {
            return Err(CastLoftError::NonPositiveExtent { index });
        }
        if s.exponent < 2.0 {
            return Err(CastLoftError::InvalidExponent);
        }
        if s.y <= prev_y {
            return Err(CastLoftError::NonMonotonicStations);
        }
        prev_y = s.y;
    }

    for (index, b) in spec.bumps.iter().enumerate() {
        let finite = b.azimuth.is_finite()
            && b.az_width.is_finite()
            && b.y.is_finite()
            && b.y_width.is_finite()
            && b.amount.is_finite();
        if !finite || b.az_width <= 0.0 || b.y_width <= 0.0 {
            return Err(CastLoftError::InvalidBump { index });
        }

        // A recess is a dent, not a hole. Pushed deeper than the local half-extent, the surface
        // crosses its own axis and self-intersects — a mesh that is watertight by edge count and
        // nonsense by geometry, which no downstream contract can see.
        let local_radius = spec
            .sections
            .iter()
            .filter(|station| (station.y - b.y).abs() <= b.y_width.max(1.0e-3))
            .map(|station| {
                station.half_width.min(station.half_len_front.min(station.half_len_rear))
            })
            .fold(f32::INFINITY, f32::min);
        if local_radius.is_finite() && b.amount < 0.0 && -b.amount >= local_radius {
            return Err(CastLoftError::RecessDeeperThanTheCasting {
                index,
                amount: b.amount,
                local_radius,
            });
        }

        // Bumps are sampled AT station heights. A feature narrower than the local station
        // spacing lands between samples: it either vanishes or shows up on one ring only,
        // depending on where the stations happen to sit. Dense stations are the price of a
        // crisp feature, and the caller must pay it explicitly.
        let mut station_gap = f32::INFINITY;
        for pair in spec.sections.windows(2) {
            let (low, high) = (pair[0].y, pair[1].y);
            if b.y >= low - b.y_width && b.y <= high + b.y_width {
                station_gap = station_gap.min(high - low);
            }
        }
        if station_gap.is_finite() && b.y_width < station_gap * 0.5 {
            return Err(CastLoftError::BumpFallsBetweenStations {
                index,
                y: b.y,
                y_width: b.y_width,
                station_gap,
            });
        }
    }

    for cap in [spec.caps.bottom, spec.caps.top] {
        if let CastCap::Apex(apex) = cap
            && !apex.is_finite()
        {
            return Err(CastLoftError::InvalidCap);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    use super::*;
    use crate::{CastBump, CastCaps, CastSection};

    fn section(y: f32) -> CastSection {
        CastSection::symmetric(y, 0.8, 0.9, 0.0, 2.8)
    }

    fn spec<'a>(
        sections: &'a [CastSection],
        bumps: &'a [CastBump],
        caps: CastCaps,
    ) -> CastLoftSpec<'a> {
        CastLoftSpec {
            sections,
            bumps,
            segments: 16,
            caps,
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup(2),
        }
    }

    #[test]
    fn a_valid_spec_builds() {
        let sections = [section(0.0), section(0.5)];
        assert!(try_build_cast_loft(&spec(&sections, &[], CastCaps::default())).is_ok());
    }

    #[test]
    fn too_few_stations_is_rejected() {
        let sections = [section(0.0)];
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &[], CastCaps::default())).unwrap_err(),
            CastLoftError::TooFewStations
        );
    }

    #[test]
    fn too_few_segments_is_rejected() {
        let sections = [section(0.0), section(0.5)];
        let mut s = spec(&sections, &[], CastCaps::default());
        s.segments = 2;
        assert_eq!(try_build_cast_loft(&s).unwrap_err(), CastLoftError::TooFewSegments);
    }

    #[test]
    fn a_non_finite_station_is_rejected() {
        let sections = [section(0.0), section(f32::NAN)];
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &[], CastCaps::default())).unwrap_err(),
            CastLoftError::NonFiniteStation { index: 1 }
        );
    }

    #[test]
    fn non_monotonic_stations_are_rejected() {
        let sections = [section(0.5), section(0.2)];
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &[], CastCaps::default())).unwrap_err(),
            CastLoftError::NonMonotonicStations
        );
    }

    #[test]
    fn a_non_positive_extent_is_rejected() {
        let mut bad = section(0.5);
        bad.half_width = 0.0;
        let sections = [section(0.0), bad];
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &[], CastCaps::default())).unwrap_err(),
            CastLoftError::NonPositiveExtent { index: 1 }
        );
    }

    #[test]
    fn an_exponent_below_two_is_rejected() {
        let sections = [CastSection::symmetric(0.0, 0.8, 0.9, 0.0, 1.5), section(0.5)];
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &[], CastCaps::default())).unwrap_err(),
            CastLoftError::InvalidExponent
        );
    }

    /// A recess is a dent, not a hole. Pushed past the local half-extent the surface crosses
    /// its own axis: still watertight by edge count, geometric nonsense in fact — the class of
    /// defect no downstream contract can see.
    /// The technology a gun embrasure needs: a feature with STEEP walls and a flat floor, not a
    /// dish. A plain Gaussian recess reads as a dimple pressed into the casting; an aperture is
    /// cut into it. Same kernel, same station stack — the falloff exponent is the difference.
    #[test]
    fn a_plateau_bump_cuts_a_squarer_pocket_than_a_gaussian_of_the_same_size() {
        let sections = [section(0.0), section(0.20), section(0.25), section(0.30), section(0.5)];
        // Total material left around the feature's ring: a plateau holds full depth across its
        // whole width, so it removes MORE metal than a Gaussian of identical width and depth,
        // while both cut to the same floor at the centre.
        let ring_material = |bump: CastBump| {
            let mesh = try_build_cast_loft(&spec(&sections, &[bump], CastCaps::default()))
                .expect("the loft builds");
            let mut sum = 0.0_f64;
            let mut floor = f32::MAX;
            for vertex in mesh.vertices() {
                if (vertex.position.y - 0.25).abs() > 0.01 {
                    continue;
                }
                let radius = (vertex.position.x * vertex.position.x
                    + vertex.position.z * vertex.position.z)
                    .sqrt();
                sum += f64::from(radius);
                // The floor is the deepest cut anywhere on the ring.
                floor = floor.min(radius);
            }
            (sum, floor)
        };

        let (soft_sum, soft_floor) =
            ring_material(CastBump::gaussian(0.0, 0.45, 0.25, 0.12, -0.15));
        let (sharp_sum, sharp_floor) =
            ring_material(CastBump::plateau(0.0, 0.45, 0.25, 0.12, -0.15, 8.0));

        assert!(
            (soft_floor - sharp_floor).abs() < 0.02,
            "both cut to the same floor at the centre: {soft_floor:.3} vs {sharp_floor:.3}"
        );
        assert!(
            sharp_sum < soft_sum - 0.05,
            "the plateau holds its depth across the feature instead of fading out, so it takes              more metal: {sharp_sum:.3} vs {soft_sum:.3}"
        );
    }

    /// The default is unchanged: `gaussian` IS the curve this kernel has always used, so every
    /// existing casting keeps its shape to the bit.
    #[test]
    fn the_gaussian_constructor_is_the_kernels_historical_curve() {
        let sections = [section(0.0), section(0.2), section(0.3), section(0.5)];
        let named = CastBump::gaussian(0.3, 0.4, 0.25, 0.2, 0.1);
        let literal = CastBump {
            azimuth: 0.3,
            az_width: 0.4,
            y: 0.25,
            y_width: 0.2,
            amount: 0.1,
            falloff_exponent: 2.0,
        };
        let build = |bump| {
            let mesh = try_build_cast_loft(&spec(
                &sections,
                std::slice::from_ref(&bump),
                CastCaps::default(),
            ))
            .expect("builds");
            mesh.vertices().iter().map(|vertex| vertex.position.to_array()).collect::<Vec<_>>()
        };
        assert_eq!(build(named), build(literal), "the named constructor must change nothing");
    }

    #[test]
    fn a_recess_deeper_than_the_casting_is_rejected() {
        let sections = [section(0.0), section(0.5)];
        // The test casting's smallest local half-extent is 0.8 m; ask for a 0.9 m dent.
        let bumps = [CastBump {
            azimuth: 0.0,
            az_width: 0.4,
            y: 0.25,
            y_width: 0.3,
            amount: -0.9,
            falloff_exponent: 2.0,
        }];
        assert!(matches!(
            try_build_cast_loft(&spec(&sections, &bumps, CastCaps::default())).unwrap_err(),
            CastLoftError::RecessDeeperThanTheCasting { index: 0, .. }
        ));
        // A shallow dent of the same shape is exactly what this kernel is for.
        let shallow = [CastBump {
            azimuth: 0.0,
            az_width: 0.4,
            y: 0.25,
            y_width: 0.3,
            amount: -0.12,
            falloff_exponent: 2.0,
        }];
        assert!(try_build_cast_loft(&spec(&sections, &shallow, CastCaps::default())).is_ok());
    }

    /// Bumps are sampled AT station heights, so a feature narrower than the local station
    /// spacing lands between samples and is aliased away — or appears on exactly one ring,
    /// depending on where the stations happen to sit. The embrasure work depends on this being
    /// an error rather than a silent disappearance.
    #[test]
    fn a_bump_narrower_than_the_station_spacing_is_rejected() {
        let sections = [section(0.0), section(0.5)];
        let sharp = [CastBump {
            azimuth: 1.4,
            az_width: 0.3,
            y: 0.25,
            y_width: 0.05,
            amount: -0.1,
            falloff_exponent: 2.0,
        }];
        assert!(matches!(
            try_build_cast_loft(&spec(&sections, &sharp, CastCaps::default())).unwrap_err(),
            CastLoftError::BumpFallsBetweenStations { index: 0, .. }
        ));
        // Densify the stations around the feature and the same bump is legal — the fix is to
        // pay for the resolution, which is what a crisp aperture actually costs.
        let dense = [section(0.0), section(0.20), section(0.25), section(0.30), section(0.5)];
        assert!(try_build_cast_loft(&spec(&dense, &sharp, CastCaps::default())).is_ok());
    }

    #[test]
    fn an_invalid_bump_is_rejected() {
        let sections = [section(0.0), section(0.5)];
        let bumps = [CastBump {
            azimuth: 0.0,
            az_width: 0.0,
            y: 0.2,
            y_width: 0.2,
            amount: 0.1,
            falloff_exponent: 2.0,
        }];
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &bumps, CastCaps::default())).unwrap_err(),
            CastLoftError::InvalidBump { index: 0 }
        );
    }

    #[test]
    fn a_non_finite_apex_cap_is_rejected() {
        let sections = [section(0.0), section(0.5)];
        let caps = CastCaps { bottom: CastCap::Apex(Vec3::NAN), top: CastCap::Planar };
        assert_eq!(
            try_build_cast_loft(&spec(&sections, &[], caps)).unwrap_err(),
            CastLoftError::InvalidCap
        );
    }
}
