//! The semantic part graph: a vehicle as a set of named parts, each with a gameplay anchor, a
//! material role, a local frame, bounds, and a note on where its proportions come from.
//!
//! The graph supersedes the flat `VehicleBlueprint` (the prototype). It is *derived* from that same
//! blueprint, so the parts cannot drift from the hitbox/mount source of truth — and the mount chain
//! is rebuilt from the parts (see [`ForgePartGraph::mount_frames`]), proving the parts carry the
//! pose information rather than restating magic values.

mod types;

pub use types::{ForgePart, ForgePartKind, GameplayRole, LodPolicy, PartAnchor, PartGroup};
pub(crate) use types::{part, turret_material};

use game_core::{MountFrame, MountFrames, VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

use crate::ReferencePack;
use crate::registry::PartStrategy;

/// A vehicle decomposed into semantic parts. Built from the vehicle's [`VehicleBlueprint`].
#[derive(Debug, Clone, PartialEq)]
pub struct ForgePartGraph {
    kind: VehicleKind,
    road_wheel_count_per_side: usize,
    turret_traverses: bool,
    parts: Vec<ForgePart>,
}

impl ForgePartGraph {
    /// The part graph for `kind`, dispatched on the vehicle's registered part strategy. Bespoke
    /// blueprint tables (T-54, IS-3, Centurion) derive every part extent from their single shape
    /// source; the German line uses a coarser graph derived from the baked geometry bounds and
    /// the family reference pack. `None` only if a vehicle has no forge spec. Because the
    /// strategy is registered per vehicle, a blueprint-backed
    /// vehicle can no longer silently inherit another vehicle's part table.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        let spec = crate::registry::forge_spec(kind)?;
        match spec.parts {
            PartStrategy::Blueprint(build) => {
                let blueprint = VehicleBlueprint::for_vehicle(kind)?;
                Some(ForgePartGraph {
                    kind,
                    road_wheel_count_per_side: blueprint.track.wheel_count,
                    turret_traverses: !kind.has_fixed_casemate(),
                    parts: build(&blueprint),
                })
            }
            PartStrategy::BakedGeometry => Self::from_baked_geometry(
                kind,
                &ReferencePack::embedded(kind).expect("a registered vehicle has a reference pack"),
            ),
        }
    }

    /// Derive a coarse part graph from a vehicle's baked submesh bounds plus its reference pack.
    /// This carries no new magic values: every extent comes from the geometry the recipes already
    /// produce, the running-gear count comes from the reference pack, and the commander's station
    /// comes from the blueprint that PLACED it — bounding-box fractions cannot tell you which side
    /// of the roof a cupola sits on, and guessing put the Jagdtiger's on the wrong one.
    fn from_baked_geometry(kind: VehicleKind, pack: &ReferencePack) -> Option<Self> {
        let baked = bake_vehicle(kind).ok()?;
        let blueprint = VehicleBlueprint::for_vehicle(kind)?;
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
                &blueprint.turret,
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

    /// Every part in one top-level group (hull, turret, or gun) — the slices the runtime assembles
    /// and animates independently (suspension, traverse, elevation).
    pub fn parts_in_group(&self, group: PartGroup) -> impl Iterator<Item = &ForgePart> {
        self.parts.iter().filter(move |p| p.group() == group)
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
