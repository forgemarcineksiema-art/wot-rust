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
    let r = kin.idler_radius();
    let half_w = kin.wheel_half_width;
    MeshBuilder::new()
        // An OPEN wheel, not a drum (2026-08-12 review: the idler read as a solid barrel with
        // one hole). The reference idler is a cast wheel with an open web: an outer ring under
        // the rim, a hub, and spokes with DAYLIGHT between them. At the distance tier the
        // openwork cannot be resolved, so the far mesh keeps the solid dished web — same
        // silhouette, coarser construction, exactly the tier contract.
        .append(&if kin.detail == crate::GearDetail::Near {
            MeshBuilder::new()
                .append(&steel_rim(0.0, r * 0.58, r * 0.86, half_w * 0.30, seg))
                .append(&idler_spokes(r * 0.24, r * 0.62, half_w * 0.30, 6))
                .build()
        } else {
            wheel_disc_at(0.0, r * 0.86, half_w * 0.42, seg, MaterialRole::TrackMetal)
        })
        .append(&steel_rim(0.0, r * 0.62, r * 0.88, half_w, seg))
        .append(&tread_band(0.0, r, half_w * 0.9, seg))
        .append(&wheel_disc_at(0.0, r * 0.28, half_w * 1.12, seg, MaterialRole::TrackMetal))
        .build()
}

/// The idler's eccentric tension crank — its own part ([`crate::GearPart::IdlerCrank`]), placed
/// at the idler axle UNROTATED: a crank spans the hull bearing and the axle, and until K12
/// (2026-09-06) it lived inside the idler's mesh and turned with the wheel.
pub fn idler_crank_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    tension_crank(kin, kin.idler_radius())
}

/// The left-hand crank: mirrored geometry, winding re-reversed, because the arm reaches
/// inboard and pivots toward the hull on both sides.
pub fn idler_crank_unit_mesh_left(kin: &RunningGearKinematics) -> GeometryMesh {
    crate::running_gear_arms::mirror_x(&idler_crank_unit_mesh(kin))
}

/// The idler's cast spokes: closed bars from the hub boss out to the web ring, with open air
/// between them — the daylight that makes the wheel a wheel. Each spoke is its own closed
/// prism (the same separate-solids construction the swing arm uses), so the mesh stays
/// manifold under the gear quality audit.
fn idler_spokes(r_in: f32, r_out: f32, half_depth: f32, count: usize) -> GeometryMesh {
    let mut builder = MeshBuilder::new();
    for k in 0..count {
        let theta = std::f32::consts::TAU * (k as f32 + 0.5) / count as f32;
        let dir = Vec2::new(theta.cos(), theta.sin());
        let across = Vec2::new(-dir.y, dir.x) * 0.040;
        builder = builder.append(
            &MeshBuilder::new()
                .extrude(
                    Vec3::ZERO,
                    ExtrudeSpec {
                        section: vec![
                            dir * r_in - across,
                            dir * r_in + across,
                            dir * r_out + across,
                            dir * r_out - across,
                        ],
                        axis: Axis::X,
                        half_depth,
                        material: MaterialRole::TrackMetal,
                        smoothing: SG_HARD,
                    },
                )
                .build(),
        );
    }
    builder.build()
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
///
/// The bearing sits TOWARD THE HULL from the axle — aft of a front idler, ahead of a rear one —
/// 35° off straight down, so the arm reads as a crank and not as a stub; until K12 it pointed
/// straight down and carried its worm housing on the axle instead of at the bearing.
fn tension_crank(kin: &RunningGearKinematics, r: f32) -> GeometryMesh {
    let arm_x = -kin.wheel_half_width * 1.35;
    let reach = r * 0.55;
    // The extrude section is (z, y): toward the hull's middle is -z for a front idler (rear
    // drive) and +z for a rear one (front drive).
    let toward_hull = if kin.drive_front { 1.0 } else { -1.0 };
    let (sin, cos) = 35.0_f32.to_radians().sin_cos();
    let dir = Vec2::new(toward_hull * sin, -cos);
    let perp = Vec2::new(-dir.y, dir.x);
    let root = dir * 0.055;
    let end = dir * reach;
    let mut builder = MeshBuilder::new()
        // The arm, from the axle boss to the bearing.
        .append(
            &MeshBuilder::new()
                .extrude(
                    Vec3::new(arm_x, 0.0, 0.0),
                    ExtrudeSpec {
                        section: vec![
                            root - perp * 0.045,
                            root + perp * 0.045,
                            end + perp * 0.030,
                            end - perp * 0.030,
                        ],
                        axis: Axis::X,
                        half_depth: 0.030,
                        material: MaterialRole::TrackMetal,
                        smoothing: SG_HARD,
                    },
                )
                .build(),
        );
    // The axle boss and the worm housing at the bearing end: close-range reads, inboard of the
    // wheel in the hull's shadow, so they go with the rest of the surface detail at range.
    if kin.detail == crate::GearDetail::Near {
        builder = builder
            .append(&wheel_disc_at(arm_x, 0.055, 0.032, 10, MaterialRole::TrackMetal))
            .append(
                &MeshBuilder::new()
                    .capped_revolve_at(
                        Vec3::new(arm_x, end.y, end.x),
                        RevolveSpec {
                            profile: vec![
                                ProfilePoint::new(0.070, -0.040),
                                ProfilePoint::new(0.070, 0.040),
                            ],
                            axis: Axis::X,
                            segments: 10,
                            material: MaterialRole::TrackMetal,
                            smoothing: SG_WHEEL,
                        },
                    )
                    .build(),
            );
    }
    builder.build()
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
    let r = kin.sprocket_radius();
    let half_w = kin.wheel_half_width;
    let wrap_r = crate::running_gear_belt::wrap_radius_of(kin.sprocket_radius());
    // PIN ENGAGEMENT (K9, 2026-09-06) for the OMSh wearers. The shoe is a frame with a window
    // beside its horn; the tooth passes THROUGH that window to the dossier's tip — the pitch
    // circle + 55 mm (⌀682 over the tips on the T-54) — and bears on the hinge-eye barrel
    // beside it. So the tooth is a slender pin-engagement tooth centred where the barrels end
    // (30 mm inboard of the belt edge), rooted in the dossier's ring: an annulus 120 mm DEEP
    // (ID 442) bolted to the disc's flange, its bolts proud of it. Everyone else keeps the
    // capped tooth under the shoe plate — a plate shoe has no window to pass through.
    let pin_engagement = kin.shoe == game_core::ShoePattern::Omsh;
    let tooth_half = if pin_engagement { 0.020 } else { 0.028 };
    let ring_x = if pin_engagement {
        kin.band_half_width - 0.030 - tooth_half
    } else {
        (kin.band_half_width - tooth_half).max(0.02)
    };
    // Out to the hinge-eye barrel and no further: engagement, without cutting the shoe plate.
    let tooth_outer_r =
        if pin_engagement { wrap_r + 0.055 } else { wrap_r - kin.hinge_eye_offset() * 0.90 };
    let tooth_phase = sprocket_tooth_phase(kin);
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
    // The German line's sprocket (the Kgs family) is a SPOKED wheel: eight spokes from a
    // six-bolt hub to the carrier rings, daylight between them — the STT 1944 side view
    // (K22-2, 2026-09-06). At the far tier it keeps the disc, like the idler. The Soviet and
    // British sprockets stay the dished disc their references show.
    let spoked = kin.shoe == game_core::ShoePattern::Kgs && kin.detail == crate::GearDetail::Near;
    let mut builder = if spoked {
        let mut hub = MeshBuilder::new()
            .append(&wheel_disc_at(0.0, r * 0.26, ring_x, seg, MaterialRole::TrackMetal))
            .append(&idler_spokes(r * 0.24, r * 0.70, ring_x * 0.92, 8));
        for i in 0..6 {
            let angle = (i as f32 / 6.0) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            for side in [-1.0_f32, 1.0] {
                hub = hub.append(&ring_bolt(Vec3::new(
                    side * ring_x,
                    sin * r * 0.16,
                    cos * r * 0.16,
                )));
            }
        }
        hub
    } else if pin_engagement {
        // The disc's flange reaches under the ring's bolt circle, its face meeting the ring's
        // inner face — a ring is bolted to a flange, not hung beside one.
        MeshBuilder::new()
            .append(&wheel_disc_at(
                0.0,
                wrap_r - 0.036,
                ring_x - 0.012,
                seg,
                MaterialRole::TrackMetal,
            ))
            .append(&wheel_disc_at(0.0, r * 0.26, half_w * 1.15, seg, MaterialRole::TrackMetal))
    } else {
        MeshBuilder::new()
            .append(&wheel_disc_at(0.0, r * 0.70, ring_x, seg, MaterialRole::TrackMetal))
            .append(&wheel_disc_at(0.0, r * 0.26, half_w * 1.15, seg, MaterialRole::TrackMetal))
    };
    let (ring_in, ring_out, ring_half) = if pin_engagement {
        (wrap_r - 0.065, wrap_r - 0.026, 0.012)
    } else {
        (r * 0.68, r * 0.84, 0.022)
    };
    let (root_half, tip_half) = if pin_engagement { (0.020, 0.010) } else { (0.048, 0.022) };
    let tooth_root_r = if pin_engagement { ring_out } else { r * 0.72 };
    for side in [-1.0_f32, 1.0] {
        let center_x = side * ring_x;
        // The carrier ring: an annulus the teeth root into, not a coin.
        builder = builder.append(&steel_rim(center_x, ring_in, ring_out, ring_half, seg));
        for i in 0..teeth {
            let angle = tooth_phase + (i as f32 / teeth as f32) * std::f32::consts::TAU;
            builder = builder.append(&sprocket_tooth(
                center_x,
                angle,
                tooth_root_r,
                tooth_outer_r,
                tooth_half,
                root_half,
                tip_half,
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
        // On a pin-engagement ring the bolt circle sits on the annulus, between two teeth,
        // and the head stands PROUD of the ring's outer face — a fastener shows a face outside
        // what it fastens (K9's rule), where before all forty sat inside the ring's metal.
        let (bolt_r, bolt_x) = if pin_engagement {
            (wrap_r - 0.046, center_x + side * (ring_half + 0.002))
        } else {
            (r * 0.76, center_x + side * 0.008)
        };
        for i in 0..(if kin.detail == crate::GearDetail::Near { RING_BOLTS } else { 0 }) {
            let angle = tooth_phase
                + ((i as f32 + 0.5) / RING_BOLTS as f32)
                    * std::f32::consts::TAU
                    * if pin_engagement { 1.0 } else { 0.0 }
                + (i as f32 / RING_BOLTS as f32)
                    * std::f32::consts::TAU
                    * if pin_engagement { 0.0 } else { 1.0 };
            let (sin, cos) = angle.sin_cos();
            builder = builder.append(&ring_bolt(Vec3::new(bolt_x, sin * bolt_r, cos * bolt_r)));
        }
    }
    builder.build()
}

/// Where tooth 0 stands so that the teeth meet the shoes' windows: the angle (about the
/// sprocket's axle, in the unit mesh's frame — `atan2(z, y)`, the same frame `sprocket_tooth`
/// draws in) of the link centred nearest the wrap's outermost point at phase 0, reduced to one
/// tooth pitch. Links advance along the belt by the phase and the sprocket turns by phase over
/// the wrap radius, so the relation is fixed once; the LEFT sprocket is the same mesh turned
/// through Y, which mirrors the angle, and its placement adds twice this phase to undo it.
/// A plate shoe has no window to meet: everyone but the OMSh wearers returns 0 (byte-exact).
pub fn sprocket_tooth_phase(kin: &RunningGearKinematics) -> f32 {
    if kin.shoe != game_core::ShoePattern::Omsh {
        return 0.0;
    }
    let path = crate::running_gear_belt::BeltPath::new(kin);
    let (length, count) = (path.length(), kin.link_count().max(1));
    let wrap_r = crate::running_gear_belt::wrap_radius_of(kin.sprocket_radius());
    let pitch = (kin.belt_length() / count as f32).max(0.05);
    let teeth = ((std::f32::consts::TAU * wrap_r) / pitch).round().max(8.0) as f32;
    let tooth_pitch = std::f32::consts::TAU / teeth;
    let (cz, cy) = (if kin.drive_front { kin.end_front_cz } else { -kin.end_cz }, kin.end_cy);
    // The wrap's outermost point: astern of a rear sprocket, ahead of a front one.
    let far = if kin.drive_front { cz + wrap_r } else { cz - wrap_r };
    let mut best: Option<(f32, f32)> = None;
    for i in 0..count {
        let sample = path.sample((i as f32 / count as f32) * length);
        let (dy, dz) = (sample.y - cy, sample.z - cz);
        let on_wrap = (dy.hypot(dz) - wrap_r).abs() < 0.02;
        let d = (sample.z - far).abs();
        if on_wrap && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, dz.atan2(dy)));
        }
    }
    best.map(|(_, a)| a.rem_euclid(tooth_pitch)).unwrap_or(0.0)
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

/// One tooth: a trapezoid from the ring's outer edge (`inner_r`, `root_half` wide either side)
/// to the tip (`outer_r`, `tip_half`), `half_width` deep along the axle.
fn sprocket_tooth(
    center_x: f32,
    angle: f32,
    inner_r: f32,
    outer_r: f32,
    half_width: f32,
    root_half: f32,
    tip_half: f32,
) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let section = vec![
        radial * inner_r - tangent * root_half,
        radial * inner_r + tangent * root_half,
        radial * outer_r + tangent * tip_half,
        radial * outer_r - tangent * tip_half,
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
