use game_core::{MountFrame, MountFrames, VehicleKind};
use glam::Vec3;
use thiserror::Error;

use crate::{GeometryMesh, MeshBounds};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BakeError {
    #[error("no procedural vehicle recipe for {0:?}")]
    MissingRecipe(VehicleKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubmeshKind {
    Hull,
    Turret,
    Gun,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Submesh {
    pub kind: SubmeshKind,
    pub mesh: GeometryMesh,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BakedVehicle {
    kind: VehicleKind,
    submeshes: Vec<Submesh>,
    mounts: MountFrames,
}

impl BakedVehicle {
    pub fn new(kind: VehicleKind, submeshes: Vec<Submesh>, mounts: MountFrames) -> Self {
        Self { kind, submeshes, mounts }
    }

    pub fn kind(&self) -> VehicleKind {
        self.kind
    }

    pub fn submesh(&self, kind: SubmeshKind) -> Option<&Submesh> {
        self.submeshes.iter().find(|submesh| submesh.kind == kind)
    }

    pub fn submeshes(&self) -> &[Submesh] {
        &self.submeshes
    }

    pub fn mounts(&self) -> &MountFrames {
        &self.mounts
    }

    pub fn body_bounds(&self) -> Option<MeshBounds> {
        [SubmeshKind::Hull, SubmeshKind::Turret]
            .into_iter()
            .filter_map(|kind| self.submesh(kind).and_then(|submesh| submesh.mesh.bounds()))
            .reduce(MeshBounds::union)
    }

    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash_vehicle_kind(&mut hash, self.kind);
        for submesh in &self.submeshes {
            hash_u64(&mut hash, submesh.kind as u64);
            for vertex in submesh.mesh.vertices() {
                for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array())
                {
                    hash_u64(&mut hash, value.to_bits() as u64);
                }
                hash_u64(&mut hash, vertex.uv0.x.to_bits() as u64);
                hash_u64(&mut hash, vertex.uv0.y.to_bits() as u64);
                hash_u64(&mut hash, vertex.mapping as u64);
                hash_u64(&mut hash, vertex.material as u64);
                hash_u64(&mut hash, vertex.smoothing.0 as u64);
                hash_u64(&mut hash, vertex.surface_shade.to_bits() as u64);
            }
            for index in submesh.mesh.indices() {
                hash_u64(&mut hash, u64::from(*index));
            }
        }
        hash_frame(&mut hash, &self.mounts.turret_ring);
        hash_frame(&mut hash, &self.mounts.gun_trunnion);
        hash_frame(&mut hash, &self.mounts.muzzle);
        hash
    }
}

fn hash_frame(hash: &mut u64, frame: &MountFrame) {
    hash_vec3(hash, frame.translation);
    for col in frame.basis.to_cols_array_2d() {
        for component in col {
            hash_u64(hash, component.to_bits() as u64);
        }
    }
}

fn hash_vehicle_kind(hash: &mut u64, kind: VehicleKind) {
    for byte in kind.slug().as_bytes() {
        hash_u64(hash, u64::from(*byte));
    }
}

fn hash_vec3(hash: &mut u64, value: Vec3) {
    for component in value.to_array() {
        hash_u64(hash, component.to_bits() as u64);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    *hash ^= value;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}
