use serde::{Deserialize, Serialize};
use vehicle_geometry::LodLevel;

use super::ArtifactError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BakeProfile {
    Lod0,
    Lod1,
    Lod2,
}

impl BakeProfile {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Lod0 => "lod0",
            Self::Lod1 => "lod1",
            Self::Lod2 => "lod2",
        }
    }

    /// The geometry-kernel LOD tier this bake profile selects.
    pub fn lod_level(self) -> LodLevel {
        match self {
            Self::Lod0 => LodLevel::Lod0,
            Self::Lod1 => LodLevel::Lod1,
            Self::Lod2 => LodLevel::Lod2,
        }
    }
}

impl std::str::FromStr for BakeProfile {
    type Err = ArtifactError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "lod0" => Ok(Self::Lod0),
            "lod1" => Ok(Self::Lod1),
            "lod2" => Ok(Self::Lod2),
            other => Err(ArtifactError::UnknownProfile(other.to_string())),
        }
    }
}
