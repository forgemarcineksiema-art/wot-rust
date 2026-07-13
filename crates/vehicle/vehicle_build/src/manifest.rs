//! The executable part manifest: the single production source of truth for a vehicle's parts.
//!
//! Every entry is derived from the *executable* [`VehiclePart`](crate::VehiclePart) list — its key,
//! pose group, material, LOD class, generator kind, and the bounds of the mesh that part actually
//! produces. The Forge consumes this instead of re-deriving parts from the flat blueprint fields, so
//! the three views of a vehicle (blueprint, semantic graph, executable geometry) cannot silently
//! drift: the manifest *is* the geometry.

use vehicle_geometry::{MaterialRole, MeshBounds, SubmeshKind};

use crate::description::VehicleDescription;
use crate::part::{GeneratorKind, PartKey, PartLod};

/// What a part is for gameplay, independent of where it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameplayRole {
    Armor,
    RunningGear,
    Weapon,
    Fitting,
}

/// One executable part as the Forge sees it: identity, placement, provenance, and real bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct PartManifestEntry {
    pub key: PartKey,
    pub group: SubmeshKind,
    pub material: MaterialRole,
    pub lod: PartLod,
    pub generator: GeneratorKind,
    pub role: GameplayRole,
    pub source: String,
    pub bounds: MeshBounds,
}

/// Why a part manifest is not production-ready.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum PartManifestError {
    #[error("part {0:?} has an empty source note")]
    EmptySource(&'static str),
    #[error("part {0:?} has degenerate or non-finite bounds")]
    DegenerateBounds(&'static str),
    #[error("the manifest is empty")]
    Empty,
}

/// Build the production part manifest from a vehicle description. Each part is meshed once so its
/// bounds come from the geometry it really generates — not a parallel guess at its size.
pub fn part_manifest(desc: &VehicleDescription) -> Vec<PartManifestEntry> {
    desc.parts
        .iter()
        .map(|part| {
            let bounds = part
                .mesh()
                .bounds()
                .unwrap_or(MeshBounds { min: glam::Vec3::ZERO, max: glam::Vec3::ZERO });
            let role = gameplay_role(part.material, part.lod);
            PartManifestEntry {
                key: part.key,
                group: part.submesh,
                material: part.material,
                lod: part.lod,
                generator: part.generator,
                role,
                source: source_note(part.key, part.submesh, part.generator, role),
                bounds,
            }
        })
        .collect()
}

/// Validate a manifest against the production part requirements: a non-empty source note and a
/// non-degenerate, finite volume for every part. (Group, LOD, material and generator are enums, so
/// they are "known" by construction.)
pub fn validate_manifest(entries: &[PartManifestEntry]) -> Result<(), PartManifestError> {
    if entries.is_empty() {
        return Err(PartManifestError::Empty);
    }
    for e in entries {
        if e.source.trim().is_empty() {
            return Err(PartManifestError::EmptySource(e.key.name));
        }
        let b = e.bounds;
        let degenerate = !b.min.is_finite()
            || !b.max.is_finite()
            || b.max.x <= b.min.x
            || b.max.y <= b.min.y
            || b.max.z <= b.min.z;
        if degenerate {
            return Err(PartManifestError::DegenerateBounds(e.key.name));
        }
    }
    Ok(())
}

/// A part's gameplay role, read off its material and LOD class. Detail-tier armour/track pieces are
/// cosmetic fittings; silhouette/mount armour is real armour and the road wheels are running gear.
fn gameplay_role(material: MaterialRole, lod: PartLod) -> GameplayRole {
    match material {
        MaterialRole::BarrelSteel => GameplayRole::Weapon,
        MaterialRole::Rubber => GameplayRole::RunningGear,
        MaterialRole::TrackMetal => {
            if lod == PartLod::Detail {
                GameplayRole::Fitting
            } else {
                GameplayRole::RunningGear
            }
        }
        MaterialRole::RolledArmor | MaterialRole::CastArmor => {
            if lod == PartLod::Detail {
                GameplayRole::Fitting
            } else {
                GameplayRole::Armor
            }
        }
        MaterialRole::InteriorPrimer
        | MaterialRole::InteriorMachinery
        | MaterialRole::Ammunition
        | MaterialRole::ExposedSteel => GameplayRole::Fitting,
    }
}

/// A human-readable note recording where the part's geometry comes from — its group, generator and
/// gameplay role. Never empty, so the manifest always answers "what built this and why".
fn source_note(
    key: PartKey,
    group: SubmeshKind,
    generator: GeneratorKind,
    role: GameplayRole,
) -> String {
    format!(
        "{} [{:?}] built by the `{}` kernel as {:?}",
        key.name,
        group,
        generator.kernel_name(),
        role,
    )
}
