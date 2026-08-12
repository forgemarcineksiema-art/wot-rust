//! Unit meshes for the belt's end wheels — the smooth front idler and the toothed rear drive
//! sprocket. Split from `running_gear_wheels` (the road wheel) to stay within the reviewability
//! budget.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::running_gear_wheels::wheel_disc_at;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// The larger end wheel (drive sprocket / idler), centred at the origin with its axle along X.
pub fn end_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    idler_unit_mesh(kin)
}

/// The front IDLER, centred at the origin with its axle along X.
///
/// Two corrections from the dossier ("Part construction", S1b). Its tyres are **steel, not
/// rubber** — the T-54 idler is a 510 mm cast wheel with steel tyres, and drawing it in rubber
/// made the front of the track read as a second road wheel. And it carries the track TENSIONER:
/// a two-worm crank whose eccentric arm swings the axle along an arc. That crank is the part
/// that tells you which end of the tank the idler is, and it was not there at all.
pub fn idler_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(20);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    MeshBuilder::new()
        // The dished disc: a rim band and a recessed web, not one flat coin.
        .append(&wheel_disc_at(0.0, r * 0.86, half_w * 0.42, seg, MaterialRole::TrackMetal))
        .append(&steel_rim(0.0, r * 0.62, r * 0.88, half_w, seg))
        .append(&tread_band(0.0, r, half_w * 0.9, seg))
        .append(&wheel_disc_at(0.0, r * 0.28, half_w * 1.12, seg, MaterialRole::TrackMetal))
        // The tension crank: the eccentric arm the axle rides, standing inboard of the wheel.
        // It is a close-range read — inboard of the wheel, in the hull's shadow — so it goes
        // with the rest of the surface detail at the distance tier.
        .append(&if kin.detail == crate::GearDetail::Near {
            tension_crank(kin, r)
        } else {
            GeometryMesh::default()
        })
        .build()
}

/// A steel rim band: the ring the tyres are pressed onto, closed on both faces.
fn steel_rim(
    center_x: f32,
    r_in: f32,
    r_out: f32,
    half_width: f32,
    segments: usize,
) -> GeometryMesh {
    let (lo, hi) = (center_x - half_width, center_x + half_width);
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r_in, lo),
                ProfilePoint::new(r_out, lo),
                ProfilePoint::new(r_out, hi),
                ProfilePoint::new(r_in, hi),
                ProfilePoint::new(r_in, lo),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::TrackMetal,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// The idler's eccentric tension crank: the arm between the hull bearing and the wheel axle.
/// Turning it walks the axle along an arc and takes up the slack — the mechanism that makes a
/// thrown track a repair rather than a write-off.
fn tension_crank(kin: &RunningGearKinematics, r: f32) -> GeometryMesh {
    let arm_x = -kin.wheel_half_width * 1.35;
    let reach = r * 0.55;
    MeshBuilder::new()
        // The arm, reaching back and down from the axle to its bearing.
        .append(
            &MeshBuilder::new()
                .extrude(
                    Vec3::new(arm_x, 0.0, 0.0),
                    ExtrudeSpec {
                        section: vec![
                            Vec2::new(-0.045, -0.055),
                            Vec2::new(0.045, -0.055),
                            Vec2::new(0.030, -reach),
                            Vec2::new(-0.030, -reach),
                        ],
                        axis: Axis::X,
                        half_depth: 0.030,
                        material: MaterialRole::TrackMetal,
                        smoothing: SG_HARD,
                    },
                )
                .build(),
        )
        // The worm housing at the bearing end.
        .append(&wheel_disc_at(arm_x, 0.070, 0.040, 10, MaterialRole::TrackMetal))
        .build()
}

/// A rubber tire tread ring at `radius`, spanning `center_x ± half_width` along the axle, with
/// side lips running inward to 0.82·radius. The lips radially overlap the idler's steel disc
/// (0.86 r, on wider planes), so the band reads as a solid tire instead of opening an annular
/// see-through window between tread and disc. The centre groove gives the shoes' guide horns a
/// channel around the wrap, exactly like the road wheels' dual-tire groove.
fn tread_band(center_x: f32, radius: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(radius * 0.82, center_x - half_width),
                ProfilePoint::new(radius, center_x - half_width),
                ProfilePoint::new(radius, center_x - half_width * 0.34),
                ProfilePoint::new(radius * 0.90, center_x),
                ProfilePoint::new(radius, center_x + half_width * 0.34),
                ProfilePoint::new(radius, center_x + half_width),
                ProfilePoint::new(radius * 0.82, center_x + half_width),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::TrackMetal,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// The rear DRIVE SPROCKET: a steel drum carrying two removable toothed rings.
///
/// The teeth used to stop 32 mm short of the belt line — decoration passing near a track it
/// never touched, on the one wheel whose entire job is to push it. They reach it now, and they
/// reach exactly as far as the surface they bear on: the barrel of hinge eyes on the wheel side
/// of each shoe (the цевка), whose stand-off both parts read from
/// [`RunningGearKinematics::hinge_eye_offset`]. That is the real engagement — tooth on eye, not
/// tooth on horn — and stopping there is also what keeps the tooth out of the shoe plate, which
/// sits further out on the wrap.
///
/// The carrier rings are RINGS: annuli with the bolt circle that fixes them (40 bolts per wheel
/// on a T-54, the dominant visual feature of the disc). They used to be solid coins.
pub fn sprocket_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(16);
    let r = kin.end_radius;
    let half_w = kin.wheel_half_width;
    let tooth_half = 0.028;
    let ring_x = (kin.band_half_width - tooth_half).max(0.02);
    let wrap_r = crate::running_gear_belt::wrap_radius(kin);
    // Out to the hinge-eye barrel and no further: engagement, without cutting the shoe plate.
    let tooth_outer_r = wrap_r - kin.hinge_eye_offset() * 0.90;
    let pitch = (kin.belt_length() / kin.link_count().max(1) as f32).max(0.05);
    // The count is not a style choice: a tooth must meet a link, so it is the number of link
    // pitches around the wrap circle. (On the T-54 that resolves to 14 rather than the
    // documented 13, because the belt pitch and the wrap radius are both still long — see the
    // dimensional register, M9/PR-18. Faking the count here would only hide it.)
    let teeth = ((std::f32::consts::TAU * wrap_r) / pitch).round().max(8.0) as usize;

    // THE DISC REACHES ITS RINGS. It used to stop at 0.62 of the radius and at the road wheel's
    // half-width, which on the T-54 is x 0.18 and r 0.165 — while the toothed rings sit at x
    // 0.262 with an inner radius of 0.181. Nothing touched anything: 82 mm of air along the axle,
    // 16 mm across it, and forty bolts hanging in the gap between. The bounds test that covered
    // this asked how big the wheel was, not what was joined to what.
    //
    // Widened to carry them: the rings now land on the disc's rim and the bolts pass through
    // both. Same segment count, so the same triangles — a sprocket is a plate with rings bolted
    // to its edge, and it costs nothing to say so.
    let mut builder = MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.70, ring_x, seg, MaterialRole::TrackMetal))
        .append(&wheel_disc_at(0.0, r * 0.26, half_w * 1.15, seg, MaterialRole::TrackMetal));
    for side in [-1.0_f32, 1.0] {
        let center_x = side * ring_x;
        // The carrier ring: an annulus the teeth root into, not a coin.
        builder = builder.append(&steel_rim(center_x, r * 0.68, r * 0.84, 0.022, seg));
        for i in 0..teeth {
            let angle = (i as f32 / teeth as f32) * std::f32::consts::TAU;
            builder = builder.append(&sprocket_tooth(
                center_x,
                angle,
                r * 0.72,
                tooth_outer_r,
                tooth_half,
            ));
        }
        // The bolt circle that holds the removable ring on: TWENTY per ring, because the
        // documented wheel carries 40 bolts and 40 nuts across its two rings. (The first pass
        // put 42 on each ring — twice the real hardware, and 1,408 triangles a tank was paying
        // for a number nobody had looked up.)
        // Sub-pixel at the switch range, exactly like the shoe detail: a 32 mm bolt head on a
        // wheel 60 m away is not a thing anyone can see, and there are forty of them per tank.
        // ON the ring, through into the disc behind it — a fastener that holds two things
        // together has to touch both. These sat at 0.50 of the radius, inboard of the ring's own
        // inner edge (0.68) and outboard of the disc that ended at 0.62: a bolt circle fixing
        // nothing, in mid-air, which is the "dominant visual feature of the disc" the dossier
        // describes.
        for i in 0..(if kin.detail == crate::GearDetail::Near { RING_BOLTS } else { 0 }) {
            let angle = (i as f32 / RING_BOLTS as f32) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            builder = builder.append(&ring_bolt(Vec3::new(
                center_x + side * 0.008,
                sin * r * 0.76,
                cos * r * 0.76,
            )));
        }
    }
    builder.build()
}

/// Bolts per toothed ring. The T-54's drive wheel carries 40 bolts and 40 nuts across its two
/// removable rings — the dossier calls the bolt circle the dominant visual feature of the disc.
const RING_BOLTS: usize = 20;

/// One bolt on a sprocket ring's fixing circle: a square-headed fastener sunk into the disc
/// face. (Distinct from `detail::bolt_head`, which is a chamfered revolved cylinder for hull
/// panels — and which this crate cannot call, since `detail` is built on top of it.)
fn ring_bolt(center: Vec3) -> GeometryMesh {
    MeshBuilder::new()
        .extrude(
            center,
            ExtrudeSpec {
                section: vec![
                    Vec2::new(-0.016, -0.016),
                    Vec2::new(0.016, -0.016),
                    Vec2::new(0.016, 0.016),
                    Vec2::new(-0.016, 0.016),
                ],
                axis: Axis::X,
                half_depth: 0.010,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}

fn sprocket_tooth(
    center_x: f32,
    angle: f32,
    inner_r: f32,
    outer_r: f32,
    half_width: f32,
) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let section = vec![
        radial * inner_r - tangent * 0.048,
        radial * inner_r + tangent * 0.048,
        radial * outer_r + tangent * 0.022,
        radial * outer_r - tangent * 0.022,
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::new(center_x, 0.0, 0.0),
            ExtrudeSpec {
                section,
                axis: Axis::X,
                half_depth: half_width,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}
