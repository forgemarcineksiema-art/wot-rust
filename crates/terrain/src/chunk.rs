use serde::{Deserialize, Serialize};

use crate::HeightMap;

pub const DEFAULT_CHUNK_SIZE_M: f32 = 128.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerrainChunkId {
    pub x: i32,
    pub z: i32,
}

impl TerrainChunkId {
    pub fn from_world_position(x: f32, _y: f32, z: f32) -> Self {
        Self {
            x: (x / DEFAULT_CHUNK_SIZE_M).floor() as i32,
            z: (z / DEFAULT_CHUNK_SIZE_M).floor() as i32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainChunk {
    pub id: TerrainChunkId,
    pub heightmap: HeightMap,
}
