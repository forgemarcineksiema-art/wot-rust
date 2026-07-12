use game_core::math::segment_box_entry;
use glam::Vec3;
use terrain::StaticCoverObject;

/// First static-cover object the segment enters, if any. Cover boxes are axis-aligned, so a slab
/// test against each gives the nearest entry point along the segment.
pub(super) fn first_cover_impact(
    previous: Vec3,
    current: Vec3,
    cover: &[StaticCoverObject],
    radius_m: f32,
) -> Option<Vec3> {
    let mut nearest: Option<(f32, Vec3)> = None;
    for object in cover {
        let center = Vec3::from_array(object.center);
        let half = Vec3::from_array(object.half_extents_m);
        let swept_half = half + Vec3::splat(radius_m.max(0.0));
        if let Some(t) =
            segment_box_entry(previous, current, center - swept_half, center + swept_half)
            && nearest.is_none_or(|(best_t, _)| t < best_t)
        {
            let sphere_center = previous + (current - previous) * t;
            let contact = sphere_center.clamp(center - half, center + half);
            nearest = Some((t, contact));
        }
    }
    nearest.map(|(_, point)| point)
}
