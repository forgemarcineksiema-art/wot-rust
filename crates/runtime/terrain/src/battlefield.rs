use serde::{Deserialize, Serialize};

use crate::HeightMap;
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
}

impl BattlefieldMap {
    pub fn feature(&self, kind: MapFeatureKind, name: &str) -> Option<&MapFeature> {
        self.features.iter().find(|feature| feature.kind == kind && feature.name.contains(name))
    }
}
