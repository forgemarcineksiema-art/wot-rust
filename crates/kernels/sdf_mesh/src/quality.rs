//! A measured contract for SDF meshing: pick the grid resolution from an explicit spec rather than
//! a magic constant, and report not just the triangle count but how much of the budget was used and
//! how far the generated vertices drift from the true surface. Uniform Surface Nets remains the
//! production mesher; adaptive octree / QEF dual contouring is an R10 research prototype only.

use sdf::Sdf;
use vehicle_geometry::{GeometryMesh, MaterialRole, MeshBounds, SmoothingGroup};

use crate::surface_nets::{Grid, mesh_sdf};

/// What to mesh, the triangle budget, and the resolution bounds the grid estimate is clamped to.
pub struct SdfMeshingSpec {
    pub bounds: MeshBounds,
    pub triangle_budget: usize,
    pub min_cells_per_axis: usize,
    pub max_cells_per_axis: usize,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}

/// The meshing outcome plus the quality signals a reviewer wants: which grid was chosen, how much of
/// the budget the result used, and the worst surface residual sampled on the output.
pub struct SdfMeshingResult {
    pub mesh: GeometryMesh,
    pub grid: Grid,
    pub triangle_count: usize,
    /// Triangles produced as a fraction of the budget (`> 1.0` only if the floor resolution
    /// overshoots).
    pub budget_utilization: f32,
    /// The largest `|sdf.eval(vertex)|` over the output — how far the mesh sits off the true surface.
    pub max_residual: f32,
}

/// Estimate the on-budget resolution deterministically, then step coarser until the mesh fits the
/// triangle budget, never going below `min_cells_per_axis`.
pub fn mesh_to_spec(sdf: &Sdf, spec: &SdfMeshingSpec) -> SdfMeshingResult {
    let budget = spec.triangle_budget.max(1);
    let (lo, hi) = (spec.min_cells_per_axis.max(2), spec.max_cells_per_axis.max(2));
    // Surface Nets produces roughly `~4 * target^2` triangles for a closed blob, so the on-budget
    // resolution is about `sqrt(budget / 4)`.
    let mut target = ((budget as f32 / 4.0).sqrt().round() as usize).clamp(lo, hi);

    let mut grid = Grid::wrapping(spec.bounds.min, spec.bounds.max, target);
    let mut mesh = mesh_sdf(sdf, &grid, spec.material, spec.smoothing);
    while mesh.triangle_count() > budget && target > lo {
        target = ((target * 9 / 10).max(lo)).min(target - 1);
        grid = Grid::wrapping(spec.bounds.min, spec.bounds.max, target);
        mesh = mesh_sdf(sdf, &grid, spec.material, spec.smoothing);
    }

    let triangle_count = mesh.triangle_count();
    let max_residual =
        mesh.vertices().iter().map(|v| sdf.eval(v.position).abs()).fold(0.0_f32, f32::max);
    SdfMeshingResult {
        mesh,
        grid,
        triangle_count,
        budget_utilization: triangle_count as f32 / budget as f32,
        max_residual,
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    fn spec(bounds: MeshBounds, budget: usize) -> SdfMeshingSpec {
        SdfMeshingSpec {
            bounds,
            triangle_budget: budget,
            min_cells_per_axis: 8,
            max_cells_per_axis: 96,
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup(2),
        }
    }

    fn bounds(r: f32) -> MeshBounds {
        MeshBounds { min: Vec3::splat(-r), max: Vec3::splat(r) }
    }

    #[test]
    fn a_sphere_lands_under_budget_with_a_tight_surface_residual() {
        let result = mesh_to_spec(&Sdf::sphere(1.0), &spec(bounds(1.3), 2000));
        assert!(result.triangle_count > 0 && result.triangle_count <= 2000);
        assert!(result.budget_utilization <= 1.0);
        // Vertices sit within a cell of the true sphere surface.
        assert!(
            result.max_residual < 2.0 * result.grid.cell_size(),
            "residual {} too large for cell {}",
            result.max_residual,
            result.grid.cell_size()
        );
    }

    #[test]
    fn a_blended_turret_fragment_fits_its_budget() {
        let blob = Sdf::sphere(0.9)
            .translate(Vec3::new(-0.4, 0.0, 0.0))
            .smooth_union(Sdf::sphere(0.9).translate(Vec3::new(0.4, 0.0, 0.0)), 0.4);
        let result = mesh_to_spec(&blob, &spec(bounds(1.6), 3000));
        assert!(result.triangle_count > 0 && result.triangle_count <= 3000);
        assert!(result.max_residual < 2.0 * result.grid.cell_size());
    }

    #[test]
    fn a_deliberately_empty_field_meshes_to_nothing() {
        let outside = Sdf::sphere(0.4).translate(Vec3::new(100.0, 0.0, 0.0));
        let result = mesh_to_spec(&outside, &spec(bounds(1.0), 2000));
        assert_eq!(result.triangle_count, 0);
        assert_eq!(result.budget_utilization, 0.0);
        assert_eq!(result.max_residual, 0.0);
    }

    #[test]
    fn the_selected_grid_respects_the_cell_bounds() {
        let result = mesh_to_spec(&Sdf::sphere(1.0), &spec(bounds(1.3), 200));
        // A tiny budget forces a coarse grid, but never below the floor (+2 pad cells).
        let max_dim = result.grid.cells_per_axis().into_iter().max().unwrap();
        assert!(max_dim >= 8, "grid never goes below the cell floor");
    }
}
