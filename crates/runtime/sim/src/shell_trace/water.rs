use glam::Vec3;
use terrain::{HeightMap, WaterBody};

/// Ignore film-thin water: a splash needs real depth under the surface, or a damp field would
/// eat shells.
const MIN_SPLASH_DEPTH_M: f32 = 0.05;

/// First crossing of the still-water surface along the segment, where the terrain below is
/// actually submerged. The surface is an exact plane, so the crossing is solved analytically —
/// no stepped sweep — and the terrain sweep still wins wherever the ground rises above the
/// water (banks, causeways) because that impact is nearer.
pub(super) fn first_water_impact(
    previous: Vec3,
    current: Vec3,
    heightmap: Option<&HeightMap>,
    water: Option<WaterBody>,
    radius_m: f32,
) -> Option<Vec3> {
    let heightmap = heightmap?;
    let level = water?.surface_level_m;
    let center_level = level + radius_m.max(0.0);
    if previous.y <= center_level || current.y > center_level {
        return None;
    }
    let t = (previous.y - center_level) / (previous.y - current.y);
    let center = previous.lerp(current, t.clamp(0.0, 1.0));
    let point = Vec3::new(center.x, level, center.z);
    let ground = heightmap.sample_height(point.x, point.z)?;
    (ground < level - MIN_SPLASH_DEPTH_M).then_some(point)
}
