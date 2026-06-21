//! Thin fabricated-plate kernel (renderer-free): a convex outline given thickness and a hard-edged
//! edge treatment, for engine-deck panels, fender sections and cover/grille frames. It is *not* a
//! replacement for exact `solid` armour plates — use it only where a part is genuinely thin sheet.
//!
//! Faces are exact planes with hard-edge smoothing by default, so a panel reads as crisp folded
//! steel rather than a softened blob.

mod build;

use glam::{Vec2, Vec3};
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

use crate::build::{Frame, PanelRings, build, inset_convex, signed_area};

/// How a panel's top perimeter edge is finished.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelEdge {
    /// A square, unbroken edge.
    Sharp,
    /// A 45-degree bevel of the given width around the top rim.
    Chamfer { width: f32 },
    /// A folded return lip: a chamfer of `width` plus an extra `return_depth` of skirt below.
    Hem { width: f32, return_depth: f32 },
}

/// A thin fabricated panel: a convex outline in the plane at `origin` facing `normal`, extruded by
/// `thickness` with an edge treatment.
pub struct PanelSpec {
    pub outline: Vec<Vec2>,
    pub origin: Vec3,
    pub normal: Vec3,
    pub thickness: f32,
    pub edge: PanelEdge,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}

/// Why a panel cannot be built.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum PanelError {
    #[error("a panel outline needs at least three points")]
    TooFewOutlinePoints,
    #[error("panel has non-finite data")]
    NonFinite,
    #[error("panel thickness must be positive")]
    NonPositiveThickness,
    #[error("panel normal must be finite and non-zero")]
    InvalidNormal,
    #[error("panel outline must be convex")]
    NonConvexOutline,
    #[error("edge treatment is invalid for this outline and thickness")]
    InvalidEdge,
}

/// Validate the spec and build the closed panel mesh.
pub fn try_panel(spec: &PanelSpec) -> Result<GeometryMesh, PanelError> {
    if spec.outline.len() < 3 {
        return Err(PanelError::TooFewOutlinePoints);
    }
    if spec.outline.iter().any(|p| !p.is_finite())
        || !spec.origin.is_finite()
        || !spec.thickness.is_finite()
    {
        return Err(PanelError::NonFinite);
    }
    if spec.thickness <= 0.0 {
        return Err(PanelError::NonPositiveThickness);
    }
    if !spec.normal.is_finite() || spec.normal.length() < 1.0e-6 {
        return Err(PanelError::InvalidNormal);
    }
    // Normalise to a counter-clockwise convex outline so the top face winds toward +normal.
    let mut outline = spec.outline.clone();
    if signed_area(&outline) < 0.0 {
        outline.reverse();
    }
    if !is_convex(&outline) {
        return Err(PanelError::NonConvexOutline);
    }

    let rings = rings_for(&outline, spec.thickness, spec.edge)?;
    let frame = Frame::new(spec.origin, spec.normal);
    Ok(build(&frame, &rings, spec.material, spec.smoothing))
}

fn rings_for(outline: &[Vec2], thickness: f32, edge: PanelEdge) -> Result<PanelRings, PanelError> {
    match edge {
        PanelEdge::Sharp => Ok(PanelRings {
            top: outline.to_vec(),
            chamfer_outer: None,
            wall_top_depth: 0.0,
            bottom: outline.to_vec(),
            bottom_depth: thickness,
        }),
        PanelEdge::Chamfer { width } => {
            let inset = chamfer_inset(outline, width, thickness)?;
            Ok(PanelRings {
                top: inset,
                chamfer_outer: Some((outline.to_vec(), width)),
                wall_top_depth: width,
                bottom: outline.to_vec(),
                bottom_depth: thickness,
            })
        }
        PanelEdge::Hem { width, return_depth } => {
            if !return_depth.is_finite() || return_depth <= 0.0 {
                return Err(PanelError::InvalidEdge);
            }
            let inset = chamfer_inset(outline, width, thickness)?;
            Ok(PanelRings {
                top: inset,
                chamfer_outer: Some((outline.to_vec(), width)),
                wall_top_depth: width,
                bottom: outline.to_vec(),
                bottom_depth: thickness + return_depth,
            })
        }
    }
}

fn chamfer_inset(outline: &[Vec2], width: f32, thickness: f32) -> Result<Vec<Vec2>, PanelError> {
    if !width.is_finite() || width <= 0.0 || width >= thickness {
        return Err(PanelError::InvalidEdge);
    }
    inset_convex(outline, width).ok_or(PanelError::InvalidEdge)
}

/// A polygon is convex if every consecutive turn keeps the same sign (collinear runs allowed).
fn is_convex(points: &[Vec2]) -> bool {
    let n = points.len();
    let mut sign = 0.0_f32;
    for i in 0..n {
        let cross =
            (points[(i + 1) % n] - points[i]).perp_dot(points[(i + 2) % n] - points[(i + 1) % n]);
        if cross.abs() > 1.0e-6 {
            if sign != 0.0 && sign * cross < 0.0 {
                return false;
            }
            sign = cross;
        }
    }
    true
}
