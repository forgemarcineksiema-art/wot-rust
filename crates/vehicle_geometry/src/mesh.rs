use std::collections::HashMap;

use glam::Vec3;

use crate::MeshBounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialRole {
    RolledArmor,
    CastArmor,
    BarrelSteel,
    TrackMetal,
    Rubber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SmoothingGroup(pub u16);

impl SmoothingGroup {
    pub const fn hard_edges() -> Self {
        Self(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
    pub surface_shade: f32,
}

impl GeometryVertex {
    pub fn new(
        position: Vec3,
        normal: Vec3,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) -> Self {
        Self {
            position,
            normal: normal.normalize_or_zero(),
            material,
            smoothing,
            surface_shade: 1.0,
        }
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

/// Identity used to decide which vertices weld together. Hard edges include the (quantised) normal
/// so seams stay distinct; smooth groups omit it so coincident corners fuse and average.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WeldKey {
    position: [i64; 3],
    smoothing: SmoothingGroup,
    material: MaterialRole,
    normal: Option<[i64; 3]>,
}

fn weld_key(vertex: &GeometryVertex) -> WeldKey {
    let normal =
        (vertex.smoothing == SmoothingGroup::hard_edges()).then(|| quantize(vertex.normal));
    WeldKey {
        position: quantize(vertex.position),
        smoothing: vertex.smoothing,
        material: vertex.material,
        normal,
    }
}

/// Snap to a sub-millimetre grid so vertices the kernel meant to share a position weld together
/// despite float noise, without merging genuinely separate detail.
fn quantize(value: Vec3) -> [i64; 3] {
    const WELD_SCALE: f32 = 4096.0;
    [
        (value.x * WELD_SCALE).round() as i64,
        (value.y * WELD_SCALE).round() as i64,
        (value.z * WELD_SCALE).round() as i64,
    ]
}
