//! Renderer-neutral mesh-quality contract: the one shared audit every procedural generator is
//! measured against, so a "valid mesh" means the same thing for a `solid` plate, a `cast_loft`
//! turret, a `revolve` wheel and an SDF socket. It proves *orientational consistency* and
//! finiteness; it does not infer a global "outward" direction from an arbitrary origin —
//! shape-specific tests still prove semantic outwardness where that concept is meaningful.

use std::collections::HashMap;

use crate::{GeometryMesh, MeshBounds};

/// What end-topology a mesh is expected to have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyExpectation {
    /// Boundary edges are allowed (open shells, capped or not).
    Any,
    /// The mesh is meant to be open; boundary edges are expected, not an error.
    Open,
    /// The mesh is meant to be watertight; any boundary edge is a failure.
    ClosedManifold,
}

/// Tunable thresholds for one audit pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshQualitySpec {
    pub topology: TopologyExpectation,
    /// Squared doubled-area below which a triangle counts as degenerate.
    pub min_triangle_area: f32,
    /// Allowed deviation of a vertex normal's length from `1.0`.
    pub normal_tolerance: f32,
}

/// The measured result of one audit pass — always computable, never panics on bad input.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualityReport {
    pub vertices: usize,
    pub triangles: usize,
    pub bounds: Option<MeshBounds>,
    pub boundary_edges: usize,
    pub non_manifold_edges: usize,
    pub degenerate_triangles: usize,
    pub invalid_indices: usize,
    pub non_finite_vertices: usize,
    pub non_unit_normals: usize,
    pub inconsistent_winding_edges: usize,
}

/// The first contract violation found, in a deterministic priority order.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum MeshQualityError {
    #[error("mesh has {0} invalid indices")]
    InvalidIndices(usize),
    #[error("mesh has {0} non-finite vertex attributes")]
    NonFiniteVertices(usize),
    #[error("mesh has {0} degenerate triangles")]
    DegenerateTriangles(usize),
    #[error("mesh has {0} non-manifold edges")]
    NonManifoldEdges(usize),
    #[error("mesh is expected to be closed but has {0} boundary edges")]
    UnexpectedBoundaryEdges(usize),
    #[error("mesh has {0} non-unit normals")]
    NonUnitNormals(usize),
    #[error("mesh has {0} inconsistent winding edges")]
    InconsistentWinding(usize),
}

/// Directed uses of one undirected edge: `forward` is `low -> high`, `backward` is `high -> low`.
#[derive(Default, Clone, Copy)]
struct EdgeUse {
    forward: u32,
    backward: u32,
}

impl EdgeUse {
    fn total(&self) -> u32 {
        self.forward + self.backward
    }
}

impl GeometryMesh {
    /// Audit the mesh against `spec`, returning every measured count. Never panics.
    pub fn quality_report(&self, spec: MeshQualitySpec) -> MeshQualityReport {
        let vertices = self.vertices();
        let indices = self.indices();
        let vertex_count = vertices.len();

        // Vertex-attribute finiteness and normal length.
        let mut non_finite_vertices = 0;
        let mut non_unit_normals = 0;
        for v in vertices {
            if !v.position.is_finite() || !v.normal.is_finite() || !v.surface_shade.is_finite() {
                non_finite_vertices += 1;
            }
            if v.normal.is_finite() {
                let len = v.normal.length();
                if (len - 1.0).abs() > spec.normal_tolerance {
                    non_unit_normals += 1;
                }
            }
        }

        // Any index past the end of the vertex list is invalid, as is a trailing remainder that
        // does not complete a triangle.
        let mut invalid_indices = indices.len() % 3;
        for &i in indices {
            if i as usize >= vertex_count {
                invalid_indices += 1;
            }
        }

        let mut degenerate_triangles = 0;
        let mut edges: HashMap<(u32, u32), EdgeUse> = HashMap::new();
        for tri in indices.chunks_exact(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]];
            // Skip geometry for triangles that reference missing vertices; they are already
            // counted as invalid indices above.
            if (a as usize) >= vertex_count
                || (b as usize) >= vertex_count
                || (c as usize) >= vertex_count
            {
                continue;
            }

            if a == b || b == c || c == a {
                degenerate_triangles += 1;
            } else {
                let pa = vertices[a as usize].position;
                let pb = vertices[b as usize].position;
                let pc = vertices[c as usize].position;
                let doubled_area_sq = (pb - pa).cross(pc - pa).length_squared();
                if !doubled_area_sq.is_finite() || doubled_area_sq < spec.min_triangle_area {
                    degenerate_triangles += 1;
                }
            }

            for &(u, v) in &[(a, b), (b, c), (c, a)] {
                let key = (u.min(v), u.max(v));
                let entry = edges.entry(key).or_default();
                if u < v {
                    entry.forward += 1;
                } else {
                    entry.backward += 1;
                }
            }
        }

        let mut boundary_edges = 0;
        let mut non_manifold_edges = 0;
        let mut inconsistent_winding_edges = 0;
        for use_ in edges.values() {
            match use_.total() {
                1 => boundary_edges += 1,
                2 => {
                    // Two oppositely directed uses are consistent; two same-direction uses are not.
                    if use_.forward == 2 || use_.backward == 2 {
                        inconsistent_winding_edges += 1;
                    }
                }
                _ => non_manifold_edges += 1,
            }
        }

        MeshQualityReport {
            vertices: vertex_count,
            triangles: indices.len() / 3,
            bounds: self.bounds(),
            boundary_edges,
            non_manifold_edges,
            degenerate_triangles,
            invalid_indices,
            non_finite_vertices,
            non_unit_normals,
            inconsistent_winding_edges,
        }
    }

    /// Audit the mesh and fail with the first contract violation, in priority order.
    pub fn validate_quality(
        &self,
        spec: MeshQualitySpec,
    ) -> Result<MeshQualityReport, MeshQualityError> {
        let report = self.quality_report(spec);
        if report.invalid_indices > 0 {
            return Err(MeshQualityError::InvalidIndices(report.invalid_indices));
        }
        if report.non_finite_vertices > 0 {
            return Err(MeshQualityError::NonFiniteVertices(report.non_finite_vertices));
        }
        if report.degenerate_triangles > 0 {
            return Err(MeshQualityError::DegenerateTriangles(report.degenerate_triangles));
        }
        if report.non_manifold_edges > 0 {
            return Err(MeshQualityError::NonManifoldEdges(report.non_manifold_edges));
        }
        if report.inconsistent_winding_edges > 0 {
            return Err(MeshQualityError::InconsistentWinding(report.inconsistent_winding_edges));
        }
        if spec.topology == TopologyExpectation::ClosedManifold && report.boundary_edges > 0 {
            return Err(MeshQualityError::UnexpectedBoundaryEdges(report.boundary_edges));
        }
        if report.non_unit_normals > 0 {
            return Err(MeshQualityError::NonUnitNormals(report.non_unit_normals));
        }
        Ok(report)
    }
}

/// A closed, smoothly-shaded shell: watertight with consistent winding and unit normals.
pub const CLOSED_SMOOTH_MESH: MeshQualitySpec = MeshQualitySpec {
    topology: TopologyExpectation::ClosedManifold,
    min_triangle_area: 1.0e-10,
    normal_tolerance: 1.0e-3,
};

/// An open or closed mesh: finiteness and consistency are required, boundary edges are allowed.
pub const OPEN_OR_CLOSED_MESH: MeshQualitySpec = MeshQualitySpec {
    topology: TopologyExpectation::Any,
    min_triangle_area: 1.0e-10,
    normal_tolerance: 1.0e-3,
};
