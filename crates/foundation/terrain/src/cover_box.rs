//! ONE box (the one program's X1). Every solid was an axis-aligned box read fifteen different
//! ways — the SAT, the shell slab, the sight slab, the bake, the camera, the minimap, the map
//! report, the editor, the atlas — each with its own `center ± half` arithmetic and none of
//! them with a yaw. `CoverBox` is the box as geometry: a centre, half extents and a yaw about
//! +Y (glam's `from_rotation_y` convention, the hull's own), with the local frame, the corners,
//! the plan bounds, the point test and the segment slab that every reader takes from here.
//!
//! The yaw-0 path is the OLD arithmetic, expression for expression: an axis-aligned box must
//! resolve bit for bit as it did before the field existed (the replay locks stand on that), so
//! the rotated branch is taken only when the yaw is not exactly zero.

use crate::StaticCoverObject;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverBox {
    pub center: [f32; 3],
    pub half: [f32; 3],
    /// Rotation about +Y, radians, `glam::Mat3::from_rotation_y` convention: the box's local
    /// +X points at world `(cos, 0, -sin)`, its local +Z at `(sin, 0, cos)`.
    pub yaw_rad: f32,
}

impl CoverBox {
    pub fn of(object: &StaticCoverObject) -> Self {
        Self { center: object.center, half: object.half_extents_m, yaw_rad: object.yaw_rad }
    }

    pub fn axis_aligned(center: [f32; 3], half: [f32; 3]) -> Self {
        Self { center, half, yaw_rad: 0.0 }
    }

    pub fn is_axis_aligned(&self) -> bool {
        self.yaw_rad == 0.0
    }

    fn cos_sin(&self) -> (f32, f32) {
        (self.yaw_rad.cos(), self.yaw_rad.sin())
    }

    /// A world point in the box's frame (the centre at the origin, the box axis-aligned).
    pub fn to_local(&self, p: [f32; 3]) -> [f32; 3] {
        let dx = p[0] - self.center[0];
        let dy = p[1] - self.center[1];
        let dz = p[2] - self.center[2];
        if self.is_axis_aligned() {
            return [dx, dy, dz];
        }
        let (c, s) = self.cos_sin();
        // R_y(-yaw): the inverse of the rotation the box wears.
        [dx * c - dz * s, dy, dx * s + dz * c]
    }

    /// A box-frame point back in the world.
    pub fn to_world(&self, l: [f32; 3]) -> [f32; 3] {
        if self.is_axis_aligned() {
            return [self.center[0] + l[0], self.center[1] + l[1], self.center[2] + l[2]];
        }
        let (c, s) = self.cos_sin();
        [
            self.center[0] + (l[0] * c + l[2] * s),
            self.center[1] + l[1],
            self.center[2] + (-l[0] * s + l[2] * c),
        ]
    }

    /// A box-frame direction in the world (no translation).
    pub fn rotate_to_world(&self, v: [f32; 3]) -> [f32; 3] {
        if self.is_axis_aligned() {
            return v;
        }
        let (c, s) = self.cos_sin();
        [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
    }

    /// The four plan corners, world XZ, counter-clockwise seen from above.
    pub fn corners_xz(&self) -> [[f32; 2]; 4] {
        let (hx, hz) = (self.half[0], self.half[2]);
        [[-hx, -hz], [hx, -hz], [hx, hz], [-hx, hz]].map(|[lx, lz]| {
            let w = self.to_world([lx, 0.0, lz]);
            [w[0], w[2]]
        })
    }

    /// The plan-axis-aligned bounds `[x0, z0, x1, z1]` the rotated footprint fits in — the
    /// broadphase every reader used to compute as `center ± half`, and still does at yaw 0.
    pub fn bounds_xz(&self) -> [f32; 4] {
        if self.is_axis_aligned() {
            return [
                self.center[0] - self.half[0],
                self.center[2] - self.half[2],
                self.center[0] + self.half[0],
                self.center[2] + self.half[2],
            ];
        }
        let corners = self.corners_xz();
        let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        for [x, z] in corners {
            b[0] = b[0].min(x);
            b[1] = b[1].min(z);
            b[2] = b[2].max(x);
            b[3] = b[3].max(z);
        }
        b
    }

    /// The circle around the plan footprint, whatever the yaw.
    pub fn circumradius_xz(&self) -> f32 {
        (self.half[0] * self.half[0] + self.half[2] * self.half[2]).sqrt()
    }

    /// Whether a world point is inside the plan footprint, grown by `skin`.
    pub fn contains_xz(&self, x: f32, z: f32, skin: f32) -> bool {
        if self.is_axis_aligned() {
            return (x - self.center[0]).abs() <= self.half[0] + skin
                && (z - self.center[2]).abs() <= self.half[2] + skin;
        }
        let l = self.to_local([x, 0.0, z]);
        l[0].abs() <= self.half[0] + skin && l[2].abs() <= self.half[2] + skin
    }

    /// Whether a world point is inside the box, grown by `skin` on every axis.
    pub fn contains(&self, p: [f32; 3], skin: f32) -> bool {
        if self.is_axis_aligned() {
            return (0..3)
                .all(|axis| (p[axis] - self.center[axis]).abs() <= self.half[axis] + skin);
        }
        let l = self.to_local(p);
        (0..3).all(|axis| l[axis].abs() <= self.half[axis] + skin)
    }

    /// The broadphase: whether the segment's plan bounds miss the footprint's bounds grown by
    /// `extra`. At yaw 0 this is the `segment_xz_disjoint` arithmetic every reader used.
    pub fn xz_disjoint_from_segment(&self, from: [f32; 3], to: [f32; 3], extra: f32) -> bool {
        let [x0, z0, x1, z1] = self.bounds_xz();
        let (min_x, max_x) = if from[0] <= to[0] { (from[0], to[0]) } else { (to[0], from[0]) };
        if x0 - extra > max_x || x1 + extra < min_x {
            return true;
        }
        let (min_z, max_z) = if from[2] <= to[2] { (from[2], to[2]) } else { (to[2], from[2]) };
        z0 - extra > max_z || z1 + extra < min_z
    }

    /// The `[t_min, t_max]` sub-interval of the segment inside the box grown by `radius` (the
    /// slab method), or `None` on a miss. At yaw 0 the old world-frame slab, expression for
    /// expression; rotated, the same slab in the box's frame.
    pub fn segment_interval(
        &self,
        from: [f32; 3],
        to: [f32; 3],
        radius: f32,
    ) -> Option<(f32, f32)> {
        let (origin, dir): ([f32; 3], [f32; 3]);
        let (lo, hi): ([f32; 3], [f32; 3]);
        if self.is_axis_aligned() {
            origin = from;
            dir = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
            lo = [
                self.center[0] - (self.half[0] + radius),
                self.center[1] - (self.half[1] + radius),
                self.center[2] - (self.half[2] + radius),
            ];
            hi = [
                self.center[0] + (self.half[0] + radius),
                self.center[1] + (self.half[1] + radius),
                self.center[2] + (self.half[2] + radius),
            ];
        } else {
            origin = self.to_local(from);
            let end = self.to_local(to);
            dir = [end[0] - origin[0], end[1] - origin[1], end[2] - origin[2]];
            lo = [-(self.half[0] + radius), -(self.half[1] + radius), -(self.half[2] + radius)];
            hi = [self.half[0] + radius, self.half[1] + radius, self.half[2] + radius];
        }
        let (mut t_min, mut t_max) = (0.0f32, 1.0f32);
        for axis in 0..3 {
            let d = dir[axis];
            if d.abs() < 1.0e-6 {
                if origin[axis] < lo[axis] || origin[axis] > hi[axis] {
                    return None;
                }
            } else {
                let mut t1 = (lo[axis] - origin[axis]) / d;
                let mut t2 = (hi[axis] - origin[axis]) / d;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                t_min = t_min.max(t1);
                t_max = t_max.min(t2);
                if t_min > t_max {
                    return None;
                }
            }
        }
        Some((t_min, t_max))
    }

    /// The point ON the box's surface nearest to a world point (the box not grown): the
    /// contact a swept sphere leaves.
    pub fn clamp_to_surface(&self, p: [f32; 3]) -> [f32; 3] {
        if self.is_axis_aligned() {
            return [
                p[0].clamp(self.center[0] - self.half[0], self.center[0] + self.half[0]),
                p[1].clamp(self.center[1] - self.half[1], self.center[1] + self.half[1]),
                p[2].clamp(self.center[2] - self.half[2], self.center[2] + self.half[2]),
            ];
        }
        let l = self.to_local(p);
        self.to_world([
            l[0].clamp(-self.half[0], self.half[0]),
            l[1].clamp(-self.half[1], self.half[1]),
            l[2].clamp(-self.half[2], self.half[2]),
        ])
    }
}

/// Rotate `yaw_rad` about +Y (the box convention), for readers that turn their own geometry.
pub fn rotate_y(v: [f32; 3], yaw_rad: f32) -> [f32; 3] {
    if yaw_rad == 0.0 {
        return v;
    }
    let (c, s) = (yaw_rad.cos(), yaw_rad.sin());
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_unit(seed: &mut u64) -> f32 {
        *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// X1: at yaw 0 the box is the old arithmetic bit for bit — the local frame is the plain
    /// difference, the bounds are `center ± half`, the slab is the world-frame slab.
    #[test]
    fn the_axis_aligned_box_is_the_old_arithmetic_bit_for_bit() {
        let b = CoverBox::axis_aligned([12.3, 4.5, -7.25], [3.1, 2.0, 5.7]);
        let mut seed = 3u64;
        for _ in 0..200 {
            let p = [
                hash_unit(&mut seed) * 40.0,
                hash_unit(&mut seed) * 9.0,
                hash_unit(&mut seed) * 40.0 - 20.0,
            ];
            let l = b.to_local(p);
            assert_eq!(l, [p[0] - b.center[0], p[1] - b.center[1], p[2] - b.center[2]]);
            assert_eq!(
                b.contains_xz(p[0], p[2], 0.0),
                (p[0] - b.center[0]).abs() <= b.half[0] && (p[2] - b.center[2]).abs() <= b.half[2]
            );
        }
        assert_eq!(b.bounds_xz(), [12.3 - 3.1, -7.25 - 5.7, 12.3 + 3.1, -7.25 + 5.7]);
    }

    /// X1: a yawed box is the same box turned. Its corners land where the rotation puts them,
    /// the local frame round-trips, a point test agrees with the rotated corners' half-planes,
    /// and the segment slab agrees with a fine march of the point test.
    #[test]
    fn a_yawed_box_is_the_same_box_turned() {
        let b = CoverBox { center: [50.0, 3.0, 40.0], half: [8.0, 3.0, 3.0], yaw_rad: 0.6 };
        let corners = b.corners_xz();
        let (c, s) = (0.6f32.cos(), 0.6f32.sin());
        // local +X (hx, 0) lands at (hx*c, -hx*s): glam's from_rotation_y.
        let px = b.to_world([8.0, 0.0, 0.0]);
        assert!((px[0] - (50.0 + 8.0 * c)).abs() < 1e-5 && (px[2] - (40.0 - 8.0 * s)).abs() < 1e-5);
        let mut seed = 11u64;
        for _ in 0..500 {
            let p = [30.0 + hash_unit(&mut seed) * 40.0, 3.0, 20.0 + hash_unit(&mut seed) * 40.0];
            let l = b.to_local(p);
            let back = b.to_world(l);
            assert!((back[0] - p[0]).abs() < 1e-4 && (back[2] - p[2]).abs() < 1e-4, "round trip");
            // Inside iff on the inner side of all four edges (cross products).
            let inside = (0..4).all(|i| {
                let a = corners[i];
                let bq = corners[(i + 1) % 4];
                let cross = (bq[0] - a[0]) * (p[2] - a[1]) - (bq[1] - a[1]) * (p[0] - a[0]);
                cross >= -1e-4
            });
            assert_eq!(b.contains_xz(p[0], p[2], 0.0), inside, "{p:?}");
        }
        let bounds = b.bounds_xz();
        for [x, z] in corners {
            assert!(
                x >= bounds[0] - 1e-5
                    && x <= bounds[2] + 1e-5
                    && z >= bounds[1] - 1e-5
                    && z <= bounds[3] + 1e-5
            );
        }
        assert!(bounds[2] - bounds[0] > 16.0, "the turned box's bounds outgrow its length");
        for _ in 0..200 {
            let from =
                [20.0 + hash_unit(&mut seed) * 60.0, 3.0, 10.0 + hash_unit(&mut seed) * 60.0];
            let to = [20.0 + hash_unit(&mut seed) * 60.0, 3.0, 10.0 + hash_unit(&mut seed) * 60.0];
            let slab = b.segment_interval(from, to, 0.0).is_some();
            let march = (0..=400).any(|i| {
                let t = i as f32 / 400.0;
                b.contains_xz(
                    from[0] + (to[0] - from[0]) * t,
                    from[2] + (to[2] - from[2]) * t,
                    1e-3,
                )
            });
            assert_eq!(slab, march, "slab vs march from {from:?} to {to:?}");
            assert!(
                !(b.xz_disjoint_from_segment(from, to, 0.0) && slab),
                "the broadphase never hides a hit"
            );
        }
        let on = b.clamp_to_surface([50.0 + 20.0 * c, 3.0, 40.0 - 20.0 * s]);
        let l = b.to_local(on);
        assert!((l[0] - 8.0).abs() < 1e-4 && l[2].abs() < 1e-4, "clamped onto the turned end");
    }
}
