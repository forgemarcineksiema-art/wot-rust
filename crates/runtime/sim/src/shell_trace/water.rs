use glam::Vec3;
use terrain::{HeightMap, WaterView};

/// Ignore film-thin water: a splash needs real depth under the surface, or a damp field would
/// eat shells.
const MIN_SPLASH_DEPTH_M: f32 = 0.05;

/// First crossing of ANY still-water surface along the segment, where that surface actually
/// governs the crossing point (`level_at` — a sheet inside its rect, the table elsewhere) and
/// the terrain below is genuinely submerged. Each candidate level is an exact plane, so each
/// crossing is solved analytically — no stepped sweep — and the nearest surviving crossing
/// wins. The terrain sweep still beats it wherever the ground rises above the water (banks,
/// causeways), because that impact is nearer. Side entry into a sheet cannot be missed: the
/// standing-water report gate requires every rect edge to be dry ground, so the only way
/// into a pool is down through its surface.
pub(super) fn first_water_impact(
    previous: Vec3,
    current: Vec3,
    heightmap: Option<&HeightMap>,
    water: WaterView<'_>,
    radius_m: f32,
) -> Option<Vec3> {
    let heightmap = heightmap?;
    let mut nearest: Option<(f32, Vec3)> = None;
    for level in water.levels() {
        let center_level = level + radius_m.max(0.0);
        if previous.y <= center_level || current.y > center_level {
            continue;
        }
        let t = ((previous.y - center_level) / (previous.y - current.y)).clamp(0.0, 1.0);
        let center = previous.lerp(current, t);
        // The plane only exists where this level actually governs the water column.
        if water.level_at(center.x, center.z) != Some(level) {
            continue;
        }
        let point = Vec3::new(center.x, level, center.z);
        let Some(ground) = heightmap.sample_height(point.x, point.z) else {
            continue;
        };
        if ground < level - MIN_SPLASH_DEPTH_M && nearest.is_none_or(|(best, _)| t < best) {
            nearest = Some((t, point));
        }
    }
    nearest.map(|(_, point)| point)
}
