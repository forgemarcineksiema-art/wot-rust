//! The parametric vehicle description: the part list, where parts mount, and how the whole thing
//! bakes into one `BakedVehicle` for the Forge. This is the single place that knows a vehicle is a
//! *set of parts*, each routed to the right generator and grouped into hull / turret / gun submeshes.

use game_core::{MountFrames, VehicleKind};
use vehicle_geometry::{BakedVehicle, LodLevel, Submesh, SubmeshKind};

use crate::part::VehiclePart;

/// A vehicle as a parametric part list plus its mount frames.
pub struct VehicleDescription {
    pub kind: VehicleKind,
    pub parts: Vec<VehiclePart>,
    pub mounts: MountFrames,
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
        let mut submeshes = Vec::new();
        for kind in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let meshes: Vec<_> = self
                .parts
                .iter()
                .filter(|part| part.submesh == kind && part.lod.kept_at(lod))
                .map(VehiclePart::mesh)
                .collect();
            if !meshes.is_empty() {
                submeshes.push(Submesh { kind, mesh: revolve::merge(&meshes) });
            }
        }
        BakedVehicle::new(self.kind, submeshes, self.mounts)
    }
}
