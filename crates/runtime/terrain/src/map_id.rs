use serde::{Deserialize, Serialize};

/// Stable identity of a playable map — the registry both ends of the wire call.
///
/// The battlefield itself is never networked: server and client each compile the same
/// blueprint document (`map_forge::battlefield`), so agreeing on a `MapId` is what keeps
/// their worlds identical. The variant order is wire identity (bincode discriminants) —
/// append, never reorder.
///
/// The default is the map the game PLAYS. Prokhorovka stays in the registry as the test
/// substrate (replay fixtures and contract tests were recorded on it — determinism proof),
/// but every battle runs on the Bystra valley unless `WOT_MAP` explicitly says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MapId {
    ProkhorovkaHill252_2,
    #[default]
    BystraValley,
    /// The dev-only playtest map (map editor D2): it carries NO content of its own — the
    /// process loads a blueprint document from a path handed out-of-band (`WOT_MAP` set to
    /// a `.map.ron` path) before any battle uses it. Deliberately NOT in [`Self::ALL`]:
    /// the shipped catalog, the rotation, goldens and baked assets never see it. Its
    /// discriminant is frozen like every other — the variant order is wire identity, so
    /// later maps append AFTER it.
    Scratch,
    /// The mountain pass (Caucasus foothills, 1942): an impassable wall on the mirror axis
    /// with three gates for lanes, summits reachable only through the pass col. The first
    /// map born entirely in Map Forge. Plays opt-in via `WOT_MAP=orliny-pereval`.
    OrlinyPereval,
}

impl MapId {
    /// Every SHIPPED map — the catalog the rotation, goldens and baked assets cover.
    /// `Scratch` is deliberately absent (a dev vessel, not a shipped map).
    pub const ALL: &'static [MapId] =
        &[MapId::ProkhorovkaHill252_2, MapId::BystraValley, MapId::OrlinyPereval];

    /// CLI/asset slug: `generate-map --map <slug>` and the `assets/maps/` filename stem
    /// (with `-` mapped to `_`).
    pub fn slug(self) -> &'static str {
        match self {
            Self::ProkhorovkaHill252_2 => "prokhorovka-hill-252-2",
            Self::BystraValley => "bystra-valley",
            Self::Scratch => "scratch",
            Self::OrlinyPereval => "orliny-pereval",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.slug() == slug)
    }
}
