//! The parametric vehicle description: the part list, where parts mount, and how the whole thing
//! bakes into one `BakedVehicle` for the Forge. This is the single place that knows a vehicle is a
//! *set of parts*, each routed to the right generator and grouped into hull / turret / gun submeshes.

use game_core::{MountFrames, VehicleBlueprint, VehicleKind, VehicleModules};
use glam::Vec3;
use vehicle_geometry::{BakedVehicle, LodLevel, Submesh, SubmeshKind, reduce_mesh, reduce_vehicle};

use crate::part::VehiclePart;
use crate::surface_bake::SurfaceBake;

/// How far a description has been built, and therefore which cost envelope and golden regime it
/// answers to. A property of the DESCRIPTION, read by the forge — never a match on the vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fidelity {
    /// Built from the fleet part library with a complete blueprint visual: the hybrid-class
    /// budgets (`MEDIUM_LOD0_TRI_BUDGET`), no recipe golden.
    Benchmark,
    /// A lean recipe wrapped as a description: the procedural fleet's budgets and goldens
    /// (`vehicle_recipes::VEHICLE_BUDGETS`, `GOLDEN_BAKE_HASHES`).
    Sketch,
}

/// How a description reduces to LOD1/LOD2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodStrategy {
    /// Each retained part clustered by its own importance BEFORE the merge (`build_reduced_lod`).
    PartAware,
    /// The full bake flattened, then clustered per submesh (`vehicle_geometry::reduce_vehicle`) —
    /// what the recipes always did; kept byte-exact for the sketches.
    WholeMesh,
}

/// What happens to a submesh after its parts merge and before the surface bake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostMerge {
    /// The parts are final meshes: merge and shade (the part library's way).
    None,
    /// Weld coincident vertices and smooth normals across the merged parts — what a recipe's
    /// `assemble` always did to its concatenated builders, kept so a recipe split into pieces
    /// bakes byte-exact (Forge 2.0 K3).
    WeldAndSmooth,
}

/// A vehicle as a parametric part list plus its mount frames.
pub struct VehicleDescription {
    pub kind: VehicleKind,
    pub parts: Vec<VehiclePart>,
    pub mounts: MountFrames,
    /// Semantic contact cavities baked into `surface_shade` after merge (empty = flat shade).
    pub surface_bake: SurfaceBake,
    pub fidelity: Fidelity,
    pub lod: LodStrategy,
    pub post_merge: PostMerge,
}

/// The description the part library builds for `kind`, if its blueprint carries a complete
/// visual. This is the fleet's migration rule (Forge 2.0 K1): a vehicle joins the hybrid path by
/// DATA — its blueprint gains the visual tree — and nothing in the forge names it.
pub fn description_for(kind: VehicleKind) -> Option<VehicleDescription> {
    let bp = VehicleBlueprint::for_vehicle(kind)?;
    description_from_blueprint(&bp)
}

/// The same rule over an explicit (possibly live) blueprint at the stock loadout.
pub fn description_from_blueprint(bp: &VehicleBlueprint) -> Option<VehicleDescription> {
    description_for_modules_with_blueprint(&bp.kind.default_loadout(), bp)
}

/// The same rule at an explicit module loadout (the tank compiler's path).
pub fn description_for_modules(
    kind: VehicleKind,
    modules: &VehicleModules,
) -> Option<VehicleDescription> {
    let bp = VehicleBlueprint::for_vehicle(kind)?;
    description_for_modules_with_blueprint(modules, &bp)
}

fn description_for_modules_with_blueprint(
    modules: &VehicleModules,
    bp: &VehicleBlueprint,
) -> Option<VehicleDescription> {
    bp.complete_visual()?;
    Some(crate::t54_from_modules_with_blueprint(modules, bp))
}

impl VehicleDescription {
    /// The production bake at `lod`, reduced the way this description declares.
    pub fn production_bake(&self, lod: LodLevel) -> BakedVehicle {
        match self.lod {
            LodStrategy::PartAware => self.build_reduced_lod(lod),
            LodStrategy::WholeMesh => reduce_vehicle(&self.build(), lod),
        }
    }

    /// Mesh every part, merge by submesh kind, and assemble the `BakedVehicle` the Forge consumes.
    /// This is the full-detail LOD0 bake.
    pub fn build(&self) -> BakedVehicle {
        self.build_lod(LodLevel::Lod0)
    }

    /// Bake only the parts whose LOD policy keeps them in `lod`: LOD0 carries everything, LOD1/LOD2
    /// drop the detail fittings and track links, leaving the silhouette and the mount-bearing parts.
    /// Triangle decimation within a tier is left to `vehicle_geometry::reduce_vehicle`.
    pub fn build_lod(&self, lod: LodLevel) -> BakedVehicle {
        let mut submeshes = Vec::new();
        for kind in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let bands = self.surface_bake.bands_for(kind);
            let meshes: Vec<_> = self
                .parts
                .iter()
                .filter(|part| part.submesh == kind && part.lod.kept_at(lod))
                .map(VehiclePart::mesh)
                .collect();
            if !meshes.is_empty() {
                let merged = revolve::merge(&meshes);
                let merged = match self.post_merge {
                    PostMerge::None => merged,
                    PostMerge::WeldAndSmooth => merged.weld_and_smooth(),
                };
                submeshes.push(Submesh { kind, mesh: merged.with_contact_cavity(&bands) });
            }
        }
        BakedVehicle::new(self.kind, submeshes, self.mounts)
    }

    /// The executable part manifest: the renderer-free production source of truth the Forge consumes
    /// (each part's key, group, material, LOD class, generator and *real* mesh bounds).
    pub fn part_manifest(&self) -> Vec<crate::PartManifestEntry> {
        crate::manifest::part_manifest(self)
    }

    /// Build a reduced tier the *part-aware* way: each retained part is meshed, then clustered with a
    /// cell scaled by its own importance (silhouette/mount-critical finer, detail coarser) *before*
    /// the parts merge into the three runtime groups. LOD0 has no reduction and returns the full bake.
    pub fn build_reduced_lod(&self, lod: LodLevel) -> BakedVehicle {
        let Some(fraction) = lod.cluster_fraction() else {
            return self.build_lod(lod);
        };
        // Mesh every retained part once, keeping its group and importance, and track the whole-body
        // extent so each part's cell is a fraction of the same body scale (not its own local size).
        let retained: Vec<(SubmeshKind, f32, vehicle_geometry::GeometryMesh)> = self
            .parts
            .iter()
            .filter(|part| part.lod.kept_at(lod))
            .map(|part| (part.submesh, part.lod.importance().cell_scale(), part.mesh()))
            .collect();
        let cell = body_cell(retained.iter().map(|(_, _, m)| m), fraction);

        let mut submeshes = Vec::new();
        for kind in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let bands = self.surface_bake.bands_for(kind);
            let reduced: Vec<_> = retained
                .iter()
                .filter(|(group, _, _)| *group == kind)
                .map(|(_, scale, mesh)| reduce_mesh(mesh, cell * scale))
                .collect();
            if !reduced.is_empty() {
                // Shade the reduced surface by position, matching the full bake's post-merge pass.
                let mesh = revolve::merge(&reduced).with_contact_cavity(&bands);
                submeshes.push(Submesh { kind, mesh });
            }
        }
        BakedVehicle::new(self.kind, submeshes, self.mounts)
    }
}

/// The base clustering cell: a fraction of the largest extent across all retained part meshes.
fn body_cell<'a>(
    meshes: impl Iterator<Item = &'a vehicle_geometry::GeometryMesh>,
    fraction: f32,
) -> f32 {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for mesh in meshes {
        if let Some(b) = mesh.bounds() {
            min = min.min(b.min);
            max = max.max(b.max);
        }
    }
    let extent = if min.x.is_finite() { (max - min).max_element() } else { 1.0 };
    (extent.max(1.0e-3) * fraction).max(1.0e-4)
}
