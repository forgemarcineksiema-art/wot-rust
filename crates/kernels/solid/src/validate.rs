//! Half-space validation for the convex-solid kernel: the typed failures and the bounded/usable
//! checks that keep `to_mesh` from silently emitting undefined geometry. Split out of `convex` to
//! keep each file within the reviewability budget.

use glam::Vec3;

use crate::Plane;

pub(crate) const MIN_NORMAL: f32 = 1.0e-6;
const EPS: f32 = 1.0e-4;

/// Why a set of half-spaces cannot be turned into a valid bounded convex solid.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum ConvexSolidError {
    #[error("a convex solid needs at least four usable planes, got {0}")]
    TooFewPlanes(usize),
    #[error("plane {index} has non-finite data")]
    NonFinitePlane { index: usize },
    #[error("plane {index} has a near-zero normal")]
    DegenerateNormal { index: usize },
    #[error("the half-spaces have an empty intersection")]
    EmptyIntersection,
    #[error("the half-spaces are unbounded — they enclose no finite volume")]
    Unbounded,
    #[error("a face has fewer than three corners after clipping")]
    DegenerateFace,
}

/// Reject non-finite planes, near-zero normals, and fewer than four distinct usable planes.
pub(crate) fn validate_planes(planes: &[Plane]) -> Result<(), ConvexSolidError> {
    for (index, plane) in planes.iter().enumerate() {
        if !plane.normal.is_finite() || !plane.offset.is_finite() {
            return Err(ConvexSolidError::NonFinitePlane { index });
        }
        if plane.normal.length() < MIN_NORMAL {
            return Err(ConvexSolidError::DegenerateNormal { index });
        }
    }
    let mut distinct: Vec<&Plane> = Vec::new();
    for plane in planes {
        if !distinct
            .iter()
            .any(|q| q.normal.distance(plane.normal) < EPS && (q.offset - plane.offset).abs() < EPS)
        {
            distinct.push(plane);
        }
    }
    if distinct.len() < 4 {
        return Err(ConvexSolidError::TooFewPlanes(distinct.len()));
    }
    Ok(())
}

/// Whether the half-spaces enclose a finite volume. A recession direction `d` (with
/// `normal·d <= 0` for every plane) means the solid runs to infinity; the extreme rays of that
/// recession cone lie along the plane normals and their pairwise cross products, so testing those
/// candidates detects an open solid exactly.
pub(crate) fn is_bounded(planes: &[Plane]) -> bool {
    let normals: Vec<Vec3> = planes.iter().map(|p| p.normal).collect();
    let mut candidates: Vec<Vec3> = normals.clone();
    for i in 0..normals.len() {
        for j in (i + 1)..normals.len() {
            let cross = normals[i].cross(normals[j]);
            if cross.length() > MIN_NORMAL {
                candidates.push(cross.normalize());
            }
        }
    }
    !candidates
        .iter()
        .any(|&c| [c, -c].into_iter().any(|d| normals.iter().all(|n| n.dot(d) <= 1.0e-5)))
}
