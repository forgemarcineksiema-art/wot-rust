//! The semantic part graph: a vehicle as a set of named parts, each with a gameplay anchor, a
//! material role, a local frame, bounds, and a note on where its proportions come from.
//!
//! The graph supersedes the flat `VehicleBlueprint` (the prototype). It is *derived* from that same
//! blueprint, so the parts cannot drift from the hitbox/mount source of truth — and the mount chain
//! is rebuilt from the parts (see [`ForgePartGraph::mount_frames`]), proving the parts carry the
//! pose information rather than restating magic values.

mod types;

pub use types::{ForgePart, ForgePartKind, PartAnchor};
pub(crate) use types::{part, turret_material};

use game_core::{MountFrame, MountFrames, VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_geometry::{SubmeshKind, bake_vehicle};

use crate::ReferencePack;

/// A vehicle decomposed into semantic parts. Built from the vehicle's [`VehicleBlueprint`].
#[derive(Debug, Clone, PartialEq)]
pub struct ForgePartGraph {
    kind: VehicleKind,
    road_wheel_count_per_side: usize,
    turret_traverses: bool,
    parts: Vec<ForgePart>,
}

impl ForgePartGraph {
    /// The part graph for `kind`. Blueprint-backed benchmarks (currently T-54) derive every
    /// part extent from the single shape source; the rest fall back to a coarser graph derived from
    /// the baked geometry bounds and the family reference pack. `None` only for vehicles that have
    /// neither a blueprint nor a reference pack (e.g. the placeholder prototype).
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        let _pack = ReferencePack::for_vehicle(kind)?;
        if let Some(blueprint) = VehicleBlueprint::for_vehicle(kind) {
            return Some(build_graph(kind, &blueprint));
        }
        Self::from_baked_geometry(kind)
    }

    /// Derive a coarse part graph from a vehicle's baked submesh bounds plus its reference pack.
    /// This carries no new magic values: every extent comes from the geometry the recipes already
    /// produce, and the running-gear count comes from the reference pack.
    fn from_baked_geometry(kind: VehicleKind) -> Option<Self> {
        let pack = ReferencePack::for_vehicle(kind)?;
        let baked = bake_vehicle(kind).ok()?;
        let bounds = |sub| baked.submesh(sub).and_then(|s| s.mesh.bounds());
        let hull = bounds(SubmeshKind::Hull)?;
        let turret = bounds(SubmeshKind::Turret)?;
        let gun = bounds(SubmeshKind::Gun)?;
        let traverses = !kind.has_fixed_casemate();

        Some(ForgePartGraph {
            kind,
            road_wheel_count_per_side: pack.road_wheel_count_per_side(),
            turret_traverses: traverses,
            parts: crate::part_data::geometry_derived_parts(
                pack.road_wheel_count_per_side(),
                traverses,
                hull,
                turret,
                gun,
                baked.mounts(),
            ),
        })
    }

    pub fn kind(&self) -> VehicleKind {
        self.kind
    }

    pub fn parts(&self) -> &[ForgePart] {
        &self.parts
    }

    pub fn part(&self, kind: ForgePartKind) -> Option<&ForgePart> {
        self.parts.iter().find(|part| part.kind == kind)
    }

    pub fn road_wheel_count_per_side(&self) -> usize {
        self.road_wheel_count_per_side
    }

    pub fn turret_traverses(&self) -> bool {
        self.turret_traverses
    }

    /// Rebuild the mount chain from the parts: the turret ring is the turret part's frame, the
    /// trunnion is the gun part's frame, and the muzzle is the gun frame carried to the barrel tip.
    pub fn mount_frames(&self) -> MountFrames {
        let turret = self.part(ForgePartKind::Turret).expect("turret part");
        let gun = self.part(ForgePartKind::Gun).expect("gun part");
        let muzzle = Vec3::new(0.0, gun.frame.translation.y, gun.bounds.max.z);
        MountFrames {
            turret_ring: turret.frame,
            gun_trunnion: gun.frame,
            muzzle: MountFrame::new(muzzle),
        }
    }

    /// A human-readable audit: which parts exist, their role/material, and the source of their
    /// proportions. This answers the Milestone 2 acceptance question directly.
    pub fn part_report(&self) -> String {
        let mut out = format!(
            "# {} Forge part graph\n\nParts: {} · road wheels/side: {} · turret traverses: {}\n\n",
            self.kind.display_name(),
            self.parts.len(),
            self.road_wheel_count_per_side,
            self.turret_traverses
        );
        out.push_str("| Part | Anchor | Material | Source |\n| --- | --- | --- | --- |\n");
        for part in &self.parts {
            out.push_str(&format!(
                "| {:?} | {:?} | {:?} | {} |\n",
                part.kind, part.anchor, part.material, part.source
            ));
        }
        out
    }
}

fn build_graph(kind: VehicleKind, bp: &VehicleBlueprint) -> ForgePartGraph {
    ForgePartGraph {
        kind,
        road_wheel_count_per_side: bp.track.wheel_count,
        turret_traverses: !kind.has_fixed_casemate(),
        parts: crate::part_data::t54_family_parts(bp),
    }
}
