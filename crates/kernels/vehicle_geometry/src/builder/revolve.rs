//! Revolve (lathe) surfaces: a profile swept around an axis, with optional end caps. Split from
//! the parent builder to stay reviewable; reaches the parent's private vertex/index buffers
//! directly, like the other builder submodules.

use glam::Vec3;

use super::{MeshBuilder, axis_vector, triangle_normal};
use crate::{Axis, GeometryVertex, ProfilePoint, RevolveSpec};

impl MeshBuilder {
    pub fn revolve(self, spec: RevolveSpec) -> Self {
        self.append_revolve(Vec3::ZERO, spec, false)
    }

    pub fn capped_revolve_at(self, origin: Vec3, spec: RevolveSpec) -> Self {
        self.append_revolve(origin, spec, true)
    }

    fn append_revolve(mut self, origin: Vec3, spec: RevolveSpec, cap_ends: bool) -> Self {
        assert!(spec.segments >= 3);
        assert!(spec.profile.len() >= 2);

        // A MACHINED ARRIS IS A LINE, NOT A BLEND. One vertex per (segment, profile point) means
        // one normal per point — averaged over every band that touches it — so a 90-degree
        // corner on a rim, flange or drum shaded like a fillet on every lathe in the fleet
        // (while the CLOSED profile's duplicated first point left exactly one corner
        // accidentally crisp: three soft corners and one hard on the same ring). Profile points
        // whose bands turn sharper than `HARD_BREAK_COS` get a duplicated row, so each band
        // keeps its own normal across the corner. Zero new triangles: no band is emitted
        // between a point and its duplicate.
        //
        // Apex rows (radius 0) never split — a cone's tip is one point, not a seam.
        const HARD_BREAK_COS: f32 = 0.766; // cos 40 deg
        let n = spec.profile.len();
        let band_dir = |a: ProfilePoint, b: ProfilePoint| {
            let d = glam::Vec2::new(b.offset - a.offset, b.radius - a.radius);
            d.normalize_or_zero()
        };
        let mut splits = vec![false; n];
        for i in 1..n - 1 {
            if spec.profile[i].radius <= f32::EPSILON {
                continue;
            }
            let before = band_dir(spec.profile[i - 1], spec.profile[i]);
            let after = band_dir(spec.profile[i], spec.profile[i + 1]);
            splits[i] = before.dot(after) < HARD_BREAK_COS;
        }
        // Expanded rows: each split point occupies two rows at the same position. `top_row[k]`
        // is the row band k starts from (the copy facing it); `bottom_row[k]` the row it ends
        // on.
        let mut row_of = Vec::with_capacity(n);
        let mut expanded: Vec<ProfilePoint> = Vec::with_capacity(n + 4);
        for (i, point) in spec.profile.iter().enumerate() {
            row_of.push(expanded.len() as u32);
            expanded.push(*point);
            if splits[i] {
                expanded.push(*point);
            }
        }
        let profile_len = expanded.len() as u32;

        let base = self.vertices.len() as u32;
        for segment in 0..spec.segments {
            let angle = (segment as f32 / spec.segments as f32) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let radial = radial_for(spec.axis, cos, sin);
            for point in &expanded {
                let position = origin + position_for(spec.axis, radial, *point);
                self.vertices.push(GeometryVertex::new(
                    position,
                    radial,
                    spec.material,
                    spec.smoothing,
                ));
            }
        }

        let surface_index_start = self.indices.len();
        // Emit the whole lathe with ONE winding. Every quad follows the same (segment, row)
        // traversal, so the surface is consistently wound by construction: each interior edge is
        // walked once in each direction, and no band can disagree with its neighbours.
        //
        // It did not used to work that way. Each quad was oriented independently, by asking
        // whether its normal pointed AWAY FROM THE AXIS — which is only true for a band on the
        // OUTSIDE of the lathe. A ring's inner wall faces the axis, and a flat annulus faces
        // neither way, so those bands were flipped (or coin-flipped) relative to the rest and
        // every seam between them shipped inconsistently wound. That was 22 broken edges per
        // steel ring, on every road wheel, tyre, roller and drum in the fleet.
        for segment in 0..spec.segments as u32 {
            let next = (segment + 1) % spec.segments as u32;
            for band in 0..n - 1 {
                // The band starts on its point's LAST copy and ends on the next point's FIRST:
                // the duplicates between belong to the corner, not to any band.
                let top = row_of[band] + splits[band] as u32;
                let bottom = row_of[band + 1];
                let a = base + segment * profile_len + top;
                let b = base + next * profile_len + top;
                let c = base + next * profile_len + bottom;
                let d = base + segment * profile_len + bottom;
                self.indices.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }
        let surface_index_end = self.indices.len();
        // Now decide, ONCE for the whole surface, whether that winding faces out. The vote is
        // area-weighted (the cross product is not normalised), so the lathe's dominant faces —
        // the outer wall of a ring, the tread of a tyre — carry the decision and the minority
        // bands follow them instead of flipping alone.
        self.orient_revolve_outward(origin, &spec, surface_index_start, surface_index_end);
        self.rebuild_surface_normals(
            base,
            spec.segments * profile_len as usize,
            surface_index_start,
            surface_index_end,
        );
        if cap_ends {
            // Which way the profile actually TRAVELS along the axis, not which way we hope it
            // does. A profile may run backwards — the gun's bore funnel recedes from the muzzle
            // face INTO the tube — and a cap normal taken from the axis alone then points into
            // the solid at both ends. Both fans then wind inward (back-facing, so the pipeline's
            // `cull_mode: Back` drops them), and once `weld_and_smooth` fuses their rims with the
            // wall — a smooth group leaves the normal out of the weld key — every rim edge is
            // traversed twice the same way. That shipped on seven of eight guns while every
            // ratio, budget and silhouette gate stayed green.
            let first = spec.profile[0].offset;
            let last = spec.profile[spec.profile.len() - 1].offset;
            let forward = if last >= first { 1.0 } else { -1.0 };
            let axial = axis_vector(spec.axis) * forward;
            self.push_revolve_cap(origin, &spec, base, profile_len, 0, -axial);
            self.push_revolve_cap(origin, &spec, base, profile_len, profile_len - 1, axial);
        }
        self
    }

    fn rebuild_surface_normals(
        &mut self,
        base: u32,
        vertex_count: usize,
        index_start: usize,
        index_end: usize,
    ) {
        let mut sums = vec![Vec3::ZERO; vertex_count];
        for tri in self.indices[index_start..index_end].chunks_exact(3) {
            let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let normal = triangle_normal(
                self.vertices[a].position,
                self.vertices[b].position,
                self.vertices[c].position,
            )
            .normalize_or_zero();
            for &index in tri {
                sums[(index - base) as usize] += normal;
            }
        }
        for (offset, sum) in sums.into_iter().enumerate() {
            let normal = sum.normalize_or_zero();
            if normal != Vec3::ZERO {
                self.vertices[base as usize + offset].normal = normal;
            }
        }
    }

    /// Flip a freshly-emitted lathe surface if its consistent winding came out facing INWARD.
    ///
    /// The test is the area-weighted agreement between each triangle's normal and the direction
    /// away from the revolve axis at that triangle. A lathe that is mostly outer wall votes
    /// positive and is left alone; one emitted back-to-front votes negative and every triangle in
    /// the range is reversed together — which keeps the winding consistent either way.
    fn orient_revolve_outward(
        &mut self,
        origin: Vec3,
        spec: &RevolveSpec,
        index_start: usize,
        index_end: usize,
    ) {
        let axis = axis_vector(spec.axis);
        let mut vote = 0.0;
        for tri in self.indices[index_start..index_end].chunks_exact(3) {
            let [a, b, c] = [
                self.vertices[tri[0] as usize].position,
                self.vertices[tri[1] as usize].position,
                self.vertices[tri[2] as usize].position,
            ];
            let relative = (a + b + c) / 3.0 - origin;
            let outward = relative - axis * relative.dot(axis);
            vote += triangle_normal(a, b, c).dot(outward);
        }
        // A profile that never leaves its own plane (a flat annulus) has no radial answer at all:
        // the surface is one-sided and either winding is consistent. Fall back to the direction
        // the profile travels along the axis, the same tie-break the end caps use.
        if vote.abs() < 1.0e-12 {
            let first = spec.profile[0].offset;
            let last = spec.profile[spec.profile.len() - 1].offset;
            if last >= first {
                return;
            }
        } else if vote > 0.0 {
            return;
        }
        for tri in self.indices[index_start..index_end].chunks_exact_mut(3) {
            tri.swap(1, 2);
        }
    }

    fn push_revolve_cap(
        &mut self,
        origin: Vec3,
        spec: &RevolveSpec,
        base: u32,
        // The EXPANDED row stride and row index: hard-break duplicates widen the lathe's rows,
        // and a cap reading the original profile length would fan around the wrong ring.
        profile_len: u32,
        row: u32,
        normal: Vec3,
    ) {
        let end = if row == 0 { 0 } else { spec.profile.len() - 1 };
        let center = origin + axis_vector(spec.axis) * spec.profile[end].offset;
        let center_index = self.vertices.len() as u32;
        self.vertices.push(GeometryVertex::new(center, normal, spec.material, spec.smoothing));
        // The cap gets its own rim vertices carrying the axial cap normal, not the side wall's radial
        // normals — sharing the wall verts shaded a flat end disc as a dome (axial→radial fan).
        let rim_base = self.vertices.len() as u32;
        let segments = spec.segments as u32;
        for segment in 0..segments {
            let position = self.vertices[(base + segment * profile_len + row) as usize].position;
            self.vertices.push(GeometryVertex::new(
                position,
                normal,
                spec.material,
                spec.smoothing,
            ));
        }
        let flip = triangle_normal(
            center,
            self.vertices[rim_base as usize].position,
            self.vertices[rim_base as usize + 1].position,
        )
        .dot(normal)
            < 0.0;
        for segment in 0..segments {
            let (current, next) = (rim_base + segment, rim_base + (segment + 1) % segments);
            let tri =
                if flip { [center_index, next, current] } else { [center_index, current, next] };
            self.indices.extend_from_slice(&tri);
        }
    }
}

fn radial_for(axis: Axis, cos: f32, sin: f32) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(0.0, cos, sin),
        Axis::Y => Vec3::new(cos, 0.0, sin),
        Axis::Z => Vec3::new(cos, sin, 0.0),
    }
}

fn position_for(axis: Axis, radial: Vec3, point: ProfilePoint) -> Vec3 {
    radial * point.radius
        + match axis {
            Axis::X => Vec3::X * point.offset,
            Axis::Y => Vec3::Y * point.offset,
            Axis::Z => Vec3::Z * point.offset,
        }
}
