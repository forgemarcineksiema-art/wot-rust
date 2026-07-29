//! The world's forge (Inna Liga B1): structures, flora and props authored the same way vehicles
//! are — a RON blueprint per object, a deterministic bake onto the shared `GeometryMesh`
//! contract, golden hashes as the review gate, and a strict renderer-free rule so every consumer
//! (scene builder, collision footprint, destruction forms) reads ONE source of truth. This crate
//! is the multiplier for trees 2.0, the building generator and map dressing: content becomes a
//! blueprint edit, not a code change.

pub mod building;
pub mod flora;
pub mod tree;

use std::collections::BTreeMap;

use glam::Vec3;
use serde::{Deserialize, Serialize};
use vehicle_geometry::{
    GeometryMesh, GeometryVertex, MaterialRole, MeshQualityError, MeshQualitySpec, SmoothingGroup,
    TopologyExpectation,
};

#[derive(Debug, thiserror::Error)]
pub enum WorldForgeError {
    #[error("blueprint parse: {0}")]
    Parse(String),
    #[error("blueprint '{name}' bakes no geometry")]
    Empty { name: String },
    #[error("blueprint '{name}' violates the mesh contract: {error}")]
    Quality { name: String, error: MeshQualityError },
}

/// What an object IS to the battle — the scene builder and destruction rules key on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldObjectKind {
    Structure,
    Flora,
    Prop,
}

/// The world's own surface vocabulary (M2b): what a static surface IS — never a borrowed
/// vehicle `MaterialRole`. Each material carries its PBR-lite defaults (albedo + roughness);
/// the scene builder may still override color per instance (building palettes), but the
/// SEMANTIC is authored here. Wetness needs no lane of its own: the scene shader's weather
/// wet-darken/gloss-sharpen treats statics exactly like vehicles, keyed off the gloss this
/// roughness feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldMaterial {
    /// Rendered/limewashed masonry — wall bodies, towers, gable triangles, rubble slabs.
    Wall,
    /// Roof cladding courses (slate/tile/shingle), spires, cones, fallen shards.
    Roof,
    /// Untreated fieldstone plinth.
    PlinthStone,
    /// Window glass: the one thing on a wall that answers the sky.
    WindowGlass,
    /// Plank door: dark weathered joinery.
    PlankDoor,
    /// Sawn structural timber — fence runs, the windmill's cladding.
    Timber,
    /// Piled straw — haystacks, thatch.
    Straw,
    /// Tree bark.
    Bark,
    /// Tree canopy (species picks the tone; this is the fallback).
    Canopy,
}

impl WorldMaterial {
    /// PBR-lite default albedo (linear). Consumers with authored palettes (building walls
    /// and roofs vary per instance) override the color and keep the semantic.
    pub fn albedo(self) -> [f32; 3] {
        match self {
            WorldMaterial::Wall => [0.58, 0.52, 0.48],
            WorldMaterial::Roof => [0.36, 0.30, 0.22],
            WorldMaterial::PlinthStone => [0.24, 0.22, 0.20],
            WorldMaterial::WindowGlass => [0.07, 0.09, 0.11],
            WorldMaterial::PlankDoor => [0.16, 0.11, 0.07],
            WorldMaterial::Timber => [0.30, 0.25, 0.19],
            WorldMaterial::Straw => [0.45, 0.38, 0.20],
            WorldMaterial::Bark => [0.23, 0.18, 0.13],
            WorldMaterial::Canopy => [0.22, 0.30, 0.14],
        }
    }

    /// PBR-lite roughness (1 = fully matte). The scene gloss lane is `1 − roughness`.
    pub fn roughness(self) -> f32 {
        match self {
            WorldMaterial::Wall => 0.90,
            WorldMaterial::Roof => 0.72,
            WorldMaterial::PlinthStone => 0.85,
            WorldMaterial::WindowGlass => 0.55,
            WorldMaterial::PlankDoor => 0.94,
            WorldMaterial::Timber => 0.94,
            WorldMaterial::Straw => 0.97,
            WorldMaterial::Bark => 0.96,
            WorldMaterial::Canopy => 0.95,
        }
    }

    /// The carrier tag inside `GeometryVertex` — the shared mesh contract is the VEHICLE
    /// kernel's vertex format, so world materials ride its role channel through a private
    /// bijection. This is an ENCODING, not semantics: nothing outside this pair of
    /// functions may interpret a world vertex's `MaterialRole`.
    pub(crate) fn carrier(self) -> MaterialRole {
        match self {
            WorldMaterial::Wall => MaterialRole::RolledArmor,
            WorldMaterial::Roof => MaterialRole::CastArmor,
            WorldMaterial::PlinthStone => MaterialRole::BarrelSteel,
            WorldMaterial::WindowGlass => MaterialRole::InteriorMachinery,
            WorldMaterial::PlankDoor => MaterialRole::InteriorPrimer,
            WorldMaterial::Timber => MaterialRole::TrackMetal,
            WorldMaterial::Straw => MaterialRole::Rubber,
            WorldMaterial::Bark => MaterialRole::Ammunition,
            WorldMaterial::Canopy => MaterialRole::ExposedSteel,
        }
    }

    /// Decode a world vertex's carrier tag back to its material. Total — the bijection is
    /// locked by test, so every role a world bake can emit maps back to exactly one
    /// material.
    pub fn from_carrier(role: MaterialRole) -> Self {
        match role {
            MaterialRole::RolledArmor => WorldMaterial::Wall,
            MaterialRole::CastArmor => WorldMaterial::Roof,
            MaterialRole::BarrelSteel => WorldMaterial::PlinthStone,
            MaterialRole::InteriorMachinery => WorldMaterial::WindowGlass,
            MaterialRole::InteriorPrimer => WorldMaterial::PlankDoor,
            MaterialRole::TrackMetal => WorldMaterial::Timber,
            MaterialRole::Rubber => WorldMaterial::Straw,
            MaterialRole::Ammunition => WorldMaterial::Bark,
            MaterialRole::ExposedSteel => WorldMaterial::Canopy,
            // Fabric is a VEHICLE role (the mantlet's dust cover). No world bake emits it,
            // and the bijection this function locks runs the other way — from the nine world
            // materials out and back. Giving it a world material would break that lock to
            // satisfy a call that cannot happen.
            MaterialRole::Canvas => unreachable!("the world never bakes canvas"),
            MaterialRole::Glass => unreachable!("the world never bakes vehicle glass"),
        }
    }

    /// Every material, for exhaustive tests.
    pub const ALL: [WorldMaterial; 9] = [
        WorldMaterial::Wall,
        WorldMaterial::Roof,
        WorldMaterial::PlinthStone,
        WorldMaterial::WindowGlass,
        WorldMaterial::PlankDoor,
        WorldMaterial::Timber,
        WorldMaterial::Straw,
        WorldMaterial::Bark,
        WorldMaterial::Canopy,
    ];
}

/// One authored world object: a named list of parts in object-local meters, +Y up, the origin
/// on the ground plane. Deliberately the same shape-vocabulary philosophy as the vehicle
/// blueprints: a few honest primitives, composed, never free-form soup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldObjectBlueprint {
    pub name: String,
    pub kind: WorldObjectKind,
    pub parts: Vec<WorldPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldPart {
    pub name: String,
    pub material: WorldMaterial,
    pub shape: WorldShape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldShape {
    /// Axis-aligned box: `center` to `center ± half`.
    Box { center: [f32; 3], half: [f32; 3] },
    /// Upright drum (cylinder about +Y), `segments` sides.
    Drum { center: [f32; 3], radius: f32, half_height: f32, segments: u32 },
}

impl WorldObjectBlueprint {
    pub fn from_ron(source: &str) -> Result<Self, WorldForgeError> {
        ron::from_str(source).map_err(|error| WorldForgeError::Parse(error.to_string()))
    }
}

/// Deterministic bake: parts in blueprint order, welded vertices per part, hard edges — the
/// painterly low-poly read the art bible asks of world objects. The result satisfies the CLOSED
/// mesh contract part-by-part (each primitive is watertight).
pub fn bake_world_object(
    blueprint: &WorldObjectBlueprint,
) -> Result<BakedWorldObject, WorldForgeError> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for part in &blueprint.parts {
        let mesh = match &part.shape {
            WorldShape::Box { center, half } => {
                world_box_mesh(Vec3::from_array(*center), Vec3::from_array(*half), part.material)
            }
            WorldShape::Drum { center, radius, half_height, segments } => world_drum_mesh(
                Vec3::from_array(*center),
                *radius,
                *half_height,
                (*segments).clamp(3, 48),
                part.material,
            ),
        };
        let offset = vertices.len() as u32;
        vertices.extend_from_slice(mesh.vertices());
        indices.extend(mesh.indices().iter().map(|index| index + offset));
    }
    if indices.is_empty() {
        return Err(WorldForgeError::Empty { name: blueprint.name.clone() });
    }
    let mesh = GeometryMesh::new(vertices, indices);
    // Hard-edged low-poly primitives split vertices along every crease, so the INDEX topology is
    // open even though every part is geometrically watertight; the contract that matters here is
    // no degenerates, no non-manifold fans, consistent winding.
    mesh.validate_quality(MeshQualitySpec {
        topology: TopologyExpectation::Open,
        min_triangle_area: 1.0e-12,
        normal_tolerance: 1.0e-3,
        require_outward: false,
    })
    .map_err(|error| WorldForgeError::Quality { name: blueprint.name.clone(), error })?;
    Ok(BakedWorldObject { name: blueprint.name.clone(), kind: blueprint.kind, mesh })
}

#[derive(Debug, Clone)]
pub struct BakedWorldObject {
    pub name: String,
    pub kind: WorldObjectKind,
    pub mesh: GeometryMesh,
}

impl BakedWorldObject {
    /// The same FNV walk the vehicle goldens use: every vertex attribute and index, in order.
    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in self.name.as_bytes() {
            fnv(&mut hash, u64::from(*byte));
        }
        for vertex in self.mesh.vertices() {
            for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array()) {
                fnv(&mut hash, u64::from(value.to_bits()));
            }
            fnv(&mut hash, u64::from(vertex.material as u8));
        }
        for index in self.mesh.indices() {
            fnv(&mut hash, u64::from(*index));
        }
        hash
    }
}

pub(crate) fn fnv(hash: &mut u64, word: u64) {
    *hash ^= word;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}

/// The authored catalog: every blueprint shipped with the game, by name. `include_str!` keeps
/// the crate self-contained and the bake hermetic (no filesystem at runtime).
pub fn authored_blueprints() -> Result<BTreeMap<String, WorldObjectBlueprint>, WorldForgeError> {
    let mut catalog = BTreeMap::new();
    for source in [
        include_str!("../blueprints/farm_haystack.world.ron"),
        include_str!("../blueprints/wooden_fence_run.world.ron"),
    ] {
        let blueprint = WorldObjectBlueprint::from_ron(source)?;
        catalog.insert(blueprint.name.clone(), blueprint);
    }
    Ok(catalog)
}

/// The review gate, as DATA (same philosophy as `VEHICLE_BUDGETS`): the golden bake hash per
/// authored object. A change here is a deliberate look change, reviewed, never an accident.
pub const WORLD_GOLDEN_HASHES: [(&str, u64); 2] =
    [("farm_haystack", 0x055c_e95d_f794_9cc6), ("wooden_fence_run", 0x0cce_21e8_6885_d143)];

pub(crate) fn world_box_mesh(center: Vec3, half: Vec3, material: WorldMaterial) -> GeometryMesh {
    let material = material.carrier();
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    // Six faces, four welded-per-face vertices each, hard edges.
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::Y, Vec3::X),
    ];
    for (normal, u, v) in faces {
        let base = vertices.len() as u32;
        let face_center = center + normal * (half * normal.abs()).dot(normal.abs());
        let eu = u * (half * u.abs()).dot(u.abs());
        let ev = v * (half * v.abs()).dot(v.abs());
        for corner in [
            face_center - eu - ev,
            face_center + eu - ev,
            face_center + eu + ev,
            face_center - eu + ev,
        ] {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                material,
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    GeometryMesh::new(vertices, indices)
}

fn world_drum_mesh(
    center: Vec3,
    radius: f32,
    half_height: f32,
    segments: u32,
    material: WorldMaterial,
) -> GeometryMesh {
    let material = material.carrier();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let ring = |angle: f32| Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
    // Side quads (welded per segment, hard edges between).
    for segment in 0..segments {
        let a0 = segment as f32 / segments as f32 * std::f32::consts::TAU;
        let a1 = (segment + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        // Flat side facets: the mid-angle normal for the whole quad (painterly low-poly read).
        let mid = (a0 + a1) * 0.5;
        let base = vertices.len() as u32;
        for corner in [
            center + ring(a0) - Vec3::Y * half_height,
            center + ring(a1) - Vec3::Y * half_height,
            center + ring(a1) + Vec3::Y * half_height,
            center + ring(a0) + Vec3::Y * half_height,
        ] {
            vertices.push(GeometryVertex::new(
                corner,
                Vec3::new(mid.cos(), 0.0, mid.sin()),
                material,
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    // Caps as triangle fans.
    for (y, normal, flip) in [(half_height, Vec3::Y, false), (-half_height, -Vec3::Y, true)] {
        let base = vertices.len() as u32;
        for segment in 0..segments {
            let angle = segment as f32 / segments as f32 * std::f32::consts::TAU;
            vertices.push(GeometryVertex::new(
                center + ring(angle) + Vec3::Y * y,
                normal,
                material,
                SmoothingGroup::hard_edges(),
            ));
        }
        for segment in 1..segments - 1 {
            if flip {
                indices.extend_from_slice(&[base, base + segment, base + segment + 1]);
            } else {
                indices.extend_from_slice(&[base, base + segment + 1, base + segment]);
            }
        }
    }
    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_authored_object_bakes_clean_deterministic_and_on_its_golden() {
        let catalog = authored_blueprints().expect("catalog parses");
        assert_eq!(catalog.len(), WORLD_GOLDEN_HASHES.len(), "every blueprint owns a golden");
        for (name, golden) in WORLD_GOLDEN_HASHES {
            let blueprint = catalog.get(name).unwrap_or_else(|| panic!("{name} authored"));
            let first = bake_world_object(blueprint).expect("bakes clean");
            let second = bake_world_object(blueprint).expect("bakes clean twice");
            assert_eq!(first.deterministic_hash(), second.deterministic_hash(), "{name}");
            assert_eq!(
                first.deterministic_hash(),
                golden,
                "{name}: the look changed — bless the golden DELIBERATELY (0x{:016x})",
                first.deterministic_hash()
            );
            assert!(first.mesh.triangle_count() >= 12, "{name} has real geometry");
        }
    }

    /// The carrier encoding is a BIJECTION: every world material rides a distinct vehicle
    /// role and decodes back to itself. This is what lets `GeometryVertex` stay the shared
    /// mesh contract while nothing outside world_forge ever interprets a vehicle role on a
    /// world surface again.
    #[test]
    fn the_carrier_encoding_is_a_bijection() {
        let mut seen = std::collections::BTreeSet::new();
        for material in WorldMaterial::ALL {
            assert_eq!(WorldMaterial::from_carrier(material.carrier()), material);
            assert!(seen.insert(material.carrier() as u8), "{material:?} shares a carrier");
        }
    }

    /// PBR-lite discipline: every world material's defaults stay inside the art bible's
    /// world envelope — matte-leaning roughness, albedo that is paint on a real thing.
    #[test]
    fn world_materials_hold_the_pbr_lite_envelope() {
        for material in WorldMaterial::ALL {
            let roughness = material.roughness();
            assert!((0.5..=1.0).contains(&roughness), "{material:?} roughness {roughness}");
            for channel in material.albedo() {
                assert!((0.02..=0.75).contains(&channel), "{material:?} albedo {channel}");
            }
        }
    }

    #[test]
    fn a_bad_blueprint_is_refused_with_a_teaching_error() {
        let empty = WorldObjectBlueprint {
            name: "nothing".into(),
            kind: WorldObjectKind::Prop,
            parts: Vec::new(),
        };
        assert!(matches!(bake_world_object(&empty), Err(WorldForgeError::Empty { .. })));
        assert!(WorldObjectBlueprint::from_ron("(nonsense").is_err());
    }
}
