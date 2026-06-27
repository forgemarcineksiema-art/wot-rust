//! The parametric vehicle description: the part list, where parts mount, and how the whole thing
//! bakes into one `BakedVehicle` for the Forge. This is the single place that knows a vehicle is a
//! *set of parts*, each routed to the right generator and grouped into hull / turret / gun submeshes.

use game_core::{MountFrames, VehicleKind};
use glam::Vec3;
use vehicle_geometry::{BakedVehicle, LodLevel, Submesh, SubmeshKind, reduce_mesh};

use crate::part::VehiclePart;
use crate::surface_bake::SurfaceBake;

/// A vehicle as a parametric part list plus its mount frames.
pub struct VehicleDescription {
    pub kind: VehicleKind,
    pub parts: Vec<VehiclePart>,
    pub mounts: MountFrames,
    /// Semantic contact cavities baked into `surface_shade` after merge (empty = flat shade).
    pub surface_bake: SurfaceBake,
}

impl VehicleDescription {
    /// Mesh every part, merge by submesh kind, and assemble the `BakedVehicle` the Forge consumes.
    /// This is the full-detail LOD0 bake.
    pub fn build(&self) -> BakedVehicle {
        self.build_lod(LodLevel::Lod0)
    }

    /// Bake only the parts whose LOD policy keeps them in `lod`: LOD0 carries everything, LOD1/LOD2
    /// drop the detail fittings and track links, leaving the silhouette and the mount-bearing parts.
    /// Triangle decimation within a tier is left to `vehicle_geometry::reduce_vehicle`.
    pub fn build_lod(&self, lod: LodLevel) -> BakedVehicle {
        let bands = self.surface_bake.bands();
        let mut submeshes = Vec::new();
        for kind in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let meshes: Vec<_> = self
                .parts
                .iter()
                .filter(|part| part.submesh == kind && part.lod.kept_at(lod))
                .map(VehiclePart::mesh)
                .collect();
            if !meshes.is_empty() {
                let mesh = revolve::merge(&meshes).with_contact_cavity(&bands);
                submeshes.push(Submesh { kind, mesh });
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

        let bands = self.surface_bake.bands();
        let mut submeshes = Vec::new();
        for kind in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
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
