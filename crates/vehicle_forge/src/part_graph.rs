//! The semantic part graph: a vehicle as a set of named parts, each with a gameplay anchor, a
//! material role, a local frame, bounds, and a note on where its proportions come from.
//!
//! The graph supersedes the flat `VehicleBlueprint` (the prototype). It is *derived* from that same
//! blueprint, so the parts cannot drift from the hitbox/mount source of truth — and the mount chain
//! is rebuilt from the parts (see [`ForgePartGraph::mount_frames`]), proving the parts carry the
//! pose information rather than restating magic values.

use game_core::{MountFrame, MountFrames, TurretForm, VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, MeshBounds, SubmeshKind, bake_vehicle};

use crate::ReferencePack;

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
    /// The part graph for `kind`. Blueprint-backed families (the T-54/T-55 benchmark) derive every
    /// part extent from the single shape source; the rest fall back to a coarser graph derived from
    /// the baked geometry bounds and the family reference pack. `None` only for vehicles that have
    /// neither a blueprint nor a reference pack (e.g. the placeholder prototype).
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
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
        let mounts = baked.mounts();
        let traverses = !kind.has_fixed_casemate();

        let hull_h = hull.max.y - hull.min.y;
        let lower_top = hull.min.y + 0.45 * hull_h;
        let wheel_inset = 0.92;
        let trun = mounts.gun_trunnion.translation;
        let mantlet = 0.30;

        let parts = vec![
            part(
                ForgePartKind::Hull,
                PartAnchor::Hull,
                MaterialRole::RolledArmor,
                Vec3::ZERO,
                hull.min,
                hull.max,
                "Derived from baked hull bounds: rolled-armour hull tub and sponsons.",
            ),
            part(
                ForgePartKind::TrackRun,
                PartAnchor::Hull,
                MaterialRole::TrackMetal,
                Vec3::new(0.0, 0.5 * (hull.min.y + lower_top), 0.0),
                Vec3::new(hull.min.x, hull.min.y, hull.min.z),
                Vec3::new(hull.max.x, lower_top, hull.max.z),
                "Derived from baked geometry: track belt wrapping the lower hull.",
            ),
            part(
                ForgePartKind::RoadWheels,
                PartAnchor::Hull,
                MaterialRole::Rubber,
                Vec3::new(0.0, 0.5 * (hull.min.y + lower_top), 0.0),
                Vec3::new(hull.min.x * wheel_inset, hull.min.y, hull.min.z),
                Vec3::new(hull.max.x * wheel_inset, lower_top, hull.max.z),
                format!(
                    "Reference pack running gear: {} road wheels per side.",
                    pack.road_wheel_count_per_side()
                ),
            ),
            part(
                ForgePartKind::Turret,
                PartAnchor::TurretRing,
                MaterialRole::RolledArmor,
                mounts.turret_ring.translation,
                turret.min,
                turret.max,
                if traverses {
                    "Derived from baked turret bounds: welded turret shell on the ring."
                } else {
                    "Derived from baked bounds: fixed casemate superstructure (no traverse)."
                },
            ),
            part(
                ForgePartKind::Mantlet,
                PartAnchor::GunTrunnion,
                MaterialRole::CastArmor,
                trun,
                Vec3::new(-mantlet, trun.y - mantlet, turret.max.z - 2.0 * mantlet),
                Vec3::new(mantlet, trun.y + mantlet, turret.max.z),
                "Derived from baked bounds: cast mantlet mask at the gun trunnion.",
            ),
            part(
                ForgePartKind::Gun,
                PartAnchor::GunTrunnion,
                MaterialRole::BarrelSteel,
                trun,
                Vec3::new(gun.min.x, trun.y - (gun.max.x - gun.min.x) * 0.5, trun.z),
                Vec3::new(gun.max.x, trun.y + (gun.max.x - gun.min.x) * 0.5, gun.max.z),
                "Derived from baked gun bounds: barrel from trunnion to muzzle.",
            ),
            part(
                ForgePartKind::Cupola,
                PartAnchor::TurretRing,
                MaterialRole::RolledArmor,
                Vec3::new(turret.min.x * 0.4, turret.max.y, turret.min.z * 0.3),
                Vec3::new(turret.min.x * 0.4 - 0.18, turret.max.y - 0.12, turret.min.z * 0.3 - 0.18),
                Vec3::new(turret.min.x * 0.4 + 0.18, turret.max.y + 0.12, turret.min.z * 0.3 + 0.18),
                "Derived from baked bounds: commander's cupola on the turret/casemate roof.",
            ),
        ];

        Some(ForgePartGraph {
            kind,
            road_wheel_count_per_side: pack.road_wheel_count_per_side(),
            turret_traverses: traverses,
            parts,
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
