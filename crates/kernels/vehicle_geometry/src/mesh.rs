use std::collections::HashMap;

use glam::{Vec2, Vec3};

use crate::MeshBounds;
use crate::weld::{WeldKey, weld_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MaterialRole {
    RolledArmor,
    CastArmor,
    BarrelSteel,
    TrackMetal,
    Rubber,
    InteriorPrimer,
    InteriorMachinery,
    Ammunition,
    /// Freshly fractured armor section. Dynamic damage geometry uses this instead of borrowing
    /// barrel steel, so the renderer can distinguish a rough, heat-stained cut from a gun tube.
    ExposedSteel,
    /// Proofed canvas: the mantlet dust cover, tarpaulins, the gun-cleaning kit's rolls. Matte,
    /// unlit-looking fabric with no specular lobe worth speaking of.
    ///
    /// Its own role because the alternative is worse. Every fabric part this fleet will ever
    /// carry would otherwise be `CastArmor` — and a canvas boot rendered as cast steel is the
    /// same mistake the mantlet comment already records about the mask being merged into the
    /// barrel: one material for two things is one of them rendered wrong.
    Canvas,
    /// Glass: the headlight's lens, vision-block prisms. Bright, smooth, and the one thing on a
    /// tank that is supposed to catch the sun.
    ///
    /// A lens rendered as the steel drum behind it is not a lens — it is a disc, and a viewer
    /// reads it as one. Same argument as [`MaterialRole::Canvas`]: one material for two things
    /// is one of them rendered wrong.
    Glass,
    /// Seasoned timber: the unditching log. Matte, fibrous, unpainted.
    ///
    /// Open decision #6 resolved the honest way: the log is wood, and rendering wood as track
    /// steel was the recorded compromise. Same argument as Canvas and Glass — one material for
    /// two things is one of them rendered wrong.
    Timber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SmoothingGroup(pub u16);

impl SmoothingGroup {
    pub const fn hard_edges() -> Self {
        Self(0)
    }
}

/// How a vertex's material coordinates are derived. Parametric kernels (plates, lofts, revolves,
/// sweeps, panels) author a deterministic `uv0`; organic kernels (SDF castings, blends, deformation)
/// declare `Triplanar` and the renderer projects from object-local coordinates instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfaceMapping {
    /// Use the authored `uv0` with a tangent-space normal map.
    ParametricUv,
    /// Project material coordinates from object-local position/normal (no authored UV).
    #[default]
    Triplanar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv0: Vec2,
    pub mapping: SurfaceMapping,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
    pub surface_shade: f32,
}

impl GeometryVertex {
    /// A vertex with the default surface mapping (`Triplanar`, zero UV) — the safe fallback for the
    /// organic kernels and the many call sites that do not author parametric coordinates. Parametric
    /// kernels layer their chart on top with [`with_uv0`](Self::with_uv0).
    pub fn new(
        position: Vec3,
        normal: Vec3,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) -> Self {
        Self {
            position,
            normal: normal.normalize_or_zero(),
            uv0: Vec2::ZERO,
            mapping: SurfaceMapping::Triplanar,
            material,
            smoothing,
            surface_shade: 1.0,
        }
    }

    /// Declare an authored parametric chart: sets `uv0` and switches the vertex to `ParametricUv`.
    pub fn with_uv0(mut self, uv0: Vec2) -> Self {
        self.uv0 = uv0;
        self.mapping = SurfaceMapping::ParametricUv;
        self
    }

    pub fn with_surface_shade(mut self, shade: f32) -> Self {
        self.surface_shade = shade.clamp(0.0, 1.0);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeometryMesh {
    vertices: Vec<GeometryVertex>,
    indices: Vec<u32>,
}

impl GeometryMesh {
    pub fn new(vertices: Vec<GeometryVertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn vertices(&self) -> &[GeometryVertex] {
        &self.vertices
    }

    /// Mutable vertex access for in-crate post-passes (shading, contact cavities). Topology is left
    /// untouched, so callers must not change the vertex count.
    pub(crate) fn vertices_mut(&mut self) -> &mut [GeometryVertex] {
        &mut self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn bounds(&self) -> Option<MeshBounds> {
        let mut vertices = self.vertices.iter();
        let first = vertices.next()?;
        let mut bounds = MeshBounds::from_point(first.position);
        for vertex in vertices {
            bounds.include(vertex.position);
        }
        Some(bounds)
    }

    pub fn with_height_shade(mut self, floor: f32, bright_y: f32, low_shade: f32) -> Self {
        let span = (bright_y - floor).max(0.001);
        for vertex in &mut self.vertices {
            let t = ((vertex.position.y - floor) / span).clamp(0.0, 1.0);
            let shade = low_shade + (1.0 - low_shade) * t;
            *vertex = vertex.with_surface_shade(shade);
        }
        self
    }

    /// Fuse coincident vertices and resolve their normals by smoothing group.
    ///
    /// Vertices in the same *smooth* group (anything but [`SmoothingGroup::hard_edges`]) that land
    /// on the same position and material collapse into one vertex whose normal is the average of
    /// every face that met there — so cast turrets, mantlets, and wheels read as round castings
    /// rather than facets. Hard-edge vertices fold the normal into their identity, so coincident
    /// faces with different normals stay separate and welded plates keep their crisp seams.
    ///
    /// Positions, winding, triangle count, materials, and per-vertex shade are all preserved; only
    /// normals change and duplicate vertices are removed. Normals are rebuilt from the indexed
    /// triangle faces so smoothing does not depend on pre-authored vertex normals. The pass is
    /// deterministic: buckets are numbered in first-encounter order, independent of hash-map
    /// iteration.
    pub fn weld_and_smooth(self) -> Self {
        let mut bucket_of: HashMap<WeldKey, usize> = HashMap::new();
        let mut welded: Vec<GeometryVertex> = Vec::new();
        let mut normal_sums: Vec<Vec3> = Vec::new();
        let mut remap: Vec<u32> = Vec::with_capacity(self.vertices.len());

        for vertex in &self.vertices {
            let bucket = *bucket_of.entry(weld_key(vertex)).or_insert_with(|| {
                welded.push(*vertex);
                normal_sums.push(Vec3::ZERO);
                welded.len() - 1
            });
            remap.push(bucket as u32);
        }

        for tri in self.indices.chunks_exact(3) {
            let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let normal = (self.vertices[b].position - self.vertices[a].position)
                .cross(self.vertices[c].position - self.vertices[a].position)
                .normalize_or_zero();
            if normal != Vec3::ZERO {
                normal_sums[remap[a] as usize] += normal;
                normal_sums[remap[b] as usize] += normal;
                normal_sums[remap[c] as usize] += normal;
            }
        }

        for (vertex, sum) in welded.iter_mut().zip(&normal_sums) {
            let averaged = sum.normalize_or_zero();
            if averaged != Vec3::ZERO {
                vertex.normal = averaged;
            }
        }

        let indices = self.indices.iter().map(|&index| remap[index as usize]).collect();
        Self::new(welded, indices)
    }
}
