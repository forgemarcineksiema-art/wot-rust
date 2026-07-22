use game_core::math::{segment_box_entry, segment_xz_disjoint};
use glam::Vec3;
use terrain::StaticCoverObject;

/// First static-cover object the segment enters, if any. Cover boxes are axis-aligned, so a slab
/// test against each gives the nearest entry point along the segment. The XZ-rect broadphase
/// (against the radius-swept footprint) skips the boxes nowhere near this shell segment — on an
/// urban map that is most of them.
pub(super) fn first_cover_impact(
    previous: Vec3,
    current: Vec3,
    cover: &[StaticCoverObject],
    radius_m: f32,
) -> Option<Vec3> {
    let radius = radius_m.max(0.0);
    let mut nearest: Option<(f32, Vec3)> = None;
    for object in cover {
        if segment_xz_disjoint(
            previous,
            current,
            object.center[0],
            object.center[2],
            object.half_extents_m[0] + radius,
            object.half_extents_m[2] + radius,
        ) {
            continue;
        }
        let center = Vec3::from_array(object.center);
        let half = Vec3::from_array(object.half_extents_m);
        let swept_half = half + Vec3::splat(radius);
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

#[cfg(test)]
mod broadphase_tests {
    use super::*;
    use terrain::{StaticCoverKind, StaticCoverObject};

    /// The exact reference: the same nearest-entry walk with NO prefilter.
    fn first_cover_impact_exact(
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

    fn xorshift(state: &mut u32) -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        (*state % 10_000) as f32 / 10_000.0
    }

    /// The XZ broadphase must be invisible in results: over hundreds of random shell
    /// segments (flat, plunging, short and long, swept by a real projectile radius) the
    /// prefiltered impact equals the exact walk bit for bit.
    #[test]
    fn the_prefilter_never_changes_a_shell_impact() {
        let mut cover = Vec::new();
        for column in 0..10 {
            for row in 0..15 {
                cover.push(StaticCoverObject {
                    id: format!("block_c{column}_r{row}"),
                    name: format!("block {column}/{row}"),
                    kind: StaticCoverKind::FarmBuilding,
                    center: [60.0 + column as f32 * 42.0, 4.0, 60.0 + row as f32 * 30.0],
                    half_extents_m: [8.0 + (row % 3) as f32, 4.0, 5.0 + (column % 2) as f32],
                });
            }
        }
        let mut state = 0x9e37_79b9u32;
        for _ in 0..600 {
            let previous = Vec3::new(
                xorshift(&mut state) * 520.0,
                xorshift(&mut state) * 25.0,
                xorshift(&mut state) * 520.0,
            );
            let current = Vec3::new(
                xorshift(&mut state) * 520.0,
                xorshift(&mut state) * 25.0 - 3.0,
                xorshift(&mut state) * 520.0,
            );
            let radius = xorshift(&mut state) * 0.12;
            assert_eq!(
                first_cover_impact(previous, current, &cover, radius),
                first_cover_impact_exact(previous, current, &cover, radius),
                "prefilter changed the impact for {previous:?} -> {current:?} r {radius}"
            );
        }
    }
}
