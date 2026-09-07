use glam::Vec3;
use terrain::HeightMap;

/// First terrain crossing along the segment — THE ONE MARCH (V0): `terrain::first_ground_impact`,
/// exact on the piecewise-planar surface, the same kernel the eye reads.
pub(super) fn first_terrain_impact(
    previous_position: Vec3,
    current_position: Vec3,
    heightmap: Option<&HeightMap>,
    radius_m: f32,
) -> Option<Vec3> {
    let heightmap = heightmap?;
    terrain::first_ground_impact(
        heightmap,
        previous_position.to_array(),
        current_position.to_array(),
        radius_m,
    )
    .map(Vec3::from_array)
}
