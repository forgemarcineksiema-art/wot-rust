//! An explicit, validated contract for surfaces of revolution. The low-level [`crate::revolve`]
//! takes raw `(offset, radius)` tuples and trusts them; [`RevolveProfile`] adds the validation and
//! the cumulative arc-length parameterisation (the future UV `V` axis) the production path wants.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

use crate::revolve::revolve;

/// One station of a revolution profile: a `radius` at an `offset` along the axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfilePoint {
    pub offset: f32,
    pub radius: f32,
}

impl ProfilePoint {
    pub fn new(offset: f32, radius: f32) -> Self {
        Self { offset, radius }
    }
}

/// How the ends of a revolution are closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevolveCaps {
    /// Leave the ends open (the profile already authors any radius-zero apex it wants).
    Open,
    /// Add a flat end disc at each end whose terminal radius is positive.
    Capped,
}

/// A validated revolution profile plus its cap policy.
#[derive(Debug, Clone, PartialEq)]
pub struct RevolveProfile {
    pub points: Vec<ProfilePoint>,
    pub caps: RevolveCaps,
}

/// Why a revolution cannot be built.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum RevolveError {
    #[error("axis must be finite and non-zero")]
    InvalidAxis,
    #[error("profile needs at least two points")]
    TooFewPoints,
    #[error("profile point {index} is invalid")]
    InvalidPoint { index: usize },
    #[error("profile radius must be non-negative")]
    NegativeRadius,
    #[error("adjacent profile points are coincident")]
    CoincidentPoints,
    #[error("segments must be at least 3")]
    TooFewSegments,
}

impl RevolveProfile {
    pub fn new(points: Vec<ProfilePoint>, caps: RevolveCaps) -> Self {
        Self { points, caps }
    }

    /// Cumulative arc length along the profile polyline, one entry per point — the parameter a
    /// later UV pass uses for the `V` axis.
    pub fn arc_lengths(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.points.len());
        let mut acc = 0.0;
        for (i, p) in self.points.iter().enumerate() {
            if i > 0 {
                let prev = self.points[i - 1];
                acc += ((p.offset - prev.offset).powi(2) + (p.radius - prev.radius).powi(2)).sqrt();
            }
            out.push(acc);
        }
        out
    }

    fn validate(&self) -> Result<(), RevolveError> {
        if self.points.len() < 2 {
            return Err(RevolveError::TooFewPoints);
        }
        for (index, p) in self.points.iter().enumerate() {
            if !p.offset.is_finite() || !p.radius.is_finite() {
                return Err(RevolveError::InvalidPoint { index });
            }
            if p.radius < 0.0 {
                return Err(RevolveError::NegativeRadius);
            }
        }
        for pair in self.points.windows(2) {
            if (pair[0].offset - pair[1].offset).abs() < 1.0e-7
                && (pair[0].radius - pair[1].radius).abs() < 1.0e-7
            {
                return Err(RevolveError::CoincidentPoints);
            }
        }
        Ok(())
    }

    /// The raw `(offset, radius)` rings, adding flat end discs when `Capped` and the terminal radius
    /// is positive (a radius-zero apex is already a cap and must not be duplicated).
    fn rings(&self) -> Vec<(f32, f32)> {
        let mut rings: Vec<(f32, f32)> = Vec::with_capacity(self.points.len() + 2);
        if self.caps == RevolveCaps::Capped && self.points[0].radius > 0.0 {
            rings.push((self.points[0].offset, 0.0));
        }
        rings.extend(self.points.iter().map(|p| (p.offset, p.radius)));
        let last = *self.points.last().unwrap();
        if self.caps == RevolveCaps::Capped && last.radius > 0.0 {
            rings.push((last.offset, 0.0));
        }
        rings
    }
}

/// Validate `axis`, `segments` and `profile`, then revolve into a clean surface of revolution.
pub fn try_revolve(
    axis: Vec3,
    profile: &RevolveProfile,
    segments: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> Result<GeometryMesh, RevolveError> {
    if !axis.is_finite() || axis.length() < 1.0e-6 {
        return Err(RevolveError::InvalidAxis);
    }
    if segments < 3 {
        return Err(RevolveError::TooFewSegments);
    }
    profile.validate()?;
    Ok(revolve(axis, &profile.rings(), segments, material, smoothing))
}
