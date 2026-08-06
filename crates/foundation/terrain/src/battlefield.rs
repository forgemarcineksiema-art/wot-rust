use serde::{Deserialize, Serialize};

use crate::HeightMap;
use crate::river::RiverSpec;
use crate::scenery::SceneryInstance;
use crate::water::WaterBody;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategicRole {
    HighGround,
    Crossing,
    Observation,
    HullDown,
    FlankRoute,
}

/// A capture zone: data first (map-editor M7) — the sim's capture rules arrive with their
/// own program; until then the zone is authored truth the editor, minimap and report see.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureZone {
    pub id: String,
    /// World position, grounded on the heightmap at authoring time.
    pub center: [f32; 3],
    pub radius_m: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategicPoint {
    pub id: String,
    pub name: String,
    pub role: StrategicRole,
    pub position: [f32; 3],
    pub radius_m: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnZone {
    pub team: u16,
    pub center: [f32; 3],
    pub radius_m: f32,
    pub facing_yaw_rad: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapFeatureKind {
    Hill,
    RailEmbankment,
    AntiTankDitch,
    Lowland,
    Farm,
    TreeLine,
    Ridge,
    Crossing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapFeature {
    pub kind: MapFeatureKind,
    pub name: String,
    pub center: [f32; 3],
    pub radius_m: f32,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticCoverKind {
    FarmBuilding,
    RailCover,
    TreeLine,
    Wreck,
    /// A wooden farm fence: waist-high posts and rails. Flimsy — one shell clears a span, and
    /// a hull simply drives THROUGH it (crushable), which is the entire point: the world
    /// reacts like matter, not like invisible walls (Fizyczny Świat P10). Appended last so
    /// baked map assets keep their discriminants.
    WoodenFence,
    /// A masonry city building (urban-map program PR-06): brick and plaster over a metre of
    /// wall, far tougher than a timber farm building, and it collapses into the same honest
    /// rubble mound. Durability is stated by the KIND in the map document, never inferred
    /// from box proportions. Appended after WoodenFence — the order is frozen.
    CityBuilding,
    /// A brick garden/compound wall: a shell breaches it, and a 30 t hull drives THROUGH it
    /// (crushable). Destroyed or crushed it goes fully clear — never a hull-blocking mound;
    /// the breach dressing is presentation, not collision.
    StoneWall,
    /// The bole of a single hero tree (hero-flora phase 3): the metre-and-a-half of bark a
    /// hull meets, a shell stops in and an eye cannot see past. It is the one kind that bakes
    /// NO box of its own — the tree's mesh IS its visual, and drawing a second solid around
    /// it would put a green pillar inside the trunk the player is looking at. Everything else
    /// it inherits from a hedgerow: crushable (a hull pushes the tree over), destructible,
    /// and its wreckage is the same stumps-and-fallen-trunk dressing. Appended last — the
    /// order is frozen.
    TreeTrunk,
}

impl StaticCoverKind {
    /// Every kind of cover a map may place. Append-only: the compiled map blueprints store these.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [StaticCoverKind; 8] = [
        StaticCoverKind::FarmBuilding,
        StaticCoverKind::RailCover,
        StaticCoverKind::TreeLine,
        StaticCoverKind::Wreck,
        StaticCoverKind::WoodenFence,
        StaticCoverKind::CityBuilding,
        StaticCoverKind::StoneWall,
        StaticCoverKind::TreeTrunk,
    ];

    /// Structural health before the object is destroyed; `None` is indestructible (rail
    /// embankments, pre-placed decorative wrecks — they never take damage or change phase).
    pub fn max_health(self) -> Option<u32> {
        match self {
            StaticCoverKind::FarmBuilding => Some(600),
            // Masonry over timber: a city block soaks 2.5 farm buildings' worth of shellfire
            // before it comes down (urban-map doctrine decision 2).
            StaticCoverKind::CityBuilding => Some(1500),
            StaticCoverKind::TreeLine => Some(120),
            // A mature oak's bole is not a hedgerow: it takes real shellfire to fell, and a
            // hull leans on it before it goes over.
            StaticCoverKind::TreeTrunk => Some(240),
            // A garden wall is bricks, not a bunker: a couple of shells open it.
            StaticCoverKind::StoneWall => Some(150),
            // Any shell sweeps a fence span away (the kinetic chip already deals 80).
            StaticCoverKind::WoodenFence => Some(40),
            StaticCoverKind::RailCover | StaticCoverKind::Wreck => None,
        }
    }

    /// A hull driving through at speed flattens it (hedgerows/tree lines, fences — and a
    /// brick garden wall under 30 t of tank). Buildings and rail embankments do not crush —
    /// a shell has to bring them down.
    pub fn is_crushable(self) -> bool {
        matches!(
            self,
            StaticCoverKind::TreeLine
                | StaticCoverKind::TreeTrunk
                | StaticCoverKind::WoodenFence
                | StaticCoverKind::StoneWall
        )
    }

    /// When destroyed, a building slumps into a rubble mound that still blocks hulls; foliage
    /// simply vanishes. `true` = leaves a (lowered) blocking mound, `false` = goes fully clear.
    /// A StoneWall deliberately leaves NO mound: a breached wall is a door, not a speed bump.
    pub fn leaves_rubble(self) -> bool {
        matches!(self, StaticCoverKind::FarmBuilding | StaticCoverKind::CityBuilding)
    }

    /// The fraction of its original height a rubble mound keeps: low enough that a turret-height
    /// shot clears it, tall enough to still stop a hull. The fraction is PER KIND because the
    /// promise is absolute, not proportional: 0.4 of a farm building is a 2.4 m pile a turret
    /// works over, but 0.4 of an 11 m tenement would be a 4.4 m wall that buries the
    /// destruction-opens-the-map promise — so masonry blocks keep their mound under turret
    /// eyes (urban-map PR-14, locked by `tests/ostrogorsk_urban.rs`).
    pub fn rubble_height_frac(self) -> f32 {
        match self {
            StaticCoverKind::CityBuilding => 0.18,
            _ => 0.4,
        }
    }
}

/// The phase byte a cover object is BORN in (urban-map program PR-07, doctrine decision 4):
/// an id containing `"ruin"` spawns already collapsed — rubble (1) if its kind leaves rubble,
/// gone (2) otherwise — while indestructible kinds ignore the tag (a "ruined" decorative
/// wreck is still just a wreck). The bytes speak the shared phase encoding every consumer
/// already reads (0 intact / 1 rubble / 2 gone), so the rule lives HERE, below both the sim
/// and every renderer-side baker, and all of them agree at birth.
pub fn born_cover_phase_byte(object: &StaticCoverObject) -> u8 {
    if !object.id.contains("ruin") || object.kind.max_health().is_none() {
        return 0;
    }
    if object.kind.leaves_rubble() { 1 } else { 2 }
}

/// One born-phase byte per cover object, index-aligned — what a pristine battle starts from.
pub fn initial_cover_phase_bytes(cover: &[StaticCoverObject]) -> Vec<u8> {
    cover.iter().map(born_cover_phase_byte).collect()
}

/// What a road is paved with — picks the painted tone and finish on the terrain mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadSurface {
    /// Packed farm dirt: pale earth worn into the grass.
    Dirt,
    /// Railway ballast: the crushed-stone bed under the rails.
    Ballast,
    /// Granite setts: the paved city street (urban-map program PR-05). Append-only, like
    /// every content enum — the variant order never changes.
    Cobble,
}

impl RoadSurface {
    /// Every road surface a map may lay. Append-only: the compiled map blueprints store these.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [RoadSurface; 3] =
        [RoadSurface::Dirt, RoadSurface::Ballast, RoadSurface::Cobble];
}

/// A render-only road: a polyline in world XZ painted onto the terrain vertices as worn
/// ground. Purely presentation — no collision, no movement bonus, no concealment change —
/// so the paint can follow the readable line without gameplay side effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub id: String,
    pub surface: RoadSurface,
    /// Polyline waypoints in world XZ, walked in order.
    pub points: Vec<[f32; 2]>,
    /// Full painted width; the tone feathers out over the outer half.
    pub width_m: f32,
}

impl Road {
    /// Distance from `(x, z)` to the nearest point on the polyline (the shared
    /// [`crate::polyline_distance`] walk — behavior-identical to the historical inline math).
    pub fn distance_to(&self, x: f32, z: f32) -> f32 {
        crate::sculpt::polyline_distance(&self.points, x, z)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticCoverObject {
    pub id: String,
    pub name: String,
    pub kind: StaticCoverKind,
    pub center: [f32; 3],
    pub half_extents_m: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattlefieldMap {
    pub id: String,
    pub name: String,
    pub size_m: [f32; 2],
    pub historical_basis: String,
    pub design_notes: Vec<String>,
    pub heightmap: HeightMap,
    /// The map's standing water, if any (see [`WaterBody`]): depth anywhere is
    /// `water.depth_over(heightmap height)`. `None` is a dry map; `serde(default)` keeps
    /// pre-water baked assets deserializing.
    #[serde(default)]
    pub water: Option<WaterBody>,
    /// The map's river centerline as data, if any (see [`RiverSpec`]): the carve, the water
    /// mesh, the minimap, bot water probes and the backdrop skirt all follow the same line.
    /// `serde(default)` keeps pre-river baked assets deserializing.
    #[serde(default)]
    pub river: Option<RiverSpec>,
    pub spawn_zones: Vec<SpawnZone>,
    /// Authored capture zones (`serde(default)` keeps pre-M7 baked assets deserializing).
    #[serde(default)]
    pub capture_zones: Vec<CaptureZone>,
    pub strategic_points: Vec<StrategicPoint>,
    pub features: Vec<MapFeature>,
    pub static_cover: Vec<StaticCoverObject>,
    /// Render-only dressing (trees, rocks): never collides, never blocks shells or sight.
    /// `serde(default)` keeps pre-scenery baked assets deserializing.
    #[serde(default)]
    pub scenery: Vec<SceneryInstance>,
    /// Render-only roads painted onto the terrain (see [`Road`]). `serde(default)` keeps
    /// pre-road baked assets deserializing.
    #[serde(default)]
    pub roads: Vec<Road>,
}

impl BattlefieldMap {
    pub fn feature(&self, kind: MapFeatureKind, name: &str) -> Option<&MapFeature> {
        self.features.iter().find(|feature| feature.kind == kind && feature.name.contains(name))
    }
}

#[cfg(test)]
mod born_phase_tests {
    use super::*;

    fn object(id: &str, kind: StaticCoverKind) -> StaticCoverObject {
        StaticCoverObject {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            center: [0.0, 2.0, 0.0],
            half_extents_m: [4.0, 2.0, 3.0],
        }
    }

    /// The birth rule (urban-map PR-07): "ruin" in the id collapses a destructible object at
    /// birth - to rubble if its kind leaves rubble, to gone otherwise - and indestructible
    /// kinds ignore the tag entirely.
    #[test]
    fn ruin_ids_are_born_collapsed_by_their_kind() {
        assert_eq!(
            born_cover_phase_byte(&object("tenement_ruin_a", StaticCoverKind::CityBuilding)),
            1
        );
        assert_eq!(born_cover_phase_byte(&object("ruined_barn", StaticCoverKind::FarmBuilding)), 1);
        assert_eq!(born_cover_phase_byte(&object("yard_wall_ruin", StaticCoverKind::StoneWall)), 2);
        assert_eq!(born_cover_phase_byte(&object("ruined_hedge", StaticCoverKind::TreeLine)), 2);
        assert_eq!(born_cover_phase_byte(&object("ruined_wreck", StaticCoverKind::Wreck)), 0);
        assert_eq!(born_cover_phase_byte(&object("ruin_wall", StaticCoverKind::RailCover)), 0);
        assert_eq!(born_cover_phase_byte(&object("tenement_a", StaticCoverKind::CityBuilding)), 0);
        assert_eq!(
            initial_cover_phase_bytes(&[
                object("tenement_a", StaticCoverKind::CityBuilding),
                object("tenement_ruin_b", StaticCoverKind::CityBuilding),
            ]),
            vec![0, 1]
        );
    }
}
