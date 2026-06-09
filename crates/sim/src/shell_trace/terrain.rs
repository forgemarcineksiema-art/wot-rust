use glam::Vec3;
use terrain::HeightMap;

const TERRAIN_SWEEP_STEP_M: f32 = 1.0;

/// First terrain crossing along the segment, found by a 1 m stepped sweep of ground clearance.
pub(super) fn first_terrain_impact(
    previous_position: Vec3,
    current_position: Vec3,
    heightmap: Option<&HeightMap>,
) -> Option<Vec3> {
    let heightmap = heightmap?;
    let segment = current_position - previous_position;
    let steps = (segment.length() / TERRAIN_SWEEP_STEP_M).ceil().max(1.0) as u32;
    let mut previous = previous_position;
    let mut previous_clearance = terrain_clearance(heightmap, previous);

    for step in 1..=steps {
        let current = previous_position + segment * (step as f32 / steps as f32);
        let current_clearance = terrain_clearance(heightmap, current);
        if previous_clearance > 0.0 && current_clearance <= 0.0 {
            let t = previous_clearance / (previous_clearance - current_clearance);
            return Some(previous.lerp(current, t.clamp(0.0, 1.0)));
        }
        previous = current;
        previous_clearance = current_clearance;
    }

    None
}

fn terrain_clearance(heightmap: &HeightMap, point: Vec3) -> f32 {
    heightmap.sample_height(point.x, point.z).map_or(f32::MAX, |ground| point.y - ground)
}
