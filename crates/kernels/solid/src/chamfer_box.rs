//! The bevel law's operator: an axis-aligned box whose top and vertical edges take a 45° chamfer.
//! Generic — no vehicle dimension lives here; the fleet's part library (`vehicle_build`) calls it
//! with the blueprint's numbers.

use glam::Vec3;

use crate::{ConvexSolid, Plane};

/// An axis-aligned box with 45° chamfers on its four top edges and four vertical edges — the
/// pressed-steel read of fender stowage (fuel tanks, bins) instead of a raw primitive box.
/// `chamfer` is clamped so the cuts can never cross for any sane bin.
///
/// **This is the bevel law's operator**, and it was already here. A general "chamfer any convex
/// solid" pass was written against [`crate::chamfer`] and withdrawn after four narrowings: a
/// `ConvexSolid` carries no edge topology, so "which pairs of planes actually share an edge" is
/// not a question the representation can answer. Restricting the pass to unbroken six-plane boxes
/// made it build and it still produced a 4.19e-8 m2 sliver in the T-54's hull, because the corner
/// where three chamfer planes meet needs exactly the epsilon handling written below. For boxes
/// this function is the answer; for anything else the answer needs mesh-level edge adjacency,
/// which is a different piece of work and should start from that fact rather than rediscover it.
///
/// A chamfer of zero (or one finer than the corner-merge epsilon) asks for a PLAIN box, and
/// that is what it gets: each chamfer plane would otherwise pass exactly through a box edge,
/// leaving a two-corner face ring that `to_mesh` rejects as degenerate. "No chamfer" is a sane
/// request from a caller sweeping the parameter — not a build error.
pub fn chamfered_box(center: Vec3, half: Vec3, chamfer: f32) -> ConvexSolid {
    /// Below this the cut plane is indistinguishable from the edge it would trim: the corner
    /// dedup in `ConvexSolid::to_mesh` merges at 1e-4, so anything finer is not a chamfer.
    const MIN_CHAMFER: f32 = 1.0e-3;
    let c = chamfer.clamp(0.0, 0.45 * half.min_element());
    if c < MIN_CHAMFER {
        return ConvexSolid::box_at(center, half);
    }
    let sqrt2 = std::f32::consts::SQRT_2;
    let face = |n: Vec3, off: f32| Plane::new(n, off + n.dot(center));
    let mut planes = vec![
        face(Vec3::X, half.x),
        face(-Vec3::X, half.x),
        face(Vec3::Y, half.y),
        face(-Vec3::Y, half.y),
        face(Vec3::Z, half.z),
        face(-Vec3::Z, half.z),
    ];
    for sx in [-1.0_f32, 1.0] {
        // Top edges along Z and along X, then the vertical corner edges.
        planes.push(face(Vec3::new(sx, 1.0, 0.0) / sqrt2, (half.x + half.y - c) / sqrt2));
        planes.push(face(Vec3::new(0.0, 1.0, sx) / sqrt2, (half.z + half.y - c) / sqrt2));
        for sz in [-1.0_f32, 1.0] {
            planes.push(face(Vec3::new(sx, 0.0, sz) / sqrt2, (half.x + half.z - c) / sqrt2));
        }
    }
    ConvexSolid::new(planes)
}
