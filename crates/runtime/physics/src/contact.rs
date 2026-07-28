use game_core::math::horizontal_forward;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use terrain::{HeightMap, RubbleMound};

/// The ground a hull stands on for one tick: its height and the local slope/roughness/traction the
/// rigid-body integrator resolves forces against. `forward_slope`/`side_slope` are rise/run along
/// the hull's facing and right axes, so together they are the terrain gradient in the hull frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerrainContact {
    pub height_m: f32,
    pub forward_slope: f32,
    pub side_slope: f32,
    pub roughness: f32,
    pub traction: f32,
    /// Standing water over this ground (map water level − terrain height; see
    /// [`crate::water`]). Zero on dry maps — `serde(default)` keeps older fixtures loading dry.
    #[serde(default)]
    pub water_depth_m: f32,
}

impl TerrainContact {
    pub fn flat(height_m: f32) -> Self {
        Self {
            height_m,
            forward_slope: 0.0,
            side_slope: 0.0,
            roughness: 0.0,
            traction: 1.0,
            water_depth_m: 0.0,
        }
    }
}

/// Sample the ground the DRIVE resolves its forces against. The probe cross must read the same
/// surface the support envelope rests on, debris included: `forward_slope` is what gravity, the
/// grip cap and the momentum-climb ceiling are computed from, so a mound that raised the hull but
/// left the probes on flat terrain would be a pile you climb for free.
pub fn sample_tank_terrain_contact(
    heightmap: &HeightMap,
    position: Vec3,
    yaw_rad: f32,
    probe_length_m: f32,
    rubble: &[RubbleMound],
) -> Option<TerrainContact> {
    let probe = probe_length_m.max(heightmap.cell_size_m() * 0.5).max(0.5);
    let forward = horizontal_forward(yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let center = ground(heightmap, position, rubble)?;
    let front = sample_offset(heightmap, position, forward, probe, rubble).unwrap_or(center);
    let back = sample_offset(heightmap, position, -forward, probe, rubble).unwrap_or(center);
    let right_h = sample_offset(heightmap, position, right, probe, rubble).unwrap_or(center);
    let left_h = sample_offset(heightmap, position, -right, probe, rubble).unwrap_or(center);
    let forward_slope = (front - back) / (probe * 2.0);
    let side_slope = (right_h - left_h) / (probe * 2.0);
    let roughness = [front, back, right_h, left_h]
        .into_iter()
        .map(|height| (height - center).abs() / probe)
        .fold(0.0, f32::max)
        .clamp(0.0, 1.0);
    let traction = (1.0 - roughness * 0.45 - side_slope.abs() * 0.35 - forward_slope.abs() * 0.15)
        .clamp(0.35, 1.0);

    Some(TerrainContact {
        height_m: center,
        forward_slope,
        side_slope,
        roughness,
        traction,
        water_depth_m: 0.0,
    })
}

fn sample_offset(
    heightmap: &HeightMap,
    position: Vec3,
    direction: Vec3,
    distance: f32,
    rubble: &[RubbleMound],
) -> Option<f32> {
    ground(heightmap, position + direction * distance, rubble)
}

fn ground(heightmap: &HeightMap, point: Vec3, rubble: &[RubbleMound]) -> Option<f32> {
    let terrain = heightmap.sample_height(point.x, point.z);
    if rubble.is_empty() {
        return terrain;
    }
    terrain::ground_with_rubble(terrain, terrain::rubble_height_at(rubble, point.x, point.z))
}
