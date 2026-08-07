//! The world's forge (Inna Liga B1): structures and procedural flora authored the same way
//! vehicles are — a deterministic bake onto the shared `GeometryMesh` contract, golden
//! hashes as the review gate, and a strict renderer-free rule so every consumer (scene
//! builder, collision footprint, destruction forms) reads ONE source of truth.
//!
//! Every shipped structure goes through the parameterised generators in `building` and
//! `tree`. Imported flora (`FloraAsset`, `import-flora`) was removed under Świat 2.0
//! (2026-08-06) — vegetation is procedural-only.

pub mod building;
pub mod tree;

use glam::Vec3;
use serde::{Deserialize, Serialize};
use vehicle_geometry::{
    GeometryMesh, GeometryVertex, MaterialRole, MeshQualityError, SmoothingGroup,
};

#[derive(Debug, thiserror::Error)]
pub enum WorldForgeError {
    #[error("blueprint parse: {0}")]
    Parse(String),
    #[error("blueprint '{name}' violates the mesh contract: {error}")]
    Quality { name: String, error: MeshQualityError },
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
            // The WORLD's timber travels as TrackMetal (the carrier bijection above); this is
            // the VEHICLE's wood.
            MaterialRole::Timber => unreachable!("vehicle timber is not a world carrier"),
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

pub(crate) fn fnv(hash: &mut u64, word: u64) {
    *hash ^= word;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
