//! The semantic part graph: a vehicle as a set of named parts, each with a gameplay anchor, a
//! material role, a local frame, bounds, and a note on where its proportions come from.
//!
//! The graph supersedes the flat `VehicleBlueprint` (the prototype). It is *derived* from that same
//! blueprint, so the parts cannot drift from the hitbox/mount source of truth — and the mount chain
//! is rebuilt from the parts (see [`ForgePartGraph::mount_frames`]), proving the parts carry the
//! pose information rather than restating magic values.

use game_core::{MountFrame, MountFrames, TurretForm, VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, MeshBounds};

/// Which link of the pose chain a part rides rigidly: hull origin, the traversing turret ring, or
/// the elevating gun trunnion. This is the part's gameplay role with respect to motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartAnchor {
    Hull,
    TurretRing,
    GunTrunnion,
}

/// The semantic identity of a part. Deliberately the coarse set the T-54/T-55 family reads as from
/// gameplay distance; finer fittings (hatches, handles, welds) arrive with the geometry operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForgePartKind {
    Hull,
    TrackRun,
    RoadWheels,
    Turret,
    Mantlet,
    Gun,
    Cupola,
}

/// One semantic part: where it sits, what it is made of, and where its proportions came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ForgePart {
    kind: ForgePartKind,
    anchor: PartAnchor,
    material: MaterialRole,
    frame: MountFrame,
    bounds: MeshBounds,
    source: String,
}

impl ForgePart {
    pub fn kind(&self) -> ForgePartKind {
        self.kind
    }

    pub fn anchor(&self) -> PartAnchor {
        self.anchor
    }

    pub fn material(&self) -> MaterialRole {
        self.material
    }

    pub fn frame(&self) -> MountFrame {
        self.frame
    }

    pub fn bounds(&self) -> MeshBounds {
        self.bounds
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

/// A vehicle decomposed into semantic parts. Built from the vehicle's [`VehicleBlueprint`].
#[derive(Debug, Clone, PartialEq)]
pub struct ForgePartGraph {
    kind: VehicleKind,
    road_wheel_count_per_side: usize,
    turret_traverses: bool,
    parts: Vec<ForgePart>,
}

impl ForgePartGraph {
    /// The part graph for `kind`, or `None` if it has not been migrated onto a blueprint yet.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        let blueprint = VehicleBlueprint::for_vehicle(kind)?;
        Some(build_graph(kind, &blueprint))
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

pub(crate) fn part(
    kind: ForgePartKind,
    anchor: PartAnchor,
    material: MaterialRole,
    frame: Vec3,
    min: Vec3,
    max: Vec3,
    source: impl Into<String>,
) -> ForgePart {
    ForgePart {
        kind,
        anchor,
        material,
        frame: MountFrame::new(frame),
        bounds: MeshBounds { min, max },
        source: source.into(),
    }
}

pub(crate) fn turret_material(form: TurretForm) -> MaterialRole {
    match form {
        TurretForm::CastDome => MaterialRole::CastArmor,
        TurretForm::WeldedBox | TurretForm::Casemate => MaterialRole::RolledArmor,
    }
}
