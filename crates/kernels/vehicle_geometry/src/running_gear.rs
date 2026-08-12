//! Animatable running gear: the moving parts of a tracked vehicle (road wheels, drive sprocket,
//! idler, and the shoe links riding the belt loop) separated from the static hull so the
//! renderer can spin the wheels and scroll the links from a per-side track distance.
//!
//! This module is renderer-free and deterministic. It exposes three things the client composes:
//! the per-vehicle [`RunningGearKinematics`] (derived from the blueprint), the unit meshes for one
//! wheel / one shoe link, and [`running_gear_placements`] — the list of `(part, transform)` for a
//! given per-side phase. The belt's overlap skin belongs to each link, so every visible running
//! gear part moves, tensions, and disappears with the track it represents.

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
    /// Radius the belt wraps at the REAR end wheel (the sprocket on a rear-drive layout).
    pub end_radius: f32,
    /// |Z| of the REAR end-wheel axle — beyond `half_run` on the T-54, whose belt ramps up past
    /// the road wheels onto separate end wheels; equal to `half_run` for the stadium loop.
    pub end_cz: f32,
    /// Y of the idler/sprocket axles (raised above `cy` on the T-54; shared by both ends).
    pub end_cy: f32,
    /// |Z| of the FRONT end-wheel axle. Equal to `end_cz` on the symmetric fleet; the T-54
    /// authors its idler a fifth of a metre further out than its sprocket (`TrackShape::
    /// end_front`), with the 90-link loop identity arbitrating the pair.
    pub end_front_cz: f32,
    /// Radius the belt wraps at the FRONT end wheel (`end_radius` unless authored).
    pub end_front_radius: f32,
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
    /// Visible suspension architecture: torsion arm, Christie crank, or paired Horstmann bogie.
    pub suspension: game_core::SuspensionKind,
    /// Z of each return roller (empty when the top run rests on the road wheels).
    pub roller_zs: Vec<f32>,
    /// Radius of one return roller.
    pub roller_radius: f32,
    /// Y of the return-roller axles: the roller top carries the belt's top run.
    pub roller_y: f32,
    /// Trailing-arm geometry, from the blueprint: how far the arm reaches back from its hull
    /// pivot to the axle, and how far the pivot stands above that axle at rest.
    pub arm_reach: f32,
    pub arm_rise: f32,
    pub segments: usize,
    /// How finely this gear is built. Every generator asks [`Self::segments_for`] rather than
    /// reading `segments` directly, so one field switches the whole running gear between the
    /// authored construction and the distant one.
    pub detail: GearDetail,
    link_count: usize,
}

/// How finely the running gear is built.
///
/// The gear is the largest single body of geometry on a vehicle — on a T-54 it is 38.6k
/// triangles across 204 instances, more than twice the whole static bake — and until this existed
/// it was drawn at full detail at every range, on every tank on the field, for the life of the
/// battle. A 7v7 spent 540k triangles on running gear that is four pixels tall at the far side of
/// the map.
///
/// `Far` is not a different mesh. It is the SAME construction with the tessellation and the
/// surface detail a viewer at that range cannot resolve taken out, so a wheel that is a
/// spider-web disc up close is still a spider-web disc at 80 m — just built out of fewer
/// triangles. Nothing about the silhouette changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GearDetail {
    /// The authored construction: what the blueprint asks for, what the studio tiles show.
    #[default]
    Near,
    /// The same parts, coarsely: half the ring segments and no sub-shoe surface detail.
    Far,
}

/// Range past which a vehicle's running gear switches to [`GearDetail::Far`].
///
/// Chosen from what a shoe link actually subtends: a 0.14 m detail box on a 1080p screen with a
/// 60 degree field of view crosses one pixel at about 55 m. Sixty gives a hand of margin, and it
/// sits well outside the range where a player is reading a tank's suspension.
pub const GEAR_DETAIL_SWITCH_M: f32 = 60.0;

/// Which detail tier a vehicle at `distance_m` from the camera draws its gear at.
pub fn gear_detail_for_distance(distance_m: f32) -> GearDetail {
    if distance_m > GEAR_DETAIL_SWITCH_M { GearDetail::Far } else { GearDetail::Near }
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
        let cy = track.axle_y();
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
        // Provisional: the belt path below reads only the loop's GEOMETRY, never the link count,
        // so the count can be settled once the kinematics exist.
        let link_count = 1;
        // Return rollers spread evenly along the middle of the wheel run, clear of the end
        // wraps; the roller TOP carries the belt's top run, so the axle sits one radius below.
        let roller_zs: Vec<f32> = (0..track.return_rollers)
            .map(|index| {
                let t = (index as f32 + 0.5) / track.return_rollers as f32;
                cz - half_run * 0.72 + (half_run * 1.44) * t
            })
            .collect();
        let mut kinematics = Self {
            center_x: track.center_x,
            cy,
            cz,
            half_run,
            end_radius: track.end_radius,
            end_cz: track.end_z,
            end_cy: track.end_y,
            end_front_cz: track.end_front.map(|(z, _)| z).unwrap_or(track.end_z),
            end_front_radius: track.end_front.map(|(_, r)| r).unwrap_or(track.end_radius),
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
            suspension: track.suspension,
            roller_zs,
            roller_radius: track.roller_radius,
            roller_y: track.top_y - track.roller_radius,
            arm_reach: track.arm_reach(),
            arm_rise: track.arm_rise(),
            segments: track.segments.max(12),
            detail: GearDetail::Near,
            link_count,
        };
        // Settle the shoe count against the REAL loop.
        //
        // The fallback used to measure a STADIUM — `4 * half_run + 2 * PI * end_radius` — while
        // `running_gear_place` spread those links over the true ramped `belt_length()`. Every
        // vehicle in this fleet carries its idler and sprocket beyond and above the road-wheel
        // line, so the stadium is short by about a quarter, and the shoes were stretched to cover
        // the difference. Measured 2026-08-08 across the five vehicles that had no authored
        // count: 283-307 mm of rendered pitch against 150-172 mm of real track — shoes
        // 1.65-2.05x too long, a belt reading as a chunky chain rather than a track.
        //
        // Authoring the documented count is still the right answer and the whole fleet now does
        // it; this is what a NEW vehicle gets before someone looks its track up, and it is now
        // wrong by rounding rather than by a factor of two.
        kinematics.link_count = match track.link_count {
            Some(authored) => authored,
            None => (kinematics.belt_length() / LINK_SPACING_M).round().max(4.0) as usize,
        };
        kinematics
    }

    /// Length of the closed belt loop (ground run, end wraps, sagging top run, and — where the
    /// end wheels sit beyond the road wheels — the tangent ramps up to them).
    pub fn belt_length(&self) -> f32 {
        crate::running_gear_belt::BeltPath::new(self).length()
    }

    /// The TAUT loop length — the belt with its slack pulled out, which is what the physical
    /// belt's length IS (link count times pitch). The draped [`Self::belt_length`] runs ~1%
    /// longer on a deep-scallop vehicle; link pitch and tooth meshing derive from THIS one.
    pub fn taut_belt_length(&self) -> f32 {
        crate::running_gear_belt::BeltPath::with_sag(self, 0.0).length()
    }

    /// The idler's axle |z|: the FRONT end wheel on a rear-drive layout, the rear one on the
    /// German front-drive line.
    pub fn idler_cz(&self) -> f32 {
        if self.drive_front { self.end_cz } else { self.end_front_cz }
    }

    /// The idler's wheel radius (same resolution as [`Self::idler_cz`]).
    pub fn idler_radius(&self) -> f32 {
        if self.drive_front { self.end_radius } else { self.end_front_radius }
    }

    /// The drive sprocket's axle |z| — the end the torque lives at.
    pub fn sprocket_cz(&self) -> f32 {
        if self.drive_front { self.end_front_cz } else { self.end_cz }
    }

    /// The drive sprocket's wheel radius (same resolution as [`Self::sprocket_cz`]).
    pub fn sprocket_radius(&self) -> f32 {
        if self.drive_front { self.end_front_radius } else { self.end_radius }
    }

    /// Number of shoe links around the loop.
    pub fn link_count(&self) -> usize {
        self.link_count
    }

    /// Half the axial gap between a twin-disc road wheel's two tyres — the slot the track's
    /// guide horn rides in, and the reason the horn exists at all.
    ///
    /// The Soviet road wheel is two discs bolted together with a gap between their rubber tyres
    /// (T-54: 185 mm tyres, ~53 mm gap in a 423 mm assembly — 12.5% of the width, dossier
    /// "Part construction"). The horn on every shoe from September 1949 stands up into that slot
    /// and keeps the belt from walking off the wheel. One dimension, two parts: the wheel cannot
    /// be honest without the gap and the link cannot be honest without the horn.
    ///
    /// German steel-dish and British rubber-dish wheels are ONE disc — their belts are guided
    /// between adjacent wheels or by a horn riding outside them — so they answer zero.
    pub fn tyre_gap_half(&self) -> f32 {
        match self.wheel_face {
            game_core::WheelFace::Openwork | game_core::WheelFace::SpiderWeb => {
                self.wheel_half_width * 0.125
            }
            game_core::WheelFace::SteelDish | game_core::WheelFace::RubberDish => 0.0,
        }
    }

    /// Radius of the steel the rubber tyres are pressed onto.
    ///
    /// Between a twin-disc wheel's two tyres this is the FLOOR the track's guide horn runs over,
    /// so it is the number that decides how tall the horn may be. The wheel generator seats its
    /// rim ring here and the link generator sizes its horn from here: the horn cannot bottom out
    /// on the wheel, by construction rather than by a tuned constant.
    pub fn tyre_seat_radius(&self) -> f32 {
        self.wheel_radius * 0.895
    }

    /// How far the hinge-eye barrel stands off the belt's centreline, on the wheel side.
    ///
    /// This is the surface a drive sprocket's teeth actually bear on — the цевка. The link
    /// generator builds the barrel here and the sprocket generator reaches its teeth to here, so
    /// "the teeth engage the track" is one number rather than two guesses. A tooth that stops
    /// short of it drives nothing; one that reaches past it cuts through the shoe plate.
    pub fn hinge_eye_offset(&self) -> f32 {
        0.061
    }

    /// This same gear, built for distance. The belt path, the wheel positions and every
    /// dimension are untouched — only how finely the parts are tessellated changes, so a
    /// far-detail tank stands in exactly the same place as a near one.
    pub fn at_detail(&self, detail: GearDetail) -> Self {
        Self { detail, ..self.clone() }
    }

    /// Ring segments for a part whose construction needs at least `floor` of them to read as
    /// round at all.
    ///
    /// Every generator used to write `kin.segments.max(floor)`, which meant the authored
    /// `segments` knob in the blueprint did nothing on any vehicle: every floor (22 on a wheel,
    /// 20 on an idler, 16 on a sprocket, 12 on an arm) already sat above the 14 the RON asked
    /// for. The knob was dead data. Now the floor is the part's own minimum, the blueprint can
    /// raise it, and the detail tier scales the result.
    pub fn segments_for(&self, floor: usize) -> usize {
        let full = self.segments.max(floor);
        match self.detail {
            GearDetail::Near => full,
            // Half the ring. The absolute floor is where a disc stops reading as a disc at all;
            // at the switch range an eight-sided wheel is indistinguishable from a round one.
            GearDetail::Far => (full / 2).max(8),
        }
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
    /// The left-hand [`GearPart::SwingArm`]: the arm is the one gear part that is neither a solid
    /// of revolution (the wheels — a half-turn about Y made those face outward) nor X-symmetric
    /// (the shoes). Its torsion boss stands proud INBOARD, and a rotation that kept the arm
    /// trailing would point that boss at the wheel — which is exactly how it shipped: the left
    /// arms drove their bosses 89 mm into the wheel discs. A true mirror needs mirrored GEOMETRY,
    /// so the left side is its own unit mesh with the winding re-reversed.
    SwingArmLeft,
}

/// One instanced running-gear part: the unit mesh to draw and where (hull-local).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GearPlacement {
    pub part: GearPart,
    pub transform: Mat4,
}
