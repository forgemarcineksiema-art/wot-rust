//! Unit meshes for animatable road wheels, idlers, and drive sprockets.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{
    Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, ProfilePoint, RevolveSpec,
    SmoothingGroup,
};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);

/// One road wheel, centred at the origin with its axle along X. Every family shares the steel
/// rim ring under the tyre and the proud hub cap; the FACE between them is what tells the
/// vehicles apart, and each is its own construction rather than one mesh with a parameter.
pub fn road_wheel_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    match kin.wheel_face {
        game_core::WheelFace::Openwork => openwork_wheel(kin),
        game_core::WheelFace::SteelDish => dished_wheel(kin, false),
        game_core::WheelFace::RubberDish => dished_wheel(kin, true),
        game_core::WheelFace::SpiderWeb => spider_web_wheel(kin),
    }
}

/// The Soviet post-war **"spider-web"** road wheel: a stamped disc lightened by TWO concentric
/// rings of holes — twelve small ones around the hub collar, twelve large ones further out —
/// with the radial ribs between them carrying the load out to the rim.
///
/// This is not the openwork casting, and the difference reads at any range. An openwork wheel is
/// one open void crossed by a few heavy arms, so you see the track and the hull straight through
/// it. A spider-web wheel is a metal frame: two solid bands and twelve narrow webs, so what you
/// see through are twenty-four separate punched holes.
///
/// The T-54 through T-54B rolled on this wheel (part-construction dossier, S1b). The five-arm
/// "starfish" the model assumed is a later/rebuild wheel — and the ⌀830 mm starfish belongs to
/// the 500 mm-track era, so it is not even interchangeable.
///
/// Radii are fractions of the wheel radius: hub cap, collar band, small-hole ring, mid band,
/// large-hole ring, rim ring under the tyre. The bands are thin plate; the ribs stand proud of
/// them on both faces, which is what makes the web read as a web and not as a flat gasket.
fn spider_web_wheel(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(22);
    let r = kin.wheel_radius;
    let half_w = kin.wheel_half_width;
    let body_half = half_w * 0.92;
    // Plate thickness of the stamping: thin, so the ribs read proud of it and the holes read
    // as holes rather than as tunnels.
    let plate_half = body_half * 0.26;
    let mut builder = MeshBuilder::new()
        // Full-width steel rim ring seated under the tyre (as on the openwork face).
        .append(&steel_ring(r * 0.64, kin.tyre_seat_radius(), body_half, seg))
        // The mid band, between the two rings of holes.
        .append(&steel_ring(r * 0.44, r * 0.52, plate_half, seg))
        // The hub collar the small holes are punched around (the bolt circle lands here — PR-23).
        .append(&steel_ring(r * 0.24, r * 0.34, plate_half, seg))
        // Proud central hub cap.
        .append(&wheel_disc_at(0.0, r * 0.24, half_w * 1.05, seg, MaterialRole::TrackMetal))
        .append(&road_wheel_tyres(r, half_w, kin.tyre_gap_half(), seg));
    // The webs: narrow radial ribs bridging hub collar → mid band → rim, buried at both ends.
    // They are HALF the width of an openwork arm because they stiffen a stamping rather than
    // carry the rim across a void, and their count is the disc's identity (twelve on the T-54).
    let ribs = kin.wheel_spokes.max(3);
    for i in 0..ribs {
        let angle = (i as f32 / ribs as f32) * std::f32::consts::TAU;
        builder = builder.append(&stiffening_rib(angle, r * 0.26, r * 0.68, r, plate_half * 2.1));
    }
    builder.build()
}

/// One radial web of a spider-web disc: a narrow raised bar standing proud of the stamping,
/// buried at both ends in the bands it bridges.
fn stiffening_rib(
    angle: f32,
    inner_r: f32,
    outer_r: f32,
    wheel_r: f32,
    half_width: f32,
) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    // Half the width of an openwork arm: this is a stiffening web between punched holes, not a
    // load-bearing arm spanning an open face.
    let (w_in, w_out) = (wheel_r * 0.035, wheel_r * 0.060);
    let section = vec![
        radial * inner_r - tangent * w_in,
        radial * inner_r + tangent * w_in,
        radial * outer_r + tangent * w_out,
        radial * outer_r - tangent * w_out,
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
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

/// The openwork Soviet family face: spokes/ribs over a recessed web (T-54 starfish, IS ribs,
/// T-34 spoked) — the original construction, now ONE of the family reads (audit #14).
fn openwork_wheel(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(22);
    let r = kin.wheel_radius;
    let half_w = kin.wheel_half_width;
    let body_half = half_w * 0.92;

    let mut builder = MeshBuilder::new()
        // Full-width steel rim ring seated under the tire (closed rectangular profile revolved:
        // outer wall, both side annuli, inner wall) — solid at the rim, open inboard of it. The
        // tire's side lips overlap it RADIALLY (down to 0.86 r) but sit on wider planes, so there
        // is neither a see-through gap between tire and ring nor a coplanar face to z-fight.
        .append(&steel_ring(r * 0.66, kin.tyre_seat_radius(), body_half, seg))
        // Recessed centre web the openwork reads against: thin, well inboard of the rim faces.
        .append(&wheel_disc_at(0.0, r * 0.72, body_half * 0.28, seg, MaterialRole::TrackMetal))
        // Proud central hub cap; its capped fan reads as the hub.
        .append(&wheel_disc_at(0.0, r * 0.22, half_w * 1.05, seg, MaterialRole::TrackMetal))
        // One centred rubber tire grooved down the middle — the dual-tire look without offset bands.
        .append(&road_wheel_tyres(r, half_w, kin.tyre_gap_half(), seg));
    // Radial arms bridging hub to rim, proud of the recessed web: the T-54's six-arm starfish
    // or the IS family's denser rib casting (`wheel_spokes`). Their tips are BURIED radially in
    // the rim ring and the hub, and their faces sit just INBOARD of the ring's side annuli
    // (0.94 x body width vs the ring's full body width) — an arm face flush with the ring's
    // side plane z-fights across the whole overlap band.
    let arms = kin.wheel_spokes.max(3);
    for i in 0..arms {
        let angle = (i as f32 / arms as f32) * std::f32::consts::TAU;
        builder = builder.append(&spoke_arm(angle, r * 0.16, r * 0.80, r, body_half * 0.94));
    }
    builder.build()
}

/// A bolted DISH wheel: a shallow cone face from hub to rim with a bolt ring — the German
/// late-war steel-rimmed wheel (`rubber_tire == false`: the tire band itself is steel) and the
/// Centurion's rubber-tired dish (`rubber_tire == true`). No openwork: the dish IS the face.
fn dished_wheel(kin: &RunningGearKinematics, rubber_tire: bool) -> GeometryMesh {
    let seg = kin.segments_for(22);
    let r = kin.wheel_radius;
    let half_w = kin.wheel_half_width;
    let body_half = half_w * 0.92;

    let mut builder = MeshBuilder::new()
        // Closed conical dish with separate front, rim and rear bands. Keeping the profile corners
        // split prevents the shallow face normal from smoothing into the radial rim normal and
        // turning the whole plate into a dark hemisphere under directional light.
        .append(&dish_shell(r, body_half, seg))
        // Proud hub cap.
        .append(&wheel_disc_at(0.0, r * 0.20, half_w * 1.05, seg, MaterialRole::TrackMetal));
    // The bolt ring on both faces: the dish read is the bolts.
    let bolts = 8;
    for i in 0..bolts {
        let angle = (i as f32 / bolts as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        for side in [-1.0_f32, 1.0] {
            builder = builder.append(
                &MeshBuilder::new()
                    .extrude(
                        Vec3::new(side * body_half * 1.02, sin * r * 0.52, cos * r * 0.52),
                        ExtrudeSpec {
                            section: vec![
                                Vec2::new(-0.018, -0.018),
                                Vec2::new(0.018, -0.018),
                                Vec2::new(0.018, 0.018),
                                Vec2::new(-0.018, 0.018),
                            ],
                            axis: Axis::X,
                            half_depth: 0.014,
                            material: MaterialRole::TrackMetal,
                            smoothing: SG_HARD,
                        },
                    )
                    .build(),
            );
        }
    }
    if rubber_tire {
        builder = builder.append(&road_wheel_tyres(r, half_w, kin.tyre_gap_half(), seg));
    } else {
        // Steel tire band: same silhouette as the rubber, cut in steel (no groove).
        builder = builder.append(
            &MeshBuilder::new()
                .revolve(RevolveSpec {
                    profile: vec![
                        ProfilePoint::new(r * 0.86, -half_w),
                        ProfilePoint::new(r, -half_w * 0.75),
                        ProfilePoint::new(r, half_w * 0.75),
                        ProfilePoint::new(r * 0.86, half_w),
                        // Close the ring on its INNER circumference. Without this wall the
                        // steel band is a hollow open tube; an oblique camera can see straight
                        // through it behind the recessed dish face.
                        ProfilePoint::new(r * 0.86, -half_w),
                    ],
                    axis: Axis::X,
                    segments: seg,
                    material: MaterialRole::TrackMetal,
                    smoothing: SG_WHEEL,
                })
                .build(),
        );
    }
    builder.build()
}

fn dish_shell(r: f32, body_half: f32, segments: usize) -> GeometryMesh {
    let band = |profile| {
        MeshBuilder::new()
            .revolve(RevolveSpec {
                profile,
                axis: Axis::X,
                segments,
                material: MaterialRole::TrackMetal,
                smoothing: SG_WHEEL,
            })
            .build()
    };
    MeshBuilder::new()
        .append(&band(vec![
            ProfilePoint::new(r * 0.16, body_half * 1.04),
            ProfilePoint::new(r * 0.88, body_half),
        ]))
        .append(&band(vec![
            ProfilePoint::new(r * 0.88, body_half),
            ProfilePoint::new(r * 0.88, -body_half),
        ]))
        .append(&band(vec![
            ProfilePoint::new(r * 0.88, -body_half),
            ProfilePoint::new(r * 0.16, -body_half * 1.04),
        ]))
        .build()
}

/// One return roller: a small rubber-rimmed carrier wheel for the top run (IS family), centred
/// at the origin with its axle along X — a compact steel hub disc under a rubber band.
pub fn return_roller_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    let seg = kin.segments_for(16);
    let r = kin.roller_radius.max(0.05);
    let half_w = kin.wheel_half_width * 0.55;
    MeshBuilder::new()
        .append(&wheel_disc_at(0.0, r * 0.72, half_w * 0.9, seg, MaterialRole::TrackMetal))
        .append(&wheel_disc_at(0.0, r * 0.30, half_w * 1.1, seg, MaterialRole::TrackMetal))
        .append(&rubber_band(r, half_w, seg))
        .build()
}

/// The roller's plain rubber band (no groove — carrier rollers run a flat tire).
fn rubber_band(r: f32, half_w: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r * 0.70, -half_w),
                ProfilePoint::new(r, -half_w * 0.8),
                ProfilePoint::new(r, half_w * 0.8),
                ProfilePoint::new(r * 0.70, half_w),
                ProfilePoint::new(r * 0.70, -half_w),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// A full-width steel ring (closed rectangular profile revolved about the axle): the wheel rim.
fn steel_ring(r_in: f32, r_out: f32, half_width: f32, segments: usize) -> GeometryMesh {
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r_in, -half_width),
                ProfilePoint::new(r_out, -half_width),
                ProfilePoint::new(r_out, half_width),
                ProfilePoint::new(r_in, half_width),
                ProfilePoint::new(r_in, -half_width),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::TrackMetal,
            smoothing: SG_WHEEL,
        })
        .build()
}

/// One radial starfish arm: a tapered prism from the hub seat out to the rim, spanning the full
/// body width so it stands proud of the recessed web behind it.
fn spoke_arm(
    angle: f32,
    inner_r: f32,
    outer_r: f32,
    wheel_r: f32,
    half_width: f32,
) -> GeometryMesh {
    let (sin, cos) = angle.sin_cos();
    let radial = Vec2::new(sin, cos);
    let tangent = Vec2::new(cos, -sin);
    let (w_in, w_out) = (wheel_r * 0.10, wheel_r * 0.16);
    let section = vec![
        radial * inner_r - tangent * w_in,
        radial * inner_r + tangent * w_in,
        radial * outer_r + tangent * w_out,
        radial * outer_r - tangent * w_out,
    ];
    MeshBuilder::new()
        .extrude(
            Vec3::ZERO,
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

/// A single centred rubber tire whose tread dips to a groove in the middle, giving the T-54 dual-tire
/// read as *one* concentric piece. Built as a closed annular surface of revolution so it rings the rim
/// without a disc cap that would cover the steel face. Its side lips run inward to 0.86 r —
/// radially UNDER the steel ring's outer wall (0.895 r) on a wider plane — so no annular window
/// opens between tire and ring (an open ring flickers as the spokes sweep behind it).
/// The rubber on a road wheel.
///
/// A Soviet twin-disc wheel is TWO tyres with an axial gap between them, and that gap is not
/// decoration: it is the slot the track's guide horn rides in. The generator used to draw one
/// body with a V-groove pressed down the middle and the code said so out loud — "the shoes' guide
/// horns ride INSIDE it". A groove is not a gap. From any angle where you can see between the
/// discs you saw solid rubber, and the horn that is supposed to be swallowed by the slot was
/// riding in a dent.
///
/// Wheels whose face is one disc (the German steel-rimmed, the Centurion's) answer
/// `gap_half == 0` and get the single band they really have.
fn road_wheel_tyres(r: f32, half_w: f32, gap_half: f32, segments: usize) -> GeometryMesh {
    if gap_half <= 0.0 {
        return single_tyre(r, -half_w, half_w, segments);
    }
    MeshBuilder::new()
        .append(&single_tyre(r, -half_w, -gap_half, segments))
        .append(&single_tyre(r, gap_half, half_w, segments))
        .build()
}

/// One rubber band between `x0` and `x1`: a bulged tread with a shoulder at each side, closed on
/// its inner face so the band reads as a tyre pressed onto the rim rather than a hollow sleeve.
fn single_tyre(r: f32, x0: f32, x1: f32, segments: usize) -> GeometryMesh {
    let span = x1 - x0;
    let shoulder = span * 0.16;
    MeshBuilder::new()
        .revolve(RevolveSpec {
            profile: vec![
                ProfilePoint::new(r * 0.86, x0),
                ProfilePoint::new(r * 0.91, x0),
                ProfilePoint::new(r, x0 + shoulder),
                ProfilePoint::new(r, x1 - shoulder),
                ProfilePoint::new(r * 0.91, x1),
                ProfilePoint::new(r * 0.86, x1),
                ProfilePoint::new(r * 0.86, x0),
            ],
            axis: Axis::X,
            segments,
            material: MaterialRole::Rubber,
            smoothing: SG_WHEEL,
        })
        .build()
}

pub(crate) fn wheel_disc_at(
    center_x: f32,
    radius: f32,
    half_width: f32,
    segments: usize,
    material: MaterialRole,
) -> GeometryMesh {
    MeshBuilder::new()
        .capped_revolve_at(
            Vec3::new(center_x, 0.0, 0.0),
            RevolveSpec {
                profile: vec![
                    ProfilePoint::new(radius, -half_width),
                    ProfilePoint::new(radius, half_width),
                ],
                axis: Axis::X,
                segments,
                material,
                smoothing: SG_WHEEL,
            },
        )
        .build()
}
