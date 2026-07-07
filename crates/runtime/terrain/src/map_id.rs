use serde::{Deserialize, Serialize};

use crate::battlefield::BattlefieldMap;
use crate::bystra::bystra_valley;
use crate::prokhorovka::prokhorovka_hill_252_2;

/// Stable identity of a playable map — the registry both ends of the wire call.
///
/// The battlefield itself is never networked: server and client each run the same
/// deterministic generator, so agreeing on a `MapId` is what keeps their worlds identical.
/// The variant order is wire identity (bincode discriminants) — append, never reorder.
///
/// The default is the map the game PLAYS. Prokhorovka stays in the registry as the test
/// substrate (replay fixtures and contract tests were recorded on it — determinism proof),
/// but every battle runs on the Bystra valley unless `WOT_MAP` explicitly says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MapId {
    ProkhorovkaHill252_2,
    #[default]
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
