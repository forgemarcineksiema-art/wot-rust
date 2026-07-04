//! The running-gear support envelope: terrain sampled at the vehicle's road-wheel stations, with
//! the hull resting as a rigid beam on the highest supported points. This is what makes
//! tank-shaped ground contact emerge — trenches narrower than the wheel pitch are bridged instead
//! of swallowed, and a nose pushed past a crest keeps riding the last support instead of diving
//! with the centre sample. See `docs/vehicle-movement-policy.md`, "Hull Attitude and the Support
//! Envelope".

use game_core::ContactFootprint;
use game_core::math::horizontal_forward;
use glam::Vec3;
use terrain::HeightMap;

/// Ground height under the hull origin when the rigid running gear rests on the terrain, or
/// `None` when no station lands on the map (the caller falls back to the centre probe).
///
/// Mechanism: each station samples the ground under both tracks and keeps the higher side (a
/// rigid frame rests on whichever track touches first). The upper convex hull of the
/// `(station_z, ground)` profile is the shape a rigid beam can actually rest on — samples below
/// it (a trench between wheels) never touch the beam. The support height at the origin is that
/// hull evaluated at `z = 0`, extrapolated along the last segment when the origin has passed the
/// final support (the crest overhang: the hull keeps its ride line until the geometry, not the
/// centre sample, says it drops).
pub fn support_height(
    heightmap: &HeightMap,
    position: Vec3,
    yaw_rad: f32,
    footprint: &ContactFootprint,
) -> Option<f32> {
    let stations = footprint.station_zs();
    if stations.is_empty() {
        return None;
    }
    let forward = horizontal_forward(yaw_rad);
    let right = Vec3::new(forward.z, 0.0, -forward.x);

    // Per-station ground line: the higher of the two track samples (skip stations off the map).
    let mut profile: [(f32, f32); game_core::MAX_CONTACT_STATIONS] =
        [(0.0, 0.0); game_core::MAX_CONTACT_STATIONS];
    let mut count = 0;
    for &station_z in stations {
        let centre = position + forward * station_z;
        let left = sample(heightmap, centre - right * footprint.half_gauge_x);
        let right_h = sample(heightmap, centre + right * footprint.half_gauge_x);
        let ground = match (left, right_h) {
            (Some(l), Some(r)) => l.max(r),
            (Some(l), None) => l,
            (None, Some(r)) => r,
            (None, None) => continue,
        };
        profile[count] = (station_z, ground);
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let profile = &profile[..count];

    // Upper convex hull over the (z, ground) profile (stations are in ascending z from the
    // blueprint). A monotone chain keeping only right turns leaves the segments a rigid beam can
    // rest on.
    let mut hull: [(f32, f32); game_core::MAX_CONTACT_STATIONS] =
        [(0.0, 0.0); game_core::MAX_CONTACT_STATIONS];
    let mut len = 0;
    for &point in profile {
        while len >= 2 && !turns_right(hull[len - 2], hull[len - 1], point) {
            len -= 1;
        }
        hull[len] = point;
        len += 1;
    }
    let hull = &hull[..len];

    Some(height_on_hull(hull, 0.0))
}

fn sample(heightmap: &HeightMap, point: Vec3) -> Option<f32> {
    heightmap.sample_height(point.x, point.z)
}

/// True when `a -> b -> c` turns clockwise in the (z, height) plane — the upper-hull keep rule.
fn turns_right(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0) < 0.0
}

/// Height of the upper hull at `z`, extrapolating the end segments beyond the station span (the
/// overhang read: past the last support the beam continues on the last resting slope).
fn height_on_hull(hull: &[(f32, f32)], z: f32) -> f32 {
    if hull.len() == 1 {
        return hull[0].1;
    }
    let segment = hull
        .windows(2)
        .find(|pair| z <= pair[1].0)
        .or_else(|| hull.windows(2).last())
        .expect("hull has at least two points");
    let (a, b) = (segment[0], segment[1]);
    let span = (b.0 - a.0).max(1.0e-6);
    let t = (z - a.0) / span;
    a.1 + (b.1 - a.1) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{HitboxProfile, VehicleKind};

    fn map_from(height_at: impl Fn(f32, f32) -> f32) -> HeightMap {
        let mut samples = Vec::with_capacity(61 * 61);
        for z in 0..61 {
            for x in 0..61 {
                samples.push(height_at(x as f32, z as f32));
            }
        }
        HeightMap::new(61, 61, 1.0, samples).expect("test heightmap dimensions are fixed")
    }

    fn t54() -> ContactFootprint {
        ContactFootprint::for_vehicle(VehicleKind::T54_1951)
    }

    #[test]
    fn a_narrow_trench_under_the_hull_is_bridged() {
        // A 1.6 m trench: wider than one cell, narrower than the ~1 m wheel pitch spans it —
        // wheels sit on both rims, so the support line stays at the rim height.
        let map = map_from(|_, z| if (29.2..30.8).contains(&z) { -2.0 } else { 1.0 });
        let height = support_height(&map, Vec3::new(30.0, 1.0, 30.0), 0.0, &t54())
            .expect("stations on the map");
        assert!(
            (height - 1.0).abs() < 0.05,
            "the rigid gear must bridge the trench, got {height}"
        );
        // The centre sample alone would have dropped the hull into it.
        assert!(map.sample_height(30.0, 30.0).unwrap() < -1.0);
    }

    #[test]
    fn a_pit_wider_than_the_wheelbase_swallows_the_hull() {
        let map = map_from(|_, z| if (24.0..36.0).contains(&z) { -2.0 } else { 1.0 });
        let height = support_height(&map, Vec3::new(30.0, 1.0, 30.0), 0.0, &t54())
            .expect("stations on the map");
        assert!(height < -1.5, "a pit wider than the gear must be entered, got {height}");
    }

    #[test]
    fn a_nose_over_the_crest_keeps_riding_the_plateau() {
        // Plateau at 6 m ending at z = 30, flat ground beyond. Hull origin still on the plateau,
        // front stations hanging past the edge: the support line must hold the plateau height.
        let map = map_from(|_, z| if z < 30.0 { 6.0 } else { 0.0 });
        let height = support_height(&map, Vec3::new(30.0, 6.0, 29.0), 0.0, &t54())
            .expect("stations on the map");
        // Bilinear sampling softens the edge into a short ramp, so the ride line eases a hand
        // below the plateau as the front wheel crosses — but it must NOT dive toward the floor
        // the way the old centre sample would once the origin reached the edge.
        assert!(height > 5.5, "front overhang must not dive early, got {height}");
    }

    #[test]
    fn past_the_last_support_the_ride_line_finally_drops() {
        // Hull origin pushed past the crest so only the rearmost stations still touch the
        // plateau: the extrapolated support falls, handing the hull to the ballistic follow.
        let map = map_from(|_, z| if z < 30.0 { 6.0 } else { 0.0 });
        let on_edge = support_height(&map, Vec3::new(30.0, 6.0, 29.0), 0.0, &t54()).unwrap();
        let past_edge = support_height(&map, Vec3::new(30.0, 6.0, 33.5), 0.0, &t54()).unwrap();
        assert!(past_edge < on_edge - 1.0, "support must drop past the crest: {past_edge}");
    }

    #[test]
    fn flat_ground_matches_the_centre_sample() {
        let map = map_from(|_, _| 2.5);
        let fallback = ContactFootprint::from_hitbox(&HitboxProfile::for_vehicle(
            VehicleKind::PrototypeMedium,
        ));
        for footprint in [t54(), fallback] {
            let height =
                support_height(&map, Vec3::new(30.0, 2.5, 30.0), 0.7, &footprint).unwrap();
            assert!((height - 2.5).abs() < 1.0e-4);
        }
    }

    #[test]
    fn a_uniform_slope_reads_the_ground_under_the_origin() {
        // On a plane the convex hull is the plane itself: no bridging artefact, the support at
        // the origin equals the terrain there (the pre-envelope behavior on smooth ground).
        let map = map_from(|_, z| z * 0.2);
        let height = support_height(&map, Vec3::new(30.0, 6.0, 30.0), 0.0, &t54()).unwrap();
        let ground = map.sample_height(30.0, 30.0).unwrap();
        assert!((height - ground).abs() < 0.02, "slope support {height} vs ground {ground}");
    }
}
