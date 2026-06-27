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

    #[test]
    fn an_invalid_bump_is_rejected() {
        let sections = [section(0.0), section(0.5)];
        let bumps = [CastBump { azimuth: 0.0, az_width: 0.0, y: 0.2, y_width: 0.2, amount: 0.1 }];
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
