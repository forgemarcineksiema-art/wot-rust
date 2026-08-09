//! Hala 2.0 T1: the hangar's build-time GI bake — one bounce of directed light plus emissive
//! output, written into the `SceneVertex::bounce` lane the scene shader adds onto its lit
//! result. CPU-side and deterministic (fixed sample spiral, no clock, no RNG), so the golden
//! harness locks it like any other picture-forming code.
//!
//! Why it exists: the hall had no indirect light at all. The frosted panes and lamp faces are
//! authored emitters (any colour channel past 1.0 — the same convention `bake_corner_shade`
//! honours), yet the wall under a pane received not a photon of it, the skylight shafts on the
//! floor bounced nothing back up, and D20 recorded the result: 0.3% of the hero frame bright.
//! A static room with a static rig is the textbook case for baking: all the cost at build
//! time, zero at runtime.
//!
//! Two stages:
//! 1. **Conformal subdivision** (`subdivide_for_bake`): the shell is authored as huge slabs
//!    whose faces carry vertices only at their corners, so a per-vertex bake would smear into
//!    one flat wash — the same reason `bake_corner_shade` excludes the shell. Longest-edge
//!    bisection WITH propagation (every triangle sharing the split edge divides with it) keeps
//!    the mesh crack-free while giving the bake a canvas fine enough to hold a pool of light.
//! 2. **Gather** (`bake_bounce`): for every vertex, a fixed cosine-weighted hemisphere of rays.
//!    A ray that hits geometry brings back the hit surface's DIRECT light — sun through the
//!    real skylight openings (shadow decided by a second ray toward the key), the worklamp
//!    pools, and emissive faces at their authored radiance — scaled by the hit's albedo. The
//!    result, premultiplied by the receiving vertex's own albedo, is the bounce lane; emitter
//!    vertices additionally carry their own emission so a pane GLOWS instead of merely being
//!    pale, which is what the garage's bloom chain picks up.

use glam::Vec3;
use renderer_api::{SceneLighting, SceneVertex};

/// Faces with any edge longer than this are subdivided before the bake. B2 densification,
/// priced by measurement against the ≤1 s release prewarm budget (same thermal state,
/// back-to-back runs; the machine drifts ±15% hot-to-cold):
///
///   2.2 m / 16 rays:  0.99 s, 20 244 verts   (the pre-B2 state — already AT the budget;
///                                             the A1 nave doubled the old hall's 531 ms)
///   1.6 m / 16 rays:  0.98 s, 21 378 verts   <- SHIPPED: spatial density is nearly free
///   1.4 m / 16 rays:  1.18 s, 21 888 verts   (over)
///   1.4 m / 32 rays:  2.06 s                 (the plan's headline target: costs a doubling
///                                             the gather has not earned — deferred, G-debt)
///
/// The surprise the measurement bought: edge density is cheap (the subdividable panels are
/// few), RAY count is the whole bill. The per-triangle direct-light cache (see `TriDirect`)
/// paid for this densification: it took the gather from 923 ms to ~810 ms at the old density.
const MAX_EDGE_M: f32 = 1.6;
/// Which triangles earn bake resolution: PANELS the room can see. Two filters, both learned
/// from a 140k-triangle explosion and a broken vertex-proxy test:
///
/// - **panel, not lath**: both shorter edges over a metre. A crane rail is 36 m long and
///   0.1 m wide — subdividing it buys no light gradient and (because the orbit-clearance
///   test reads VERTICES as its proxy for geometry) plants midpoints inside volumes the
///   authored geometry only crosses;
/// - **faces the room**: a triangle whose inward normal offset leaves the hall's interior is
///   the outside/underside of a closed slab and can never show its bounce.
fn earns_bake_resolution(vertices: &[SceneVertex], tri: &[u32]) -> bool {
    let p: Vec<Vec3> =
        tri.iter().map(|&i| Vec3::from_array(vertices[i as usize].position)).collect();
    let mut edges = [(p[1] - p[0]).length(), (p[2] - p[1]).length(), (p[0] - p[2]).length()];
    edges.sort_by(|a, b| a.partial_cmp(b).expect("finite edges"));
    if edges[0].min(edges[1]) < 1.0 {
        return false;
    }
    let centre = (p[0] + p[1] + p[2]) / 3.0;
    let normal = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
    let probe = centre + normal * 0.08;
    // The hall interior, DERIVED from the hangar's own constants with a slab's thickness of
    // forgiveness — this used to hard-code the old square hall (18.1/12.65), and a hall that
    // outgrew the literals would have silently stopped subdividing its far walls.
    probe.x.abs() < crate::hangar::HALF_X + 0.1
        && probe.z.abs() < crate::hangar::HALF_Z + 0.1
        && probe.y > -0.05
        && probe.y < crate::hangar::SHED_RIDGE + 0.05
}
/// Hemisphere rays per vertex. 16 cosine-weighted directions; banding hides in the vertex
/// interpolation. The plan's 32 was measured at 2.06 s against the ≤1 s prewarm budget (see
/// the table on [`MAX_EDGE_M`]) and stays deferred until the gather earns a genuine 2×.
const RAY_COUNT: usize = 16;
/// One-bounce gain: how much of the gathered radiance survives into the lane. Slightly under
/// 1.0 stands in for the energy the single bounce cannot conserve exactly.
const BOUNCE_GAIN: f32 = 0.85;
/// The emission scale: authored HDR colour → radiated HDR, applied BOTH to an emitter's own
/// lane and to what a gather ray brings back from hitting one, so the pane the eye sees and
/// the pane the wall sees are the same source.
///
/// The first value here was 0.55 on the self term only, and the user's verdict was earned:
/// "ta poświata z okien jest beznadziejna... ma z czego w ogóle?" It did not. A pane radiated
/// ~0.8 HDR — an office monitor, not daylight — and the garage's bloom composite is
/// `hdr + blurred × weight`, so at weight 0.03 the halo measured ~0.02 on screen: nothing.
/// Real daylight glazing against a dim interior runs several times the room's luminance. At
/// 2.2 a pane leaves the bake at ~3.5 HDR, which the tone curve rolls to a hot white and the
/// bloom chain turns into a visible soft halo over the near-black upper band.
const EMISSION_BOOST: f32 = 2.2;
/// The emitter convention shared with `bake_corner_shade`: any channel past 1.0 is a hot face.
fn is_emitter(color: [f32; 3]) -> bool {
    color.iter().any(|c| *c > 1.0)
}

/// Subdivide every triangle until no edge exceeds `MAX_EDGE_M`, splitting by the longest edge
/// and propagating the split to every triangle that shares it — conformal, so the render mesh
/// stays crack-free. All vertex lanes interpolate linearly at the midpoint.
fn subdivide_for_bake(vertices: &mut Vec<SceneVertex>, indices: &mut Vec<u32>) {
    let midpoint = |a: &SceneVertex, b: &SceneVertex| -> SceneVertex {
        let lerp3 = |x: [f32; 3], y: [f32; 3]| {
            [(x[0] + y[0]) * 0.5, (x[1] + y[1]) * 0.5, (x[2] + y[2]) * 0.5]
        };
        SceneVertex {
            position: lerp3(a.position, b.position),
            normal: lerp3(a.normal, b.normal),
            color: lerp3(a.color, b.color),
            tint_weight: (a.tint_weight + b.tint_weight) * 0.5,
            gloss: (a.gloss + b.gloss) * 0.5,
            surface: a.surface,
            sway: (a.sway + b.sway) * 0.5,
            uv: [(a.uv[0] + b.uv[0]) * 0.5, (a.uv[1] + b.uv[1]) * 0.5],
            bounce: [0.0; 3],
        }
    };
    let edge_len = |v: &[SceneVertex], a: u32, b: u32| {
        (Vec3::from_array(v[a as usize].position) - Vec3::from_array(v[b as usize].position))
            .length()
    };

    // Iterate to a fixed point: split the longest over-limit edge of every triangle, reusing
    // one midpoint per edge so shared edges split identically (conformity).
    loop {
        let mut midpoints: std::collections::HashMap<(u32, u32), u32> =
            std::collections::HashMap::new();
        let mut next: Vec<u32> = Vec::with_capacity(indices.len());
        let mut any_split = false;
        for tri in indices.chunks_exact(3) {
            let (a, b, c) = (tri[0], tri[1], tri[2]);
            let edges = [(a, b, c), (b, c, a), (c, a, b)];
            let longest = edges
                .into_iter()
                .max_by(|x, y| {
                    edge_len(vertices, x.0, x.1)
                        .partial_cmp(&edge_len(vertices, y.0, y.1))
                        .expect("finite edge lengths")
                })
                .expect("triangle has edges");
            if edge_len(vertices, longest.0, longest.1) <= MAX_EDGE_M
                || !earns_bake_resolution(vertices, tri)
            {
                next.extend_from_slice(tri);
                continue;
            }
            any_split = true;
            let key = (longest.0.min(longest.1), longest.0.max(longest.1));
            let mid = *midpoints.entry(key).or_insert_with(|| {
                let m = midpoint(&vertices[longest.0 as usize], &vertices[longest.1 as usize]);
                vertices.push(m);
                (vertices.len() - 1) as u32
            });
            // Split (p, q, r) across mid(p, q): two triangles that keep the winding.
            let (p, q, r) = longest;
            next.extend_from_slice(&[p, mid, r]);
            next.extend_from_slice(&[mid, q, r]);
        }
        *indices = next;
        if !any_split {
            break;
        }
    }
}

/// A flat BVH over the mesh triangles (median split on the longest centroid axis), enough to
/// take the gather from quadratic to interactive. Node children are `left`/`right` indices
/// into the same arena; leaves hold triangle ranges of `order`.
struct Bvh {
    nodes: Vec<Node>,
    /// Triangle indices (into `tris`), reordered by the build.
    order: Vec<u32>,
    tris: Vec<[Vec3; 3]>,
}

struct Node {
    min: Vec3,
    max: Vec3,
    /// Leaf: `count > 0` and `start` indexes `order`. Inner: `count == 0`, `start` is the
    /// left child's node index and `right` the right child's.
    start: u32,
    count: u32,
    right: u32,
}

impl Bvh {
    fn build(vertices: &[SceneVertex], indices: &[u32]) -> Self {
        let tris: Vec<[Vec3; 3]> = indices
            .chunks_exact(3)
            .map(|t| {
                [
                    Vec3::from_array(vertices[t[0] as usize].position),
                    Vec3::from_array(vertices[t[1] as usize].position),
                    Vec3::from_array(vertices[t[2] as usize].position),
                ]
            })
            .collect();
        let mut order: Vec<u32> = (0..tris.len() as u32).collect();
        let mut nodes = Vec::new();
        Self::split(&tris, &mut order, &mut nodes, 0, tris.len());
        Self { nodes, order, tris }
    }

    fn split(
        tris: &[[Vec3; 3]],
        order: &mut [u32],
        nodes: &mut Vec<Node>,
        start: usize,
        end: usize,
    ) -> u32 {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &t in &order[start..end] {
            for p in tris[t as usize] {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let index = nodes.len() as u32;
        nodes.push(Node { min, max, start: start as u32, count: (end - start) as u32, right: 0 });
        if end - start <= 8 {
            return index;
        }
        let extent = max - min;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };
        let centroid = |t: u32| {
            let tri = tris[t as usize];
            (tri[0][axis] + tri[1][axis] + tri[2][axis]) / 3.0
        };
        order[start..end]
            .sort_by(|&x, &y| centroid(x).partial_cmp(&centroid(y)).expect("finite centroids"));
        let mid = (start + end) / 2;
        let left = Self::split(tris, order, nodes, start, mid);
        let right = Self::split(tris, order, nodes, mid, end);
        nodes[index as usize].start = left;
        nodes[index as usize].count = 0;
        nodes[index as usize].right = right;
        index
    }

    /// Nearest hit along `dir` from `origin`, as (distance, triangle index).
    ///
    /// `stack` is the caller's traversal scratch, cleared on entry. It is a parameter rather
    /// than a local because this runs ~284 000 times per bake and a `vec![0u32]` per call is
    /// that many heap allocations — a third of the gather's wall time went into the allocator
    /// before the buffer was hoisted to the worker.
    /// A node's slab-test entry distance, or +âž if the ray misses its box entirely.
    fn entry_lo(&self, node_index: u32, origin: Vec3, inv: Vec3) -> f32 {
        let node = &self.nodes[node_index as usize];
        let t0 = (node.min - origin) * inv;
        let t1 = (node.max - origin) * inv;
        let lo = t0.min(t1).max_element().max(0.0);
        let hi = t0.max(t1).min_element();
        if lo > hi { f32::INFINITY } else { lo }
    }

    fn nearest_hit(&self, origin: Vec3, dir: Vec3, stack: &mut Vec<u32>) -> Option<(f32, u32)> {
        let inv = dir.recip();
        stack.clear();
        stack.push(0u32);
        let mut best: Option<(f32, u32)> = None;
        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index as usize];
            // Slab test against the node box.
            let t0 = (node.min - origin) * inv;
            let t1 = (node.max - origin) * inv;
            let lo = t0.min(t1).max_element().max(0.0);
            let hi = t0.max(t1).min_element();
            if lo > hi || best.is_some_and(|(d, _)| d < lo) {
                continue;
            }
            if node.count > 0 {
                for &t in &self.order[node.start as usize..(node.start + node.count) as usize] {
                    if let Some(d) = bake_ray_hits_triangle(origin, dir, self.tris[t as usize])
                        && best.is_none_or(|(bd, _)| d < bd)
                    {
                        best = Some((d, t));
                    }
                }
            } else {
                // NEAR CHILD FIRST: push the farther child under the nearer one, so the
                // nearer pops first and `best` tightens before the far subtree is examined —
                // which is what lets the `d < lo` prune above actually fire. Unordered
                // children left the prune mostly idle and the gather paid full traversals.
                let (a, b) = (node.start, node.right);
                if self.entry_lo(a, origin, inv) <= self.entry_lo(b, origin, inv) {
                    stack.push(b);
                    stack.push(a);
                } else {
                    stack.push(a);
                    stack.push(b);
                }
            }
        }
        best
    }

    /// Whether anything blocks the ray from `origin` along `dir` — any-hit with an early out,
    /// which is what makes the sun-shadow half of the gather cheap. Takes the caller's
    /// traversal scratch for the reason [`Self::nearest_hit`] states.
    fn occluded(&self, origin: Vec3, dir: Vec3, stack: &mut Vec<u32>) -> bool {
        let inv = dir.recip();
        stack.clear();
        stack.push(0u32);
        while let Some(node_index) = stack.pop() {
            let node = &self.nodes[node_index as usize];
            let t0 = (node.min - origin) * inv;
            let t1 = (node.max - origin) * inv;
            let lo = t0.min(t1).max_element().max(0.0);
            let hi = t0.max(t1).min_element();
            if lo > hi {
                continue;
            }
            if node.count > 0 {
                for &t in &self.order[node.start as usize..(node.start + node.count) as usize] {
                    if bake_ray_hits_triangle(origin, dir, self.tris[t as usize]).is_some() {
                        return true;
                    }
                }
            } else {
                stack.push(node.start);
                stack.push(node.right);
            }
        }
        false
    }
}

/// Möller–Trumbore, front-and-back faces alike (the hall's slabs are thin closed boxes).
fn bake_ray_hits_triangle(origin: Vec3, dir: Vec3, tri: [Vec3; 3]) -> Option<f32> {
    const EPS: f32 = 1.0e-6;
    let e1 = tri[1] - tri[0];
    let e2 = tri[2] - tri[0];
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < EPS {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - tri[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > EPS).then_some(t)
}

/// The fixed cosine-weighted hemisphere: a Fibonacci spiral over the disc, projected up. The
/// same directions for every vertex (rotated into its normal frame) — deterministic by
/// construction, and any residual banding disappears into vertex interpolation.
fn hemisphere_dirs() -> [Vec3; RAY_COUNT] {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    std::array::from_fn(|i| {
        let r = ((i as f32 + 0.5) / RAY_COUNT as f32).sqrt();
        let theta = golden * i as f32;
        let (sin_t, cos_t) = theta.sin_cos();
        // Cosine-weighted: disc sample lifted onto the hemisphere.
        Vec3::new(r * cos_t, (1.0 - r * r).max(0.0).sqrt(), r * sin_t)
    })
}

/// Direct light arriving at a surface point for the bake's purposes: the sun (shadowed by the
/// real geometry — the skylight openings admit it, the roof blocks it) and the worklamp pools.
/// The hemispheric ambient stays out: the runtime already lights every surface with it, and a
/// bounce of a bounce is beyond this bake's one-bounce honesty.
fn direct_light(
    bvh: &Bvh,
    lighting: &SceneLighting,
    point: Vec3,
    normal: Vec3,
    stack: &mut Vec<u32>,
) -> Vec3 {
    let mut total = Vec3::ZERO;
    let key = Vec3::from_array(lighting.key_direction).normalize_or_zero();
    let facing = normal.dot(key);
    if facing > 0.0 && !bvh.occluded(point + normal * 0.02, key, stack) {
        total += Vec3::from_array(lighting.key_rgb) * facing;
    }
    for light in &lighting.local_lights {
        if light.radius_m <= 0.0 {
            continue;
        }
        let to_light = Vec3::from_array(light.position) - point;
        let distance = to_light.length();
        let attenuation = light.attenuation_at(distance);
        if attenuation <= 0.0 {
            continue;
        }
        // The same soft-wrap facing the shader's `local_pools` uses: bounced worklight, not a
        // hard spot.
        let facing = 0.25 + 0.75 * normal.dot(to_light / distance.max(1.0e-4)).max(0.0);
        total += Vec3::from_array(light.rgb) * light.intensity * attenuation * facing;
    }
    total
}

/// One worker's slice of the gather: the bounce lane for `vertices[start..end]`, in order.
///
/// Every vertex is independent — it reads the finished mesh and the BVH and writes nothing —
/// which is what lets the slices run concurrently and be concatenated in range order without
/// the split showing in the result (`chunking_the_gather_does_not_move_a_single_lane`).
fn gather_bounce_range(
    vertices: &[SceneVertex],
    indices: &[u32],
    bvh: &Bvh,
    direct_cache: &[TriDirect],
    start: usize,
    end: usize,
) -> Vec<[f32; 3]> {
    let dirs = hemisphere_dirs();
    // One traversal scratch per worker, reused by every ray it casts (see `Bvh::nearest_hit`).
    let mut stack: Vec<u32> = Vec::with_capacity(64);

    vertices[start..end]
        .iter()
        .map(|vertex| {
            let n = Vec3::from_array(vertex.normal).normalize_or(Vec3::Y);
            let origin = Vec3::from_array(vertex.position) + n * 0.02;
            // An orthonormal frame around the normal for the fixed spiral.
            let seed = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
            let tangent = (seed - n * seed.dot(n)).normalize_or(Vec3::Z);
            let bitangent = n.cross(tangent);

            let mut gathered = Vec3::ZERO;
            for dir in dirs {
                let world = tangent * dir.x + n * dir.y + bitangent * dir.z;
                let Some((distance, tri)) = bvh.nearest_hit(origin, world, &mut stack) else {
                    // Escaped through a skylight opening: the runtime's ambient already
                    // carries the sky, so an escape contributes nothing extra.
                    continue;
                };
                let _ = distance;
                let t = indices[(tri as usize) * 3] as usize;
                let hit_vertex = &vertices[t];
                let hit_albedo = Vec3::from_array(hit_vertex.color);
                if is_emitter(hit_vertex.color) {
                    // An emitter radiates its boosted HDR colour — the same energy the eye
                    // sees on its face, so the wall's spill and the pane agree.
                    gathered += hit_albedo * EMISSION_BOOST;
                    continue;
                }
                // Direct light from the per-triangle cache, side picked the way the old
                // per-hit evaluation flipped its normal: toward the incoming ray.
                let g = (self::tri_normal(&bvh.tris[tri as usize])).normalize_or(Vec3::Y);
                let cached = &direct_cache[tri as usize];
                let direct = if g.dot(world) > 0.0 { cached.back } else { cached.front };
                gathered += direct * hit_albedo.clamp(Vec3::ZERO, Vec3::ONE);
            }
            // Cosine weighting lives in the sample distribution, so the mean IS the
            // irradiance estimate; the gain stands in for unmodelled losses.
            let indirect = gathered / RAY_COUNT as f32 * BOUNCE_GAIN;

            let albedo = Vec3::from_array(vertex.color).clamp(Vec3::ZERO, Vec3::ONE);
            let mut lane = indirect * albedo;
            if is_emitter(vertex.color) {
                lane += Vec3::from_array(vertex.color) * EMISSION_BOOST;
            }
            lane.to_array()
        })
        .collect()
}

/// Direct light landing on one triangle, both orientations, sampled once at its centroid.
///
/// This cache is where most of the old gather's second went: every one of the ~320 000 bounce
/// rays used to evaluate `direct_light` at its own hit point — a sun shadow ray plus six
/// local-light falloffs per HIT — when at bake resolution (every edge â‰¤ [`MAX_EDGE_M`]) a
/// centroid sample is the same resolution class as the vertex-interpolated output the gather
/// feeds anyway. Two evaluations per triangle replace hundreds.
struct TriDirect {
    front: Vec3,
    back: Vec3,
}

/// Build the per-triangle direct-light cache — serial, before the parallel gather, so the
/// gather's determinism argument (pure reads of finished inputs) is untouched.
fn cache_tri_direct(bvh: &Bvh, lighting: &SceneLighting) -> Vec<TriDirect> {
    let mut stack: Vec<u32> = Vec::with_capacity(64);
    bvh.tris
        .iter()
        .map(|tri| {
            let centroid = (tri[0] + tri[1] + tri[2]) / 3.0;
            let g = tri_normal(tri).normalize_or(Vec3::Y);
            TriDirect {
                front: direct_light(bvh, lighting, centroid, g, &mut stack),
                back: direct_light(bvh, lighting, centroid, -g, &mut stack),
            }
        })
        .collect()
}

/// Irradiance arriving at `point` from the hall, for a surface facing `normal` — the same
/// integrand the per-vertex gather runs (one bounce off lit surfaces, emitters at their
/// boosted radiance, escapes contribute nothing), evaluated at a free point instead of a
/// mesh vertex. This is what the hero probe is made of.
fn gather_irradiance_at(
    point: Vec3,
    normal: Vec3,
    vertices: &[SceneVertex],
    indices: &[u32],
    bvh: &Bvh,
    direct_cache: &[TriDirect],
    stack: &mut Vec<u32>,
) -> Vec3 {
    let dirs = hemisphere_dirs();
    let n = normal.normalize_or(Vec3::Y);
    let origin = point + n * 0.02;
    let seed = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let tangent = (seed - n * seed.dot(n)).normalize_or(Vec3::Z);
    let bitangent = n.cross(tangent);
    let mut gathered = Vec3::ZERO;
    for dir in dirs {
        let world = tangent * dir.x + n * dir.y + bitangent * dir.z;
        let Some((_, tri)) = bvh.nearest_hit(origin, world, stack) else {
            continue;
        };
        let hit_vertex = &vertices[indices[(tri as usize) * 3] as usize];
        let hit_albedo = Vec3::from_array(hit_vertex.color);
        if is_emitter(hit_vertex.color) {
            gathered += hit_albedo * EMISSION_BOOST;
            continue;
        }
        let g = tri_normal(&bvh.tris[tri as usize]).normalize_or(Vec3::Y);
        let cached = &direct_cache[tri as usize];
        let direct = if g.dot(world) > 0.0 { cached.back } else { cached.front };
        gathered += direct * hit_albedo.clamp(Vec3::ZERO, Vec3::ONE);
    }
    gathered / RAY_COUNT as f32 * BOUNCE_GAIN
}

/// Height above the deck the hero probe samples at: mid-hull, where the vehicle's flanks live.
const HERO_PROBE_HEIGHT_M: f32 = 1.5;

/// The hero probe: the hall's bounced light at the station, as a six-axis irradiance cube
/// (±x, ±y, ±z — E(n) blends the three faces the normal leans into by its squared
/// components). The vehicle shader adds it the way `scene.wgsl` adds the per-vertex bounce
/// lane, which closes G5: the room has had GI since Hala 2.0 and the hero standing in the
/// middle of it received none. A cube rather than L1 SH on purpose: same job, no ringing,
/// and every face is a plain hemisphere gather the bake already knows how to run.
fn gather_hero_probe(
    vertices: &[SceneVertex],
    indices: &[u32],
    bvh: &Bvh,
    direct_cache: &[TriDirect],
) -> [[f32; 3]; 6] {
    let mut stack: Vec<u32> = Vec::with_capacity(64);
    let point = Vec3::new(0.0, HERO_PROBE_HEIGHT_M, 0.0);
    [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z].map(|n| {
        gather_irradiance_at(point, n, vertices, indices, bvh, direct_cache, &mut stack).to_array()
    })
}

/// Subdivide the hall for bake resolution, then gather one bounce per vertex into the bounce
/// lane. Runs once at mesh build; everything is pure math over the arrays.
///
/// The gather is the expensive half — ~284 000 hemisphere rays plus a shadow ray per hit, and
/// the whole of the second the hall used to cost — so it runs one worker lane per range of
/// vertices and concatenates the results in range order. Two things make that safe rather than
/// merely fast: a vertex's bounce reads only the finished mesh and the BVH, and float addition
/// never crosses a range boundary, so the concatenated lane is bit-identical to the serial one.
/// `the_bake_is_deterministic` still holds the whole thing to the bit.
pub(super) fn bake_bounce_lane(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
) -> [[f32; 3]; 6] {
    subdivide_for_bake(vertices, indices);
    let lighting = SceneLighting::garage_hero();
    let (bounces, probe) = {
        let verts: &[SceneVertex] = vertices;
        let idx: &[u32] = indices;
        let bvh = Bvh::build(verts, idx);
        let direct_cache = cache_tri_direct(&bvh, &lighting);
        let bounces = gather_bounce_lanes(verts, idx, &bvh, &direct_cache);
        let probe = gather_hero_probe(verts, idx, &bvh, &direct_cache);
        (bounces, probe)
    };

    for (vertex, bounce) in vertices.iter_mut().zip(bounces) {
        vertex.bounce = bounce;
    }
    probe
}

/// The whole bounce lane, gathered across worker lanes and glued back in vertex order.
fn gather_bounce_lanes(
    vertices: &[SceneVertex],
    indices: &[u32],
    bvh: &Bvh,
    direct_cache: &[TriDirect],
) -> Vec<[f32; 3]> {
    let ranges = crate::battlefield::bake_lane_ranges(vertices.len());
    if ranges.len() < 2 {
        return gather_bounce_range(vertices, indices, bvh, direct_cache, 0, vertices.len());
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = ranges
            .iter()
            .map(|&(start, end)| {
                scope.spawn(move || {
                    gather_bounce_range(vertices, indices, bvh, direct_cache, start, end)
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("hangar bounce gather"))
            .collect()
    })
}

fn tri_normal(tri: &[Vec3; 3]) -> Vec3 {
    (tri[1] - tri[0]).cross(tri[2] - tri[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baked_hall() -> (Vec<SceneVertex>, Vec<u32>) {
        super::super::hangar::hangar_scene_mesh()
    }

    /// The bake is a pure function: two builds agree to the bit. This is what licenses the
    /// golden harness to lock frames rendered from it — and with the gather now spread over
    /// worker lanes it is also what proves the split did not move a value.
    ///
    /// It builds through the UNCACHED path on purpose. It used to call `hangar_scene_mesh()`
    /// twice, which returns two clones of one `OnceLock`, so it compared a value with itself
    /// and would have passed against a bake seeded from the clock.
    #[test]
    fn the_bake_is_deterministic() {
        let (a, _, probe_a) = crate::hangar::build_hangar_scene_mesh();
        let (b, _, probe_b) = crate::hangar::build_hangar_scene_mesh();
        assert_eq!(a.len(), b.len());
        assert_eq!(probe_a, probe_b, "the hero probe must be bit-identical across builds");
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.bounce, y.bounce, "bounce must be bit-identical across builds");
        }
    }

    /// A small lit room — floor, four walls, a hot ceiling panel — with faces wide enough to
    /// earn bake resolution. Enough to exercise every arm of the gather (a ray that escapes,
    /// a ray that hits an emitter, a ray that hits a shadowed wall) at a thousandth of the
    /// hall's cost.
    fn test_room() -> (Vec<SceneVertex>, Vec<u32>) {
        use crate::hangar::slab;
        let (mut v, mut i) = (Vec::new(), Vec::new());
        slab(&mut v, &mut i, [0.0, -0.1, 0.0], [6.0, 0.1, 6.0], [0.30, 0.29, 0.28]);
        for cz in [-6.0_f32, 6.0] {
            slab(&mut v, &mut i, [0.0, 2.5, cz], [6.0, 2.5, 0.1], [0.24, 0.245, 0.255]);
        }
        for cx in [-6.0_f32, 6.0] {
            slab(&mut v, &mut i, [cx, 2.5, 0.0], [0.1, 2.5, 6.0], [0.24, 0.245, 0.255]);
        }
        slab(&mut v, &mut i, [0.0, 5.1, 0.0], [6.0, 0.1, 6.0], [0.125, 0.13, 0.14]);
        slab(&mut v, &mut i, [0.0, 4.9, 0.0], [1.2, 0.05, 1.2], [1.55, 1.40, 1.10]);
        (v, i)
    }

    /// The lane split is invisible in the output: gathering a range at a time and gluing the
    /// results in order gives bit-identical values to gathering the whole array serially. The
    /// mirror of `chunking_the_ground_bake_does_not_move_a_single_texel`, for the bake that
    /// went parallel second.
    #[test]
    fn chunking_the_gather_does_not_move_a_single_lane() {
        let (mut vertices, mut indices) = test_room();
        subdivide_for_bake(&mut vertices, &mut indices);
        let lighting = SceneLighting::garage_hero();
        let bvh = Bvh::build(&vertices, &indices);
        let cache = cache_tri_direct(&bvh, &lighting);

        let whole = gather_bounce_range(&vertices, &indices, &bvh, &cache, 0, vertices.len());
        assert!(whole.iter().any(|lane| lane.iter().any(|c| *c > 0.0)), "the room gathers light");

        let seam = vertices.len() / 3;
        let mut glued = gather_bounce_range(&vertices, &indices, &bvh, &cache, 0, seam);
        glued.extend(gather_bounce_range(&vertices, &indices, &bvh, &cache, seam, vertices.len()));
        assert_eq!(whole, glued, "a hand-cut seam moved a bounce value");

        // ...and the shipped plan, whatever number of lanes this machine gives it, agrees too.
        let lanes = gather_bounce_lanes(&vertices, &indices, &bvh, &cache);
        assert_eq!(whole, lanes, "the worker-lane split moved a bounce value");
    }

    /// Every lane value is finite and inside a sane HDR envelope — a NaN or a runaway value
    /// here becomes a hole or a flare in every garage frame at once.
    #[test]
    fn the_bounce_lane_is_finite_and_bounded() {
        let (vertices, _) = baked_hall();
        for v in &vertices {
            for c in v.bounce {
                assert!(c.is_finite() && (0.0..=4.0).contains(&c), "bounce {:?}", v.bounce);
            }
        }
    }

    /// The point of the whole bake: light lands where the room says it should. The roof plane
    /// around the high-bay lamps carries their spill (the panes that used to anchor this lock
    /// are gone — readable-light correction — so the lamps ARE the room's emitters now), and
    /// the floor — lit by the skylight sun — returns light onto the hall at all.
    #[test]
    fn light_spills_where_the_room_emits() {
        let (vertices, _) = baked_hall();
        let luma = |b: [f32; 3]| 0.2126 * b[0] + 0.7152 * b[1] + 0.0722 * b[2];

        // Emitter vertices glow on their own.
        let emitters: Vec<_> = vertices.iter().filter(|v| super::is_emitter(v.color)).collect();
        assert!(!emitters.is_empty(), "the hall has authored emitters");
        for e in &emitters {
            assert!(luma(e.bounce) > 0.3, "an emitter must carry its own glow, got {:?}", e.bounce);
        }

        // The surfaces around each high-bay lamp read its spill. "High bay" means the pendants
        // OVER THE TURNTABLE — selected by where they hang, not by a height literal (the old
        // `y > 9.0` filter silently emptied when the roof came down to a 9 m chord).
        let rig = SceneLighting::garage_hero().local_lights;
        let high_bays: Vec<_> = rig
            .iter()
            .filter(|l| {
                l.radius_m > 0.0 && (l.position[0].powi(2) + l.position[2].powi(2)).sqrt() < 6.0
            })
            .collect();
        assert!(!high_bays.is_empty(), "the rig hangs high-bay lamps");
        let mut spill_peak = 0.0f32;
        for lamp in &high_bays {
            let near_lamp: Vec<f32> = vertices
                .iter()
                .filter(|v| {
                    let (dx, dz) =
                        (v.position[0] - lamp.position[0], v.position[2] - lamp.position[2]);
                    (dx * dx + dz * dz).sqrt() < 3.0
                        && v.position[1] > lamp.position[1] - 1.5
                        && !super::is_emitter(v.color)
                })
                .map(|v| luma(v.bounce))
                .collect();
            assert!(!near_lamp.is_empty(), "the band around a high-bay has bake vertices");
            let peak = near_lamp.iter().copied().fold(0.0f32, f32::max);
            assert!(peak > 0.005, "a lamp must spill light onto its surroundings, peak {peak}");
            spill_peak = spill_peak.max(peak);
        }

        // And the bake is not a uniform wash: somewhere far from every emitter and the
        // skylight shafts it is much dimmer than the peak.
        let floor_min = vertices
            .iter()
            .filter(|v| v.position[1] < 0.1 && !super::is_emitter(v.color))
            .map(|v| luma(v.bounce))
            .fold(f32::INFINITY, f32::min);
        assert!(
            floor_min < spill_peak * 0.5,
            "bounce must vary across the room (floor min {floor_min}, peak {spill_peak})"
        );
    }

    /// Subdivision keeps the mesh conformal and bounded: no edge past the bake limit grows
    /// the hall beyond a sane budget, and every index stays valid.
    #[test]
    fn subdivision_stays_bounded_and_valid() {
        let (vertices, indices) = baked_hall();
        assert_eq!(indices.len() % 3, 0);
        for &i in &indices {
            assert!((i as usize) < vertices.len(), "index {i} out of range");
        }
        assert!(
            indices.len() / 3 < 120_000,
            "the bake subdivision exploded: {} triangles",
            indices.len() / 3
        );
    }
}
