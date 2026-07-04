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
use glam::Mat4;

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
    /// Half the distance between the front and rear road wheels.
    pub half_run: f32,
    /// Radius the belt wraps at each end (sprocket / idler).
    pub end_radius: f32,
    /// |Z| of the idler/sprocket axles — beyond `half_run` on the T-54, whose belt ramps up past
    /// the road wheels onto separate end wheels; equal to `half_run` for the stadium loop.
    pub end_cz: f32,
    /// Y of the idler/sprocket axles (raised above `cy` on the T-54).
    pub end_cy: f32,
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
    link_count: usize,
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
            // 810 mm wheels at a ~0.92 m pitch (rims ~0.11 m apart), with the T-54's signature
            // wider gap between the first and second road wheels at the front (+Z).
            VehicleKind::T54_1951 => vec![-1.95, -1.03, -0.11, 0.81, 1.95],
            _ => match track.wheel_count {
                0 => Vec::new(),
                1 => vec![track.wheel_first_z],
                count => {
                    let step = (track.wheel_last_z - track.wheel_first_z) / (count - 1) as f32;
                    (0..count).map(|i| track.wheel_first_z + step * i as f32).collect()
                }
            },
        };
        let wheel_half_width = match kind {
            // Wide road wheels to match the tank's scale — thin discs read like wheelbarrow wheels.
            VehicleKind::T54_1951 => 0.18,
            _ => ((track.outer_x - track.inner_x) * 0.5).max(0.03),
        };
        let link_half_width = match kind {
            // A wide OMSh belt: the shoe plate (1.25 × this half-width in the unit mesh) spans the
            // full 580 mm track band (1.06..1.63), covering the road wheels it rides over.
            VehicleKind::T54_1951 => 0.23,
            _ => (track.belt_half_thickness * 0.5).max(0.02),
        };
        let link_count = match kind {
            VehicleKind::T54_1951 => 90,
            _ => {
                let loop_len = 4.0 * half_run + 2.0 * PI * track.end_radius.max(0.05);
                (loop_len / LINK_SPACING_M).round().max(4.0) as usize
            }
        };
        Some(Self {
            center_x: track.center_x,
            cy,
            cz,
            half_run,
            end_radius: track.end_radius,
            end_cz: track.end_z,
            end_cy: track.end_y,
            wheel_radius: track.wheel_radius,
            wheel_x: (track.inner_x + track.outer_x) * 0.5,
            wheel_half_width,
            link_x: match kind {
                // The track shoes ride over the road wheels, centred on the wheel plane so the belt
                // wraps the wheels instead of floating as a separate ribbon outboard of them.
                VehicleKind::T54_1951 => (track.inner_x + track.outer_x) * 0.5,
                _ => track.center_x + track.belt_half_thickness - track.belt_half_thickness * 0.5,
            },
            link_half_width,
            top_sag_m: match kind {
                VehicleKind::T54_1951 => 0.050,
                _ => 0.035,
            },
            wheel_zs,
            segments: track.segments.max(12),
            link_count,
        })
    }

    /// Length of the closed belt loop (ground run, end wraps, sagging top run, and — where the
    /// end wheels sit beyond the road wheels — the tangent ramps up to them).
    pub fn belt_length(&self) -> f32 {
        crate::running_gear_belt::BeltPath::new(self).length()
    }

    /// Number of shoe links around the loop.
    pub fn link_count(&self) -> usize {
        self.link_count
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
