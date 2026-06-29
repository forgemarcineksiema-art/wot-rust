//! Animatable running gear: the moving parts of a tracked vehicle (road wheels, drive sprocket,
//! idler, and the shoe links riding the belt loop) separated from the static belt band so the
//! renderer can spin the wheels and scroll the links from a per-side track distance.
//!
//! This module is renderer-free and deterministic. It exposes three things the client composes:
//! the per-vehicle [`RunningGearKinematics`] (derived from the blueprint), the unit meshes for one
//! wheel / one shoe link, and [`running_gear_placements`] — the list of `(part, transform)` for a
//! given per-side phase. The static belt runs and end wraps stay baked in the hull
//! (`recipes::chassis_blueprint`); only the parts that visibly move live here.

use std::f32::consts::PI;

use game_core::{VehicleBlueprint, VehicleKind};
use glam::{Mat4, Vec3};

use crate::running_gear_geom::sample_belt;

/// Target spacing between shoe links along the belt loop (metres). The link count is derived from
/// the loop length so links stay evenly sized across vehicles of different track length, dense
/// enough to read as a segmented belt rather than a few boxes.
const LINK_SPACING_M: f32 = 0.22;

/// The dimensions an animator needs to place and spin a vehicle's running gear, derived once from
/// the vehicle blueprint's [`game_core::TrackShape`]. Authored in the recipe's local space (origin
/// on the ground under the hull centre, `+Z` forward, `+Y` up), the same space [`crate::SubmeshKind::Hull`]
/// is posed in, so the client composes these straight onto the hull transform.
#[derive(Debug, Clone, PartialEq)]
pub struct RunningGearKinematics {
    /// `|x|` of each track-run centre (the belt plane offset per side).
    pub center_x: f32,
    /// Y of the belt centre line (axle height).
    pub cy: f32,
    /// Z of the belt centre.
    pub cz: f32,
    /// Half the distance between the front and rear end wheels.
    pub half_run: f32,
    /// Radius the belt wraps at each end (sprocket / idler).
    pub end_radius: f32,
    /// Radius of a road wheel.
    pub wheel_radius: f32,
    /// `x` of the wheel-disc centre (between the inner and outer rubber faces).
    pub wheel_x: f32,
    /// Half-width of a wheel disc along its axle.
    pub wheel_half_width: f32,
    /// `x` of the outer belt face (where shoe links ride).
    pub link_x: f32,
    /// Half-thickness of one shoe link along its axle.
    pub link_half_width: f32,
    /// Mid-run droop for the upper belt run. Soviet five-wheel layouts with no return rollers
    /// need a visible slack curve instead of a ruler-flat top run.
    pub top_sag_m: f32,
    /// Z of each road wheel.
    pub wheel_zs: Vec<f32>,
    pub segments: usize,
}

impl RunningGearKinematics {
    /// Build the kinematics for `kind`, or `None` for vehicles still on the legacy (non-blueprint)
    /// running gear — those render their static gear unchanged.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        let track = VehicleBlueprint::for_vehicle(kind)?.track;
        let cy = (track.top_y + track.bottom_y) * 0.5;
        let cz = (track.wheel_first_z + track.wheel_last_z) * 0.5;
        let half_run = (track.wheel_last_z - track.wheel_first_z) * 0.5;
        // Most vehicles space their road wheels evenly; the T-54 carries its signature wider
        // first/second gap, so its layout is explicit.
        let wheel_zs = match kind {
            VehicleKind::T54_1951 => vec![-2.10, -0.80, 0.22, 1.18, 2.10],
            _ => match track.wheel_count {
                0 => Vec::new(),
                1 => vec![track.wheel_first_z],
                count => {
                    let step = (track.wheel_last_z - track.wheel_first_z) / (count - 1) as f32;
                    (0..count).map(|i| track.wheel_first_z + step * i as f32).collect()
                }
            },
        };
        Some(Self {
            center_x: track.center_x,
            cy,
            cz,
            half_run,
            end_radius: track.end_radius,
            wheel_radius: track.wheel_radius,
            wheel_x: (track.inner_x + track.outer_x) * 0.5,
            wheel_half_width: ((track.outer_x - track.inner_x) * 0.5).max(0.03),
            link_x: track.center_x + track.belt_half_thickness - track.belt_half_thickness * 0.5,
            link_half_width: (track.belt_half_thickness * 0.5).max(0.02),
            top_sag_m: match kind {
                VehicleKind::T54_1951 => 0.075,
                _ => 0.035,
            },
            wheel_zs,
            segments: track.segments.max(12),
        })
    }

    /// Length of the closed belt loop (two straight runs plus two end semicircles).
    pub fn belt_length(&self) -> f32 {
        4.0 * self.half_run + 2.0 * PI * self.belt_wrap_radius()
    }

    /// Vertical radius the belt wraps at each end.
    pub(crate) fn belt_wrap_radius(&self) -> f32 {
        self.end_radius.max(0.05)
    }

    /// Number of shoe links around the loop.
    pub fn link_count(&self) -> usize {
        (self.belt_length() / LINK_SPACING_M).round().max(4.0) as usize
    }

    /// Half-length of one shoe link along the belt. Links nearly fill their spacing (only a thin
    /// seam between them) so the belt reads as a continuous segmented band rather than a dashed
    /// line — a gappy belt strobes badly when it scrolls at speed.
    pub(crate) fn link_half_length(&self) -> f32 {
        (self.belt_length() / self.link_count() as f32) * 0.47
    }
}

/// Which unit mesh a placement instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearPart {
    RoadWheel,
    Idler,
    Sprocket,
    Link,
}

/// One instanced running-gear part: the unit mesh to draw and where (hull-local).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GearPlacement {
    pub part: GearPart,
    pub transform: Mat4,
}

/// Place and orient every moving part for both tracks at the given per-side phase (the track
/// distance travelled, in metres). Wheels spin by `phase / radius`; links advance `phase` along the
/// belt loop and wrap continuously over the ends. The two sides take independent phases, so a pivot
/// shows the tracks running opposite ways.
pub fn running_gear_placements(
    kin: &RunningGearKinematics,
    left_phase_m: f32,
    right_phase_m: f32,
) -> Vec<GearPlacement> {
    let mut placements = Vec::new();
    // side_sign +1 = right (+x), -1 = left (-x). Each side carries its own phase.
    for (side_sign, phase) in [(1.0_f32, right_phase_m), (-1.0_f32, left_phase_m)] {
        place_side(kin, side_sign, phase, &mut placements);
    }
    placements
}

fn place_side(
    kin: &RunningGearKinematics,
    side_sign: f32,
    phase: f32,
    out: &mut Vec<GearPlacement>,
) {
    // Road wheels: spin about the axle (X) by phase / radius.
    let wheel_spin = phase / kin.wheel_radius.max(0.05);
    for &z in &kin.wheel_zs {
        out.push(GearPlacement {
            part: GearPart::RoadWheel,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.cy, z))
                * Mat4::from_rotation_x(wheel_spin),
        });
    }
    // Drive sprocket (rear) and idler (front): the larger end wheels the belt wraps.
    let end_spin = phase / kin.end_radius.max(0.05);
    for (part, z) in
        [(GearPart::Sprocket, kin.cz - kin.half_run), (GearPart::Idler, kin.cz + kin.half_run)]
    {
        out.push(GearPlacement {
            part,
            transform: Mat4::from_translation(Vec3::new(side_sign * kin.wheel_x, kin.cy, z))
                * Mat4::from_rotation_x(end_spin),
        });
    }
    // Shoe links: evenly spaced around the loop, advanced by the phase and wrapped.
    let count = kin.link_count();
    let length = kin.belt_length();
    let mut link_phase = phase.rem_euclid(length);
    if link_phase < 1.0e-4 || link_phase > length - 1.0e-4 {
        link_phase = 0.0;
    }
    for i in 0..count {
        let mut s = (link_phase + (i as f32 / count as f32) * length).rem_euclid(length);
        if s > length - 1.0e-4 {
            s = 0.0;
        }
        let sample = sample_belt(kin, s);
        out.push(GearPlacement {
            part: GearPart::Link,
            transform: Mat4::from_translation(Vec3::new(
                side_sign * kin.link_x,
                sample.y,
                sample.z,
            )) * Mat4::from_rotation_x(sample.rot_x),
        });
    }
}
