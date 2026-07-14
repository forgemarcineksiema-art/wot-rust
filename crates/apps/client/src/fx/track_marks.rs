//! The tracks write history (Inna Liga D5): every rolling track presses a rut into the soil —
//! a segment pair every ~1.5 m of accumulated track travel, conformal to the heightmap, fading
//! back into the field over ~40 s. The pool is budgeted like the craters (a long column
//! recycles the oldest rut, never grows), rain squalls press darker ruts into the softened
//! ground, and a broken track stops writing — the drivetrain's truth reaches the soil.

use glam::Vec3;
use renderer_api::FxVertex;
use terrain::HeightMap;

use super::decals::{Plate, premul, push_stamp};

/// How many rut segments the battle holds at once; past the budget the oldest is recycled.
pub(crate) const MAX_TRACK_MARKS: usize = 256;
/// Accumulated track travel between stamped segments — the writer's pen lift.
pub(crate) const TRACK_MARK_SPACING_M: f32 = 1.5;
/// A rut's full lifetime, and the tail of it spent fading back into the field.
const LIFETIME_S: f32 = 40.0;
const FADE_S: f32 = 12.0;
/// Lift off the ground so the rut hugs the terrain without z-fighting its triangles; the
/// craters sit higher (0.05) and correctly composite over fresh ruts.
const GROUND_LIFT_M: f32 = 0.03;
/// Half-width of one track's rut.
const HALF_WIDTH_M: f32 = 0.24;
/// Dry earth turned by a track, and the darker tone rain-softened ground takes.
const DRY_TONE: [f32; 3] = [0.075, 0.062, 0.045];
const WET_TONE: [f32; 3] = [0.038, 0.032, 0.024];
const DRY_ALPHA: f32 = 0.4;
const WET_ALPHA: f32 = 0.6;

#[derive(Debug, Default)]
pub(crate) struct TrackMarks {
    marks: Vec<TrackMark>,
}

#[derive(Debug, Clone, Copy)]
struct TrackMark {
    center: Vec3,
    /// Along the travel direction, half the segment length baked in.
    u: Vec3,
    /// Across the rut, half-width baked in.
    v: Vec3,
    wet: bool,
    age_s: f32,
}

impl TrackMarks {
    /// Press one rut segment between two ground points the track just covered. Both ends snap
    /// to the sampled terrain (the hull's translation rides suspension and slopes; the rut must
    /// not float with it), and the quad lies in the plane of the local slope.
    pub fn record_segment(&mut self, from: Vec3, to: Vec3, wet: bool, heightmap: &HeightMap) {
        let length = Vec3::new(to.x - from.x, 0.0, to.z - from.z).length();
        if length < 1.0e-3 {
            return;
        }
        let from_y = heightmap.sample_height(from.x, from.z).unwrap_or(from.y);
        let to_y = heightmap.sample_height(to.x, to.z).unwrap_or(to.y);
        let a = Vec3::new(from.x, from_y, from.z);
        let b = Vec3::new(to.x, to_y, to.z);
        let along = (b - a).normalize_or_zero();
        let across = Vec3::Y.cross(along).normalize_or_zero();
        // The plane normal from the segment's own run and the flat across — the rut leans with
        // the slope it climbs, without re-deriving the full terrain gradient.
        let normal = along.cross(across).normalize_or(Vec3::Y);
        let mark = TrackMark {
            center: (a + b) * 0.5 + normal * GROUND_LIFT_M,
            u: along * ((b - a).length() * 0.5 + HALF_WIDTH_M * 0.5),
            v: across * HALF_WIDTH_M,
            wet,
            age_s: 0.0,
        };
        if self.marks.len() < MAX_TRACK_MARKS {
            self.marks.push(mark);
            return;
        }
        if let Some(oldest) = self.marks.iter_mut().max_by(|a, b| a.age_s.total_cmp(&b.age_s)) {
            *oldest = mark;
        }
    }

    /// Age every rut one presented frame and drop the fully faded.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1).max(0.0);
        self.marks.retain_mut(|mark| {
            mark.age_s += dt;
            mark.age_s < LIFETIME_S
        });
    }

    /// Append every live rut to this frame's FX batch. Called before the craters so a shell
    /// death composites over the ruts it churned through.
    pub fn append_quads(&self, vertices: &mut Vec<FxVertex>) {
        for mark in &self.marks {
            let opacity = ((LIFETIME_S - mark.age_s) / FADE_S).clamp(0.0, 1.0);
            if opacity <= 0.0 {
                continue;
            }
            let (tone, alpha) =
                if mark.wet { (WET_TONE, WET_ALPHA) } else { (DRY_TONE, DRY_ALPHA) };
            let plate = Plate {
                center: mark.center,
                u: mark.u.normalize_or_zero(),
                v: mark.v.normalize_or_zero(),
            };
            push_stamp(
                vertices,
                plate,
                mark.u.length(),
                mark.v.length(),
                1.6,
                premul(tone, alpha * opacity),
            );
        }
    }

    #[cfg(test)]
    pub fn live_marks(&self) -> usize {
        self.marks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rut_snaps_to_the_sampled_ground_not_the_hull_height() {
        let map = HeightMap::flat(65, 65, 1.0, 2.0).expect("valid map");
        let mut marks = TrackMarks::default();
        // The hull rides suspension half a meter over the surface; the rut must not.
        marks.record_segment(Vec3::new(10.0, 4.5, 12.0), Vec3::new(11.5, 4.5, 12.0), false, &map);
        let mut vertices = Vec::new();
        marks.append_quads(&mut vertices);
        assert_eq!(vertices.len(), 6, "one rut segment is one stamp");
        for vertex in &vertices {
            assert!(
                (vertex.position[1] - (2.0 + GROUND_LIFT_M)).abs() < 1.0e-3,
                "flat ground keeps the rut at ground + lift, got y {}",
                vertex.position[1]
            );
        }
    }

    #[test]
    fn rain_presses_a_darker_rut_than_dry_ground() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut marks = TrackMarks::default();
        marks.record_segment(Vec3::new(5.0, 0.0, 5.0), Vec3::new(6.5, 0.0, 5.0), false, &map);
        marks.record_segment(Vec3::new(5.0, 0.0, 9.0), Vec3::new(6.5, 0.0, 9.0), true, &map);
        let mut vertices = Vec::new();
        marks.append_quads(&mut vertices);
        let dry_alpha = vertices[0].color[3];
        let wet_alpha = vertices[6].color[3];
        assert!(
            wet_alpha > dry_alpha,
            "a squall's rut reads stronger: dry {dry_alpha} vs wet {wet_alpha}"
        );
    }

    #[test]
    fn the_pool_is_budgeted_and_recycles_the_oldest_rut() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut marks = TrackMarks::default();
        marks.record_segment(Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.5, 0.0, 1.0), false, &map);
        marks.tick(0.1); // the first rut is now the oldest
        for index in 0..MAX_TRACK_MARKS {
            let z = 3.0 + index as f32 * 0.1;
            marks.record_segment(Vec3::new(2.0, 0.0, z), Vec3::new(3.5, 0.0, z), false, &map);
        }
        assert_eq!(marks.live_marks(), MAX_TRACK_MARKS, "pool never exceeds the budget");
    }

    #[test]
    fn a_rut_fades_out_and_dies() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut marks = TrackMarks::default();
        marks.record_segment(Vec3::new(10.0, 0.0, 10.0), Vec3::new(11.5, 0.0, 10.0), false, &map);
        let fresh = {
            let mut vertices = Vec::new();
            marks.append_quads(&mut vertices);
            vertices[0].color[3]
        };
        for _ in 0..((LIFETIME_S - FADE_S * 0.5) * 10.0) as u32 {
            marks.tick(0.1);
        }
        let mut vertices = Vec::new();
        marks.append_quads(&mut vertices);
        let faded = vertices[0].color[3];
        assert!(faded > 0.0 && faded < fresh, "mid-fade the rut is thinner but alive");
        for _ in 0..(FADE_S * 10.0) as u32 {
            marks.tick(0.1);
        }
        assert_eq!(marks.live_marks(), 0, "a fully faded rut leaves the pool");
    }

    #[test]
    fn a_zero_length_step_writes_nothing() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut marks = TrackMarks::default();
        marks.record_segment(Vec3::new(4.0, 0.0, 4.0), Vec3::new(4.0, 0.0, 4.0), false, &map);
        assert_eq!(marks.live_marks(), 0, "a parked tank writes no rut");
    }
}
