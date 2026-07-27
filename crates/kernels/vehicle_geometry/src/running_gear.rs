//! Animatable running gear: the moving parts of a tracked vehicle (road wheels, drive sprocket,
//! idler, and the shoe links riding the belt loop) separated from the static hull so the
//! renderer can spin the wheels and scroll the links from a per-side track distance.
//!
//! This module is renderer-free and deterministic. It exposes three things the client composes:
//! the per-vehicle [`RunningGearKinematics`] (derived from the blueprint), the unit meshes for one
//! wheel / one shoe link, and [`running_gear_placements`] — the list of `(part, transform)` for a
//! given per-side phase. The belt's overlap skin belongs to each link, so every visible running
//! gear part moves, tensions, and disappears with the track it represents.

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
    /// Schachtellaufwerk: how far INBOARD every odd-indexed road wheel sits (0 = single file).
    /// A wheel-width offset reads as the Tiger's interleaved double row; a small one as the
    /// Tiger II/Panther overlapped stagger.
    pub wheel_overlap_dx: f32,
    /// Half-width of a wheel disc along its axle.
    pub wheel_half_width: f32,
    /// `x` of the outer belt face (where shoe links ride).
    pub link_x: f32,
    /// Half-thickness of one shoe link along its axle.
    pub link_half_width: f32,
    /// Half-width of the belt band itself: `(outer_x - inner_x) / 2`. The shoe plates span
    /// exactly this, so the outermost gear face sits AT the blueprint's `outer_x` — the
    /// documented "width over tracks" is honest (dimension-gate finding, W1 PR-T1.2).
    pub band_half_width: f32,
    /// `true` = the toothed drive sprocket is the FRONT end wheel (the German line, audit
    /// #16); `false` = rear drive (Soviets, T-34, Centurion).
    pub drive_front: bool,
    /// Mid-run droop for the upper belt run. Soviet five-wheel layouts with no return rollers
    /// need a visible slack curve instead of a ruler-flat top run; rollered layouts stay taut.
    pub top_sag_m: f32,
    /// Z of each road wheel.
    pub wheel_zs: Vec<f32>,
    /// Radial spokes/ribs on a road wheel's face: the T-54's six-arm starfish, the IS family's
    /// twelve-rib casting.
    pub wheel_spokes: usize,
    /// The track-shoe pattern the link unit mesh stamps (audit #14 — per family, not one
    /// generator for three nations).
    pub shoe: game_core::ShoePattern,
    /// The road-wheel face construction (audit #14 — openwork Soviet spokes vs the German
    /// bolted steel dish vs the Centurion's bolted dish under rubber).
    pub wheel_face: game_core::WheelFace,
    /// The front idler's face; defaults to the fleet's shared smooth drum.
    pub idler_face: game_core::IdlerFace,
    /// Visible suspension architecture: torsion arm, Christie crank, or paired Horstmann bogie.
    pub suspension: game_core::SuspensionKind,
    /// Z of each return roller (empty when the top run rests on the road wheels).
    pub roller_zs: Vec<f32>,
    /// Radius of one return roller.
    pub roller_radius: f32,
    /// Y of the return-roller axles: the roller top carries the belt's top run.
    pub roller_y: f32,
    pub segments: usize,
    link_count: usize,
}

impl RunningGearKinematics {
    /// Build the kinematics for `kind`, or `None` for vehicles that keep fused static gear (the
    /// test-only prototype medium). The whole animated fleet reads its blueprint's track — the
    /// legacy hand-authored table is gone with the last legacy vehicle.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        let track = VehicleBlueprint::for_vehicle(kind).map(|blueprint| blueprint.track)?;
        Some(Self::from_track(&track))
    }

    /// Build the kinematics straight from a [`TrackShape`] — the constructor the static belt
    /// band shares with [`Self::for_vehicle`], so a Studio live-override bakes its band from
    /// the SAME live track it bakes everything else from.
    pub fn from_track(track: &game_core::TrackShape) -> Self {
        let track = *track;
        let cy = (track.top_y + track.bottom_y) * 0.5;
        let cz = (track.wheel_first_z + track.wheel_last_z) * 0.5;
        let half_run = (track.wheel_last_z - track.wheel_first_z) * 0.5;
        // Wheel stations come from the blueprint (explicit for the T-54's signature wider
        // first/second gap, an even spread otherwise) — the SAME stations the physics contact
        // footprint rides on, so the wheels the player sees are the wheels the hull sits on.
        let wheel_zs = track.wheel_stations();
        // Gear identity is BLUEPRINT DATA now (the TrackShape lift): wheel/shoe widths, the
        // authored link count, top-run sag and spoke count all come from the same RON file the
        // rest of the shape does — the per-kind match tables this replaced were the last
        // hand-tuned gear constants living outside the blueprint.
        let wheel_half_width = track.wheel_half_width.max(0.03);
        let link_half_width = track.link_half_width.max(0.02);
        let link_count = track.link_count.unwrap_or_else(|| {
            let loop_len = 4.0 * half_run + 2.0 * PI * track.end_radius.max(0.05);
            (loop_len / LINK_SPACING_M).round().max(4.0) as usize
        });
        // Return rollers spread evenly along the middle of the wheel run, clear of the end
        // wraps; the roller TOP carries the belt's top run, so the axle sits one radius below.
        let roller_zs: Vec<f32> = (0..track.return_rollers)
            .map(|index| {
                let t = (index as f32 + 0.5) / track.return_rollers as f32;
                cz - half_run * 0.72 + (half_run * 1.44) * t
            })
            .collect();
        Self {
            center_x: track.center_x,
            cy,
            cz,
            half_run,
            end_radius: track.end_radius,
            end_cz: track.end_z,
            end_cy: track.end_y,
            wheel_radius: track.wheel_radius,
            wheel_x: (track.inner_x + track.outer_x) * 0.5,
            band_half_width: ((track.outer_x - track.inner_x) * 0.5).max(0.05),
            drive_front: track.drive_front,
            wheel_overlap_dx: track.overlap_inner_dx,
            wheel_half_width,
            // The track shoes ride over the road wheels, centred on the wheel plane so the
            // belt wraps the wheels instead of floating as a separate ribbon outboard of them.
            link_x: (track.inner_x + track.outer_x) * 0.5,
            link_half_width,
            top_sag_m: track.top_sag_m,
            wheel_zs,
            wheel_spokes: track.wheel_spokes.max(3),
            shoe: track.shoe_pattern,
            wheel_face: track.wheel_face,
            idler_face: track.idler_face,
            suspension: track.suspension,
            roller_zs,
            roller_radius: track.roller_radius,
            roller_y: track.top_y - track.roller_radius,
            segments: track.segments.max(12),
            link_count,
        }
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
    /// Visible suspension unit: one per axle for torsion/Christie, one per pair for Horstmann.
    SwingArm,
    /// Small top-run carrier roller (IS family); spins with the belt like everything it touches.
    ReturnRoller,
}

/// One instanced running-gear part: the unit mesh to draw and where (hull-local).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GearPlacement {
    pub part: GearPart,
    pub transform: Mat4,
}
