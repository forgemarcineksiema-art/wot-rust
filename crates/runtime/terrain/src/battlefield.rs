use serde::{Deserialize, Serialize};

use crate::HeightMap;
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
}

impl StaticCoverKind {
    /// Structural health before the object is destroyed; `None` is indestructible (rail
    /// embankments, pre-placed decorative wrecks — they never take damage or change phase).
    pub fn max_health(self) -> Option<u32> {
        match self {
            StaticCoverKind::FarmBuilding => Some(600),
            StaticCoverKind::TreeLine => Some(120),
            StaticCoverKind::RailCover | StaticCoverKind::Wreck => None,
        }
    }

    /// A hull driving through at speed flattens it (hedgerows/tree lines). Buildings and rail
    /// embankments do not crush — a shell has to bring them down.
    pub fn is_crushable(self) -> bool {
        matches!(self, StaticCoverKind::TreeLine)
    }

    /// When destroyed, a building slumps into a rubble mound that still blocks hulls; foliage
    /// simply vanishes. `true` = leaves a (lowered) blocking mound, `false` = goes fully clear.
    pub fn leaves_rubble(self) -> bool {
        matches!(self, StaticCoverKind::FarmBuilding)
    }

    /// The fraction of its original height a rubble mound keeps: low enough that a turret-height
    /// shot clears it, tall enough to still stop a hull.
    pub fn rubble_height_frac(self) -> f32 {
        0.4
    }
}

/// What a road is paved with — picks the painted tone and finish on the terrain mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoadSurface {
    /// Packed farm dirt: pale earth worn into the grass.
    Dirt,
    /// Railway ballast: the crushed-stone bed under the rails.
    Ballast,
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
    /// Distance from `(x, z)` to the nearest point on the polyline.
    pub fn distance_to(&self, x: f32, z: f32) -> f32 {
        let mut best_sq = f32::MAX;
        for pair in self.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (abx, abz) = (b[0] - a[0], b[1] - a[1]);
            let (apx, apz) = (x - a[0], z - a[1]);
            let len_sq = abx * abx + abz * abz;
            let t = if len_sq <= f32::EPSILON {
                0.0
            } else {
                ((apx * abx + apz * abz) / len_sq).clamp(0.0, 1.0)
            };
            let (dx, dz) = (apx - abx * t, apz - abz * t);
            best_sq = best_sq.min(dx * dx + dz * dz);
        }
        best_sq.sqrt()
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
    pub spawn_zones: Vec<SpawnZone>,
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
