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

/// The semantic identity of a Forge-review part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForgePartKind {
    Hull,
    UpperGlacis,
    LowerPlate,
    Fenders,
    TrackRun,
    TrackBelt,
    RoadWheels,
    RoadWheelSet,
    Idler,
    DriveSprocket,
    Turret,
    TurretCheeks,
    Mantlet,
    MantletSocket,
    MovingMantlet,
    Gun,
    Cupola,
    EngineDeck,
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
