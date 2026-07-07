use serde::{Deserialize, Serialize};

use crate::battlefield::BattlefieldMap;
use crate::bystra::bystra_valley;
use crate::prokhorovka::prokhorovka_hill_252_2;

/// Stable identity of a playable map — the registry both ends of the wire call.
///
/// The battlefield itself is never networked: server and client each run the same
/// deterministic generator, so agreeing on a `MapId` is what keeps their worlds identical.
/// The variant order is wire identity (bincode discriminants) — append, never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MapId {
    #[default]
    ProkhorovkaHill252_2,
    BystraValley,
}

impl MapId {
    pub const ALL: &'static [MapId] = &[MapId::ProkhorovkaHill252_2, MapId::BystraValley];

    /// CLI/asset slug: `generate-map --map <slug>` and the `assets/maps/` filename stem
    /// (with `-` mapped to `_`).
    pub fn slug(self) -> &'static str {
        match self {
            Self::ProkhorovkaHill252_2 => "prokhorovka-hill-252-2",
            Self::BystraValley => "bystra-valley",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.slug() == slug)
    }

    /// Build this map. Deterministic: every call, on any machine, yields the same battlefield.
    pub fn battlefield(self) -> BattlefieldMap {
        match self {
            Self::ProkhorovkaHill252_2 => prokhorovka_hill_252_2(),
            Self::BystraValley => bystra_valley(),
        }
    }
}
