//! Path-sweep kernel (renderer-free): drive a stable cross-section along a path frame to build
//! tracks, and later hoses, rails and welded bead paths. Parallel transport is the default frame
//! because it minimises twist along a spatial path and avoids the arbitrary roll a naive Frenet
//! frame produces on straight or inflected runs; a fixed-up frame suits planar bands.
//!
//! v1 requires a non-self-degenerate path and a convex section, and emits a single
//! [`vehicle_geometry::GeometryMesh`] with smooth normals rebuilt from the swept faces.

mod frame;

use glam::{Vec2, Vec3};
use vehicle_geometry::{
    GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, is_convex, signed_area,
};

use crate::frame::{Frame, fixed_up, parallel_transport, tangents};

/// A path to sweep along: an ordered list of points, optionally closing back on itself.
pub struct SweepPath {
    pub points: Vec<Vec3>,
    pub closed: bool,
}

/// A cross-section in the frame's `(u, v)` plane, optionally a closed loop (the usual case).
pub struct SweepSection {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

/// How the section is oriented along the path.
pub enum SweepFrameMode {
    /// Minimal-twist transport — the default for a general spatial path.
    ParallelTransport,
    /// Keep one section axis pinned to a world direction — for planar bands like a track.
    FixedUp(Vec3),
}

/// How the ends of an open sweep are closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepCaps {
    Open,
    Both,
}

/// Everything needed to sweep one section along one path.
pub struct SweepSpec<'a> {
    pub path: &'a SweepPath,
    pub section: &'a SweepSection,
    pub frame_mode: SweepFrameMode,
    pub caps: SweepCaps,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
    /// Per-station scale of the section, one entry per path point (`None` = constant section,
    /// which is every band and bead this kernel has swept so far).
    ///
    /// A constant section can only make tubes and rails. The things a tank carries that are
    /// NOT tubes — a canvas mantlet cover flaring from the barrel out to the turret face, a
    /// tapering fuel line, a bellows — are the same sweep with the section growing along the
    /// path. Without this they had to be faked by revolving and then squashing, which cannot
    /// follow a bent path at all.
    pub section_scale: Option<&'a [f32]>,
}

/// Why a sweep cannot be built.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum SweepError {
    #[error("a sweep path needs at least two points")]
    TooFewPathPoints,
    #[error("a sweep section needs at least three points")]
    TooFewSectionPoints,
    #[error("path or section has non-finite data")]
    NonFinite,
    #[error("path segment {index} is degenerate (zero length)")]
    DegenerateSegment { index: usize },
    #[error("the up vector is invalid for this path")]
    InvalidUp,
    #[error("the section must be convex in this release")]
    NonConvexSection,
    #[error(
        "section_scale has {given} entries for a {expected}-point path — one scale per station"
    )]
    SectionScaleLengthMismatch { given: usize, expected: usize },
    #[error("section scale {value} at station {index} is not a positive, finite number")]
    InvalidSectionScale { index: usize, value: f32 },
}

/// Validate the spec, build the transport frames, and sweep the section into a closed band (or a
/// capped tube for an open path).
pub fn try_sweep(spec: &SweepSpec<'_>) -> Result<GeometryMesh, SweepError> {
    let path = spec.path;
    let section = spec.section;
    validate_spec(spec)?;

    let tans = tangents(&path.points, path.closed);
    let frames = match spec.frame_mode {
        SweepFrameMode::ParallelTransport => parallel_transport(&tans, path.closed),
        SweepFrameMode::FixedUp(up) => {
            if !up.is_finite() || up.length() < 1.0e-6 {
                return Err(SweepError::InvalidUp);
            }
            // The up vector must not be parallel to any tangent, or the projection collapses.
            if tans.iter().any(|t| t.cross(up).length() < 1.0e-4) {
                return Err(SweepError::InvalidUp);
            }
            fixed_up(&tans, up.normalize())
        }
    };

    // Normalize the section to CCW so the fixed cap/side winding below faces outward regardless of
    // how the caller listed its points — the guard `loft` and `extrude` already carry, and the one
    // this kernel alone was missing. `is_convex` (in `validate_spec`) is winding-agnostic, so a
    // CW-but-convex section passed validation and then got inward-facing, lit-from-behind walls.
    // Every section built today is already CCW, so this reverses nothing for them.
    let mut section_points = section.points.clone();
    if signed_area(&section_points) < 0.0 {
        section_points.reverse();
    }
    let rings = build_rings(&path.points, &frames, &section_points, spec);
    let m = section_points.len();
    let n = path.points.len();
    let mut indices = Vec::new();
    stitch_rings(&mut indices, n, m, path.closed, section.closed);
    if !path.closed && spec.caps == SweepCaps::Both {
        add_caps(&mut indices, n, m);
    }
    Ok(GeometryMesh::new(rings, indices).weld_and_smooth())
}

fn validate_spec(spec: &SweepSpec<'_>) -> Result<(), SweepError> {
    let path = spec.path;
    let section = spec.section;
    if path.points.len() < 2 {
        return Err(SweepError::TooFewPathPoints);
    }
    if section.points.len() < 3 {
        return Err(SweepError::TooFewSectionPoints);
    }
    if path.points.iter().any(|p| !p.is_finite()) || section.points.iter().any(|p| !p.is_finite()) {
        return Err(SweepError::NonFinite);
    }
    // A scale table is per STATION: a short one would silently leave the tail of the sweep at
    // full size, which reads as a cover that stops flaring halfway.
    if let Some(scales) = spec.section_scale {
        if scales.len() != path.points.len() {
            return Err(SweepError::SectionScaleLengthMismatch {
                given: scales.len(),
                expected: path.points.len(),
            });
        }
        for (index, value) in scales.iter().enumerate() {
            if !value.is_finite() || *value <= 0.0 {
                return Err(SweepError::InvalidSectionScale { index, value: *value });
            }
        }
    }
    let n = path.points.len();
    let last = if path.closed { n } else { n - 1 };
    for i in 0..last {
        if (path.points[(i + 1) % n] - path.points[i]).length() < 1.0e-6 {
            return Err(SweepError::DegenerateSegment { index: i });
        }
    }
    if !is_convex(&section.points) {
        return Err(SweepError::NonConvexSection);
    }
    Ok(())
}

/// One ring of section points per path station, transformed by that station's frame.
fn build_rings(
    points: &[Vec3],
    frames: &[Frame],
    section: &[Vec2],
    spec: &SweepSpec<'_>,
) -> Vec<GeometryVertex> {
    let mut rings = Vec::with_capacity(points.len() * section.len());
    for (index, (p, frame)) in points.iter().zip(frames).enumerate() {
        // The section may grow or shrink along the path — a cover that flares, a line that
        // tapers. Absent a scale table every station is the authored section, exactly as before.
        let scale = spec.section_scale.and_then(|table| table.get(index).copied()).unwrap_or(1.0);
        for s in section {
            let world = *p + frame.u * (s.x * scale) + frame.v * (s.y * scale);
            rings.push(GeometryVertex::new(world, Vec3::ZERO, spec.material, spec.smoothing));
        }
    }
    rings
}

/// Stitch consecutive rings into quads (two triangles each), wrapping the section and the path loop.
fn stitch_rings(
    indices: &mut Vec<u32>,
    n: usize,
    m: usize,
    path_closed: bool,
    section_closed: bool,
) {
    let ring_segments = if section_closed { m } else { m - 1 };
    let path_steps = if path_closed { n } else { n - 1 };
    for i in 0..path_steps {
        let a = (i * m) as u32;
        let b = (((i + 1) % n) * m) as u32;
        for k in 0..ring_segments {
            let k1 = ((k + 1) % m) as u32;
            let k = k as u32;
            indices.extend_from_slice(&[a + k, a + k1, b + k1, a + k, b + k1, b + k]);
        }
    }
}

/// Fan the first and last section rings into flat end caps for an open sweep.
fn add_caps(indices: &mut Vec<u32>, n: usize, m: usize) {
    let start = 0u32;
    for k in 1..(m as u32 - 1) {
        indices.extend_from_slice(&[start, start + k + 1, start + k]);
    }
    let last = ((n - 1) * m) as u32;
    for k in 1..(m as u32 - 1) {
        indices.extend_from_slice(&[last, last + k, last + k + 1]);
    }
}
