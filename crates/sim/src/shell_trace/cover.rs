use game_core::math::segment_box_entry;
use glam::Vec3;
use terrain::StaticCoverObject;

/// First static-cover object the segment enters, if any. Cover boxes are axis-aligned, so a slab
/// test against each gives the nearest entry point along the segment.
pub(super) fn first_cover_impact(
    previous: Vec3,
    current: Vec3,
    cover: &[StaticCoverObject],
) -> Option<Vec3> {
    let mut nearest: Option<(f32, Vec3)> = None;
    for object in cover {
        let center = Vec3::from_array(object.center);
        let half = Vec3::from_array(object.half_extents_m);
        if let Some(t) = segment_box_entry(previous, current, center - half, center + half)
            && nearest.is_none_or(|(best_t, _)| t < best_t)
        {
            nearest = Some((t, previous + (current - previous) * t));
        }
    }
    nearest.map(|(_, point)| point)
}
