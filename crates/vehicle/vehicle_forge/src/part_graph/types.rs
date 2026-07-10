use game_core::{MountFrame, TurretForm};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, MeshBounds};

/// Which link of the pose chain a part rides rigidly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartAnchor {
    Hull,
    TurretRing,
    GunTrunnion,
}

/// The three top-level part groups the runtime handles independently (suspension/hull, traversing
/// turret, elevating gun). A part's group follows its pose anchor, so the grouping cannot disagree
/// with how the part actually moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartGroup {
    Hull,
    Turret,
    Gun,
}

impl PartGroup {
    pub fn from_anchor(anchor: PartAnchor) -> Self {
        match anchor {
            PartAnchor::Hull => PartGroup::Hull,
            PartAnchor::TurretRing => PartGroup::Turret,
            PartAnchor::GunTrunnion => PartGroup::Gun,
        }
    }
}

/// What a part *is* for gameplay, independent of where it sits: armour the shell resolves against,
/// running gear that can be tracked/immobilised, the weapon, or cosmetic fittings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameplayRole {
    Armor,
    RunningGear,
    Weapon,
    Fitting,
}

/// How a part survives LOD reduction: characteristic silhouette forms are simplified but kept through
/// every tier; mount-bearing parts are always kept so the pose chain survives even at LOD2; detail
/// fittings are simplified at LOD1 and dropped at LOD2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodPolicy {
    Silhouette,
    MountCritical,
    Detail,
}

/// The semantic identity of a Forge-review part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForgePartKind {
    Hull,
    UpperGlacis,
    LowerPlate,
    /// A pike nose (IS-3): TWO swept bow plates meeting at the central ridge — one silhouette
    /// part, because the pair reads (and armors) as one construction.
    PikeBow,
    Fenders,
    /// Full-length side skirts (the Centurion's bazooka plates): gameplay-real spaced armor,
    /// not a fitting — `ArmorZone::Skirt` resolves on the plane this part stands on.
    Skirts,
    TrackRun,
    TrackBelt,
    RoadWheels,
    RoadWheelSet,
    Idler,
    DriveSprocket,
    /// Small top-run carrier rollers (IS family, Centurion) — running gear, distinct from the
    /// road wheels the belt rides between them.
    ReturnRollers,
    Turret,
    TurretCheeks,
    Mantlet,
    MantletSocket,
    MovingMantlet,
    Gun,
    Cupola,
    EngineDeck,
    /// The turret bustle stowage bin (Centurion Mk 3, the Tiger's Rommelkiste): external
    /// stowage riding the turret, cosmetic to the armor model.
    StowageBin,
    /// External fuel drums on the rear fenders (IS family): cosmetic stowage.
    FuelDrums,
}

impl ForgePartKind {
    /// What this part is for gameplay.
    pub fn gameplay_role(self) -> GameplayRole {
        use ForgePartKind::*;
        match self {
            Hull | UpperGlacis | LowerPlate | PikeBow | Skirts | Turret | TurretCheeks
            | Mantlet | MantletSocket | MovingMantlet => GameplayRole::Armor,
            TrackRun | TrackBelt | RoadWheels | RoadWheelSet | Idler | DriveSprocket
            | ReturnRollers => GameplayRole::RunningGear,
            Gun => GameplayRole::Weapon,
            Fenders | Cupola | EngineDeck | StowageBin | FuelDrums => GameplayRole::Fitting,
        }
    }

    /// How this part is treated through the LOD tiers.
    pub fn lod_policy(self) -> LodPolicy {
        use ForgePartKind::*;
        match self {
            Turret | Gun => LodPolicy::MountCritical,
            Hull | UpperGlacis | LowerPlate | PikeBow | Skirts | TrackRun | RoadWheels
            | TurretCheeks | MovingMantlet => LodPolicy::Silhouette,
            Fenders | TrackBelt | RoadWheelSet | Idler | DriveSprocket | ReturnRollers
            | Mantlet | MantletSocket | Cupola | EngineDeck | StowageBin | FuelDrums => {
                LodPolicy::Detail
            }
        }
    }
}

/// One semantic part: where it sits, what it is made of, and where its proportions came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ForgePart {
    pub(crate) kind: ForgePartKind,
    pub(crate) anchor: PartAnchor,
    pub(crate) material: MaterialRole,
    pub(crate) frame: MountFrame,
    pub(crate) bounds: MeshBounds,
    pub(crate) source: String,
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

    /// The top-level group this part belongs to (follows its pose anchor).
    pub fn group(&self) -> PartGroup {
        PartGroup::from_anchor(self.anchor)
    }

    /// What this part is for gameplay.
    pub fn gameplay_role(&self) -> GameplayRole {
        self.kind.gameplay_role()
    }

    /// How this part is treated through the LOD tiers.
    pub fn lod_policy(&self) -> LodPolicy {
        self.kind.lod_policy()
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
