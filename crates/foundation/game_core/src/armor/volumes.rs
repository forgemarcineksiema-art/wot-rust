//! Convex armor volumes: the real shape of a vehicle's armor as an intersection of zone-tagged
//! half-spaces. A shell segment clipped against the planes yields the entry point AND the entering
//! plane — which IS the struck plate: its true outward normal, its armor zone, and any weakspot
//! patch riding on it. No classification bands, no phantom plate: what the volume says you hit is
//! what the visible model shows.

use glam::Vec3;

use super::ArmorZone;

/// A circular weakspot region riding on one plate — the mantlet ball on the turret front, a
/// hatch on a deck. A hit landing within `radius_m` of `center` resolves as the patch's zone
/// instead of the carrier plane's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmorPatch {
    pub zone: ArmorZone,
    pub center: Vec3,
    pub radius_m: f32,
}

/// One armor plate as a half-space boundary: inside is `normal · p <= offset`. The normal is the
/// plate's true outward normal in the volume's local frame (hull or turret), slope built in.
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedPlane {
    pub normal: Vec3,
    pub offset: f32,
    pub zone: ArmorZone,
    pub patches: Vec<ArmorPatch>,
}

impl TaggedPlane {
    pub fn new(normal: Vec3, through: Vec3, zone: ArmorZone) -> Self {
        let normal = normal.normalize_or_zero();
        Self { normal, offset: normal.dot(through), zone, patches: Vec::new() }
    }

    pub fn with_patches(mut self, patches: Vec<ArmorPatch>) -> Self {
        self.patches = patches;
        self
    }

    /// The zone a hit at `point` resolves to: a patch if the hit lands inside one, else the plane.
    pub fn zone_at(&self, point: Vec3) -> ArmorZone {
        self.patches
            .iter()
            .find(|patch| point.distance(patch.center) <= patch.radius_m)
            .map_or(self.zone, |patch| patch.zone)
    }
}

/// A convex armor volume: the intersection of its planes' half-spaces.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArmorVolume {
    pub planes: Vec<TaggedPlane>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeInterval {
    pub enter_t: f32,
    pub exit_t: f32,
    pub enter_plane: usize,
    pub exit_plane: usize,
}

#[derive(Debug, Clone, Copy)]
struct ClippedSegment {
    enter_t: f32,
    exit_t: f32,
    enter_plane: Option<usize>,
    exit_plane: Option<usize>,
}

/// Entry of the segment `start -> end` into one convex volume: the parametric `t` and the index
/// of the ENTERING plane — the plate the shell strikes. Generalized slab clipping: the entry is
/// the latest crossing into a front-facing half-space, the exit the earliest crossing out; a
/// volume is hit only while entry precedes exit.
pub fn segment_volume_entry(start: Vec3, end: Vec3, volume: &ArmorVolume) -> Option<(f32, usize)> {
    segment_volume_entry_with_margin(start, end, volume, 0.0)
}

/// Exact sphere-vs-convex sweep via an outward margin on every unit-normal half-space.
pub fn segment_volume_entry_with_margin(
    start: Vec3,
    end: Vec3,
    volume: &ArmorVolume,
    margin_m: f32,
) -> Option<(f32, usize)> {
    let clipped = clip_segment(start, end, volume, margin_m)?;
    let plane = clipped.enter_plane?;
    (clipped.enter_t <= 1.0 && clipped.exit_t >= 0.0).then_some((clipped.enter_t.max(0.0), plane))
}

/// Complete segment interval through a convex armor volume, including both physical plate planes.
pub fn segment_volume_interval_with_margin(
    start: Vec3,
    end: Vec3,
    volume: &ArmorVolume,
    margin_m: f32,
) -> Option<VolumeInterval> {
    let clipped = clip_segment(start, end, volume, margin_m)?;
    let enter_plane = clipped.enter_plane?;
    let exit_plane = clipped.exit_plane?;
    (clipped.enter_t <= 1.0 && clipped.exit_t >= 0.0).then_some(VolumeInterval {
        enter_t: clipped.enter_t.max(0.0),
        exit_t: clipped.exit_t.min(1.0),
        enter_plane,
        exit_plane,
    })
}

fn clip_segment(
    start: Vec3,
    end: Vec3,
    volume: &ArmorVolume,
    margin_m: f32,
) -> Option<ClippedSegment> {
    let direction = end - start;
    let margin_m = margin_m.max(0.0);
    let mut enter = f32::NEG_INFINITY;
    let mut exit = 1.0f32;
    let mut enter_plane: Option<usize> = None;
    let mut exit_plane: Option<usize> = None;
    for (index, plane) in volume.planes.iter().enumerate() {
        let denom = plane.normal.dot(direction);
        let signed = plane.normal.dot(start) - (plane.offset + margin_m);
        if denom.abs() < 1.0e-8 {
            if signed > 0.0 {
                return None; // Parallel and outside this half-space: no intersection at all.
            }
            continue;
        }
        let t = -signed / denom;
        if denom < 0.0 {
            // Crossing INTO the half-space: a candidate entering plate. Crossings behind the
            // start (t < 0) stay candidates — they identify the plate a start-inside segment
            // most recently pierced.
            if t > enter {
                enter = t;
                enter_plane = Some(index);
            }
        } else if t < exit {
            exit = t;
            exit_plane = Some(index);
        }
        if enter > exit {
            return None;
        }
    }
    Some(ClippedSegment { enter_t: enter, exit_t: exit, enter_plane, exit_plane })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit box around the origin with distinct zones per face.
    fn unit_box() -> ArmorVolume {
        ArmorVolume {
            planes: vec![
                TaggedPlane::new(Vec3::Z, Vec3::Z, ArmorZone::UpperGlacis),
                TaggedPlane::new(-Vec3::Z, -Vec3::Z, ArmorZone::HullRear),
                TaggedPlane::new(Vec3::X, Vec3::X, ArmorZone::HullSide),
                TaggedPlane::new(-Vec3::X, -Vec3::X, ArmorZone::HullSide),
                TaggedPlane::new(Vec3::Y, Vec3::Y, ArmorZone::Roof),
                TaggedPlane::new(-Vec3::Y, -Vec3::Y, ArmorZone::LowerPlate),
            ],
        }
    }

    #[test]
    fn entry_reports_the_entering_plane_not_just_the_distance() {
        let volume = unit_box();
        let (t, plane) =
            segment_volume_entry(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -5.0), &volume)
                .expect("segment crosses the box");
        assert!((t - 0.4).abs() < 1.0e-6, "enters the +Z face at t=0.4, got {t}");
        assert_eq!(volume.planes[plane].zone, ArmorZone::UpperGlacis);

        let (_, roof) =
            segment_volume_entry(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -5.0, 0.0), &volume)
                .expect("plunging segment crosses the box");
        assert_eq!(volume.planes[roof].zone, ArmorZone::Roof);
    }

    #[test]
    fn entry_may_end_inside_while_a_through_interval_requires_an_exit() {
        let volume = unit_box();
        let start = Vec3::new(0.0, 0.0, 5.0);
        let inside = Vec3::ZERO;
        assert!(segment_volume_entry(start, inside, &volume).is_some());
        assert!(segment_volume_interval_with_margin(start, inside, &volume, 0.0).is_none());
    }

    #[test]
    fn swept_sphere_enters_early_but_recovers_the_original_plate_contact() {
        let volume = unit_box();
        let start = Vec3::new(0.0, 0.0, 5.0);
        let end = Vec3::new(0.0, 0.0, -5.0);
        let radius = 0.08;
        let (t, plane_index) =
            segment_volume_entry_with_margin(start, end, &volume, radius).expect("sphere hits");
        let plane = &volume.planes[plane_index];
        let center = start.lerp(end, t);
        let contact = center - plane.normal * radius;

        assert!(t < 0.4, "sphere center reaches the expanded volume before a ray: {t}");
        assert!((contact.z - 1.0).abs() < 1.0e-6, "contact remains on the real plate: {contact:?}");
        assert_eq!(plane.zone, ArmorZone::UpperGlacis);
    }

    #[test]
    fn a_sloped_plane_reports_its_true_normal() {
        // Replace the front with a 60° reclined glacis through the same front edge.
        let mut volume = unit_box();
        let normal = Vec3::new(0.0, 60.0f32.to_radians().sin(), 60.0f32.to_radians().cos());
        volume.planes[0] =
            TaggedPlane::new(normal, Vec3::new(0.0, -1.0, 1.0), ArmorZone::UpperGlacis);

        let (_, plane) =
            segment_volume_entry(Vec3::new(0.0, -0.2, 5.0), Vec3::new(0.0, -0.2, -5.0), &volume)
                .expect("flat shot meets the glacis");
        let hit_normal = volume.planes[plane].normal;
        let flat_angle = Vec3::Z.dot(hit_normal).acos().to_degrees();
        assert!((flat_angle - 60.0).abs() < 1.0e-3, "true angle from the plane: {flat_angle}");
    }

    #[test]
    fn misses_return_none() {
        let volume = unit_box();
        // Clean miss above the box.
        assert_eq!(
            segment_volume_entry(Vec3::new(0.0, 3.0, 5.0), Vec3::new(0.0, 3.0, -5.0), &volume),
            None
        );
        // Segment fully before the box.
        assert_eq!(
            segment_volume_entry(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 3.0), &volume),
            None
        );
        // Segment fully past the box, moving away: entry crossings exist behind the start but
        // the volume was already exited.
        assert_eq!(
            segment_volume_entry(Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, -5.0), &volume),
            None
        );
    }

    #[test]
    fn a_start_inside_the_volume_strikes_the_latest_pierced_plate_at_t_zero() {
        let volume = unit_box();
        // Point-blank: the muzzle poked 0.5 m through the front (+Z) face, firing deeper in.
        // The shell meets the pierced glacis immediately, not the far side, and never escapes.
        let (t, plane) =
            segment_volume_entry(Vec3::new(0.0, 0.0, 0.5), Vec3::new(0.0, 0.0, -5.0), &volume)
                .expect("a muzzle inside the armor still strikes it");
        assert_eq!(t, 0.0, "the strike is immediate, not extrapolated behind the muzzle");
        assert_eq!(volume.planes[plane].zone, ArmorZone::UpperGlacis);

        // From dead centre firing forward the latest-pierced plate along the line is the rear.
        let (t, plane) =
            segment_volume_entry(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), &volume).expect("hits");
        assert_eq!(t, 0.0);
        assert_eq!(volume.planes[plane].zone, ArmorZone::HullRear);
    }

    #[test]
    fn a_patch_overrides_the_zone_only_inside_its_radius() {
        let plane = TaggedPlane::new(Vec3::Z, Vec3::Z, ArmorZone::TurretFront).with_patches(vec![
            ArmorPatch {
                zone: ArmorZone::Mantlet,
                center: Vec3::new(0.0, 0.2, 1.0),
                radius_m: 0.3,
            },
        ]);
        assert_eq!(plane.zone_at(Vec3::new(0.0, 0.25, 1.0)), ArmorZone::Mantlet);
        assert_eq!(plane.zone_at(Vec3::new(0.0, 0.8, 1.0)), ArmorZone::TurretFront);
    }
}
