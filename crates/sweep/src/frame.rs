//! Transport frames along a swept path. The cross-section rides a `(u, v)` basis perpendicular to
//! the path tangent at every station; how that basis is chosen is the whole game. Parallel transport
//! minimises twist (no arbitrary roll on straight or inflected paths); a fixed-up frame keeps one
//! section axis pinned to a world direction (the right choice for a planar band like a track).

use glam::{Quat, Vec3};

/// A section basis at one path station: `u` and `v` span the cross-section plane.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    pub u: Vec3,
    pub v: Vec3,
}

/// Unit tangents at each path point: central differences for a closed loop, one-sided at open ends.
pub(crate) fn tangents(points: &[Vec3], closed: bool) -> Vec<Vec3> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let next = if closed { points[(i + 1) % n] } else { points[(i + 1).min(n - 1)] };
            let prev = if closed { points[(i + n - 1) % n] } else { points[i.saturating_sub(1)] };
            (next - prev).normalize_or_zero()
        })
        .collect()
}

/// Parallel-transport frames: start from an arbitrary normal, then rotate it by the minimal
/// rotation between successive tangents so it never spins about the path. For a closed loop the
/// residual seam twist is distributed evenly so the last frame meets the first.
pub(crate) fn parallel_transport(tangents: &[Vec3], closed: bool) -> Vec<Frame> {
    let mut u = first_normal(tangents[0]);
    let mut frames = vec![frame_from(tangents[0], u)];
    for i in 1..tangents.len() {
        u = rotate_between(tangents[i - 1], tangents[i], u);
        frames.push(frame_from(tangents[i], u));
    }
    if closed && tangents.len() > 2 {
        let closed_u = rotate_between(*tangents.last().unwrap(), tangents[0], u);
        let twist = signed_angle(frames[0].u, closed_u, tangents[0]);
        let n = tangents.len() as f32;
        for (i, frame) in frames.iter_mut().enumerate() {
            let fix = Quat::from_axis_angle(tangents[i], -twist * i as f32 / n);
            frame.u = fix * frame.u;
            frame.v = fix * frame.v;
        }
    }
    frames
}

/// Fixed-up frames: project `up` perpendicular to each tangent for `u`, with `v = tangent × u`.
pub(crate) fn fixed_up(tangents: &[Vec3], up: Vec3) -> Vec<Frame> {
    tangents
        .iter()
        .map(|&t| {
            let u = (up - t * up.dot(t)).normalize_or_zero();
            frame_from(t, u)
        })
        .collect()
}

fn frame_from(tangent: Vec3, u: Vec3) -> Frame {
    Frame { u, v: tangent.cross(u).normalize_or_zero() }
}

/// A unit vector perpendicular to `t`.
fn first_normal(t: Vec3) -> Vec3 {
    let seed = if t.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    (seed - t * seed.dot(t)).normalize_or_zero()
}

/// Rotate `v` by the minimal rotation that carries `from` onto `to`.
fn rotate_between(from: Vec3, to: Vec3, v: Vec3) -> Vec3 {
    let axis = from.cross(to);
    if axis.length() < 1.0e-6 {
        return v;
    }
    let angle = from.dot(to).clamp(-1.0, 1.0).acos();
    Quat::from_axis_angle(axis.normalize(), angle) * v
}

/// Signed angle from `a` to `b` measured about `axis`.
fn signed_angle(a: Vec3, b: Vec3, axis: Vec3) -> f32 {
    let cross = a.cross(b);
    cross.dot(axis).atan2(a.dot(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_up_keeps_u_along_the_projected_up() {
        // A straight path along Z with up = X keeps u exactly on X and v on the remaining axis.
        let frames = fixed_up(&[Vec3::Z, Vec3::Z], Vec3::X);
        assert!((frames[0].u - Vec3::X).length() < 1.0e-6);
        assert!(frames[0].v.dot(Vec3::Z).abs() < 1.0e-6);
    }

    #[test]
    fn parallel_transport_does_not_flip_across_an_s_bend() {
        // An S-shaped path in the XZ plane: parallel transport must keep the frame varying
        // continuously (no sudden 180-degree roll between neighbours).
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(1.0, 0.0, 3.0),
        ];
        let t = tangents(&pts, false);
        let frames = parallel_transport(&t, false);
        for pair in frames.windows(2) {
            assert!(pair[0].u.dot(pair[1].u) > 0.0, "frame u flipped between neighbours");
        }
    }
}
