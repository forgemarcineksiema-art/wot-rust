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
    let wheel = match kin.wheel_face {
        game_core::WheelFace::Openwork => openwork_wheel(kin),
        game_core::WheelFace::SteelDish => dished_wheel(kin, false),
        game_core::WheelFace::RubberDish => dished_wheel(kin, true),
        game_core::WheelFace::SpiderWeb => spider_web_wheel(kin),
    };
    paint_the_disc(wheel)
}

/// A ROAD WHEEL IS PAINTED WITH THE TANK. Only its tyres are black.
///
/// Every disc was built in `TrackMetal` — the same value as the track links running over it — so
/// the whole running gear read as one dark mass and the construction inside it went to waste: on
/// the T-54 that is two solid bands, twelve ribs, ten hub bolts and a dished stamping, all of it
/// invisible against a belt of the same colour. Every reference photograph of every vehicle in
/// this roster shows the opposite: discs in hull paint, rubber black, track a third value again.
///
/// Sprockets and idlers keep their steel. They are the ends the belt is dragged over, they wear
/// bright, and photographs agree with the model there.
fn paint_the_disc(mut wheel: GeometryMesh) -> GeometryMesh {
    for vertex in wheel.vertices_mut() {
        if vertex.material == MaterialRole::TrackMetal {
            vertex.material = MaterialRole::RolledArmor;
        }
    }
    wheel
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
    // The disc is DISHED. A road wheel is not a flat plate on an axle: the stamping steps out
    // from the hub toward the rim, which is what gives the wheel its stiffness and what makes
    // the bands read as a pressing rather than as a stack of washers. The dish is what the eye
    // reads first on a wheel seen from three-quarters on.
    let dish = body_half * 0.55;
    // Tessellation follows SUBTENDED SIZE, not the part. The rim and the tyres are the wheel's
    // outline and get the full ring; the inner bands sit at a third of that radius, where the
    // same chord error needs three quarters of the segments. Spending the same count everywhere
    // buys nothing at the hub and costs the bolt circle its budget.
    let inner_seg = (seg * 3 / 4).max(10);
    let mut builder = MeshBuilder::new()
        // Full-width steel rim ring seated under the tyre, at the OUTBOARD end of the dish.
        .append(&steel_ring(r * 0.64, kin.tyre_seat_radius(), body_half, seg))
        // The hub collar, deepest inboard — and the ring the two discs are bolted through.
        .append(&dished_ring(r * 0.24, r * 0.34, dish, plate_half, inner_seg));
    // THE STAMPING, PUNCHED (K11, 2026-09-06). Between the collar and the rim the disc is one
    // sheet part-way down the dish, lightened by two rings of ROUND holes — twelve small ones
    // (⌀47 mm) near the collar, twelve large ones (⌀58 mm) under the rim, the dossier's
    // "12 large + 12 small lightening holes" — each a through-opening with a round rim (the
    // roundness law sets its segments) and the pressed ribs standing between them. Until now
    // the "holes" were the gaps between straight ribs across two thin bands. The sheet is two
    // annuli that meet at 0.49 r on shared vertices with no wall between them. At the distance
    // tier the sheet keeps its plain mid band: a 50 mm hole is sub-pixel at the switch range.
    if kin.detail == crate::GearDetail::Near {
        let ribs = kin.wheel_spokes.max(3);
        // Holes sit half a rib pitch off the ribs, so every rib runs between two holes.
        let offset = std::f32::consts::PI / ribs as f32;
        builder = builder
            .append(&punched_sheet(&PunchedSheet {
                x: dish * 0.45,
                half_t: plate_half,
                r_in: r * 0.34,
                r_out: r * 0.49,
                holes: ribs,
                hole_r: r * 0.058,
                hole_ring_r: r * 0.415,
                first_angle: offset,
                wall_in: true,
                wall_out: false,
            }))
            .append(&punched_sheet(&PunchedSheet {
                x: dish * 0.45,
                half_t: plate_half,
                r_in: r * 0.49,
                r_out: r * 0.66,
                holes: ribs,
                hole_r: r * 0.072,
                hole_ring_r: r * 0.572,
                first_angle: offset,
                wall_in: false,
                wall_out: true,
            }));
    } else {
        // The mid band, part-way down the dish.
        builder =
            builder.append(&dished_ring(r * 0.44, r * 0.52, dish * 0.45, plate_half, inner_seg));
    }
    builder = builder
        // Proud central hub cap: the steel hub the discs are pressed onto.
        .append(&wheel_disc_at(
            dish * 0.6,
            r * 0.22,
            half_w * 0.75,
            inner_seg,
            MaterialRole::TrackMetal,
        ))
        .append(&road_wheel_tyres(r, half_w, kin.tyre_gap_half(), seg));
    // The webs: narrow radial ribs bridging hub collar → mid band → rim, buried at both ends.
    // They are HALF the width of an openwork arm because they stiffen a stamping rather than
    // carry the rim across a void, and their count is the disc's identity (twelve on the T-54).
    let ribs = kin.wheel_spokes.max(3);
    for i in 0..ribs {
        let angle = (i as f32 / ribs as f32) * std::f32::consts::TAU;
        builder = builder.append(&stiffening_rib(angle, r * 0.26, r * 0.68, r, plate_half * 2.1));
    }
    // THE BOLT CIRCLE. The wheel is two discs bolted together and pressed onto a steel hub —
    // ten bolts, on the collar. It is the detail that says "assembly" rather than "casting", and
    // at the switch range it is ten 30 mm heads on a wheel 60 m away, so it rides the near tier.
    if kin.detail == crate::GearDetail::Near {
        for i in 0..HUB_BOLTS {
            let angle = (i as f32 / HUB_BOLTS as f32) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            // On the collar's OUTBOARD face, standing proud of it — a bolt head sunk level
            // with the plate it fastens is not a bolt head, it is a texture nobody drew.
            builder = builder.append(&hub_bolt(Vec3::new(
                dish + plate_half + 0.010,
                sin * r * 0.29,
                cos * r * 0.29,
            )));
        }
    }
    builder.build()
}

/// Bolts holding a twin-disc road wheel together. Ten on a T-54: the discs are bolted to each
/// other and pressed onto a steel hub (dossier, "Part construction").
const HUB_BOLTS: usize = 10;

/// A flat stamped annulus on the axle plane `x`, punched with `holes` round holes.
struct PunchedSheet {
    /// Axle-plane offset of the sheet's mid-plane.
    x: f32,
    /// Half the sheet's thickness.
    half_t: f32,
    /// Inner and outer radii of the annulus.
    r_in: f32,
    r_out: f32,
    /// Holes around the ring, one per equal angular sector; the first at `first_angle`.
    holes: usize,
    hole_r: f32,
    hole_ring_r: f32,
    first_angle: f32,
    /// Whether the inner / outer arc gets a wall. A sheet that continues into another sheet at
    /// that radius shares its vertices with it and draws no wall there (a wall on both would
    /// put four triangles on one edge).
    wall_in: bool,
    wall_out: bool,
}

/// The sheet of a stamped road-wheel disc: a flat annulus punched with round holes, every hole a
/// true through-opening. Per sector the face between the hole's rim and the sector's boundary
/// (outer arc, radial cut, inner arc, radial cut) is bridged by triangles walked by angle around
/// the hole's centre — the sector is star-shaped from it while the hole clears the cuts. The
/// radial cuts are shared vertices between neighbouring sectors, the arcs are the roundness
/// law's own two chords per 30° (r(1 - cos 7.5°) is under the tolerance out to the rim), and
/// only the arcs and the hole rims carry walls. Winding: the front face (+x) runs counter-
/// clockwise seen from +x, the back face the reverse, every wall against the edge it shares.
fn punched_sheet(spec: &PunchedSheet) -> GeometryMesh {
    use game_core::roundness::round_segments;
    let PunchedSheet {
        x,
        half_t,
        r_in,
        r_out,
        holes,
        hole_r,
        hole_ring_r,
        first_angle,
        wall_in,
        wall_out,
    } = *spec;
    let rim_n = round_segments(hole_r);
    let arc_n = 2usize;
    let sector = std::f32::consts::TAU / holes as f32;
    // The wheel's angular convention: radial(a) = (sin a, cos a) in (y, z), so `a` runs
    // CLOCKWISE seen from +x. Counter-clockwise is `a` decreasing.
    let radial = |a: f32, r: f32| Vec2::new(a.sin() * r, a.cos() * r);
    let at = |p: Vec2, dx: f32| Vec3::new(x + dx, p.x, p.y);
    let material = MaterialRole::TrackMetal;
    let mut builder = MeshBuilder::new();
    for i in 0..holes {
        let theta = first_angle + i as f32 * sector;
        let a0 = theta - sector * 0.5;
        let centre = radial(theta, hole_ring_r);
        // One angle list per arc, shared with the neighbours: the cut at a1 of this sector is
        // the cut at a0 of the next, and the same expression yields the same float.
        let arc_angles: Vec<f32> =
            (0..=arc_n).map(|k| a0 + sector * (k as f32 / arc_n as f32)).collect();
        // The rim, counter-clockwise seen from +x (angle decreasing), its first point on the
        // hole's own radial line toward the hub — so the ring is symmetric about the rib line
        // and no chord happens to run straight at an arc's midpoint (a sliver the audit counts).
        let rim: Vec<Vec2> = (0..rim_n)
            .map(|k| {
                let phi = theta + std::f32::consts::PI
                    - (k as f32 / rim_n as f32) * std::f32::consts::TAU;
                centre + Vec2::new(phi.sin() * hole_r, phi.cos() * hole_r)
            })
            .collect();
        // The sector boundary, counter-clockwise: the outer arc a1 → a0, down the cut at a0,
        // the inner arc a0 → a1, up the cut at a1. The cuts carry their end points only.
        let mut boundary: Vec<Vec2> = Vec::with_capacity(2 * arc_n + 2);
        for &a in arc_angles.iter().rev() {
            boundary.push(radial(a, r_out));
        }
        for &a in arc_angles.iter() {
            boundary.push(radial(a, r_in));
        }
        // Both loops ordered by angle around the hole's centre.
        let angle_of = |p: Vec2| (p - centre).to_angle();
        let rotate_to_min = |list: &mut Vec<Vec2>| {
            let start = (0..list.len())
                .min_by(|&a, &b| angle_of(list[a]).total_cmp(&angle_of(list[b])))
                .unwrap_or(0);
            list.rotate_left(start);
        };
        let mut rim = rim;
        rotate_to_min(&mut rim);
        rotate_to_min(&mut boundary);
        let (m, b) = (rim.len(), boundary.len());
        let (mut ri, mut bi) = (0usize, 0usize);
        let next_angle = |list: &[Vec2], idx: usize| {
            if idx + 1 < list.len() { angle_of(list[idx + 1]) } else { f32::INFINITY }
        };
        let mut face = |p: Vec2, q: Vec2, s: Vec2| {
            builder.push_tri([at(p, half_t), at(q, half_t), at(s, half_t)], material, SG_WHEEL);
            builder.push_tri([at(q, -half_t), at(p, -half_t), at(s, -half_t)], material, SG_WHEEL);
        };
        // Bridge: advance whichever loop's next point comes first by angle. A rim step runs the
        // rim edge backwards against the outer point, a boundary step runs the boundary edge
        // forwards against the inner point — both counter-clockwise around the hole's centre.
        while ri + 1 < m || bi + 1 < b {
            let take_rim =
                bi + 1 >= b || (ri + 1 < m && next_angle(&rim, ri) <= next_angle(&boundary, bi));
            if take_rim {
                face(rim[ri + 1], rim[ri], boundary[bi]);
                ri += 1;
            } else {
                face(boundary[bi], boundary[bi + 1], rim[ri]);
                bi += 1;
            }
        }
        // Close the ring across the wrap.
        face(rim[0], rim[m - 1], boundary[b - 1]);
        face(boundary[b - 1], boundary[0], rim[0]);
        // The hole's wall, facing INTO the hole: its top edge runs p → q where the face's rim
        // edge runs q → p.
        for k in 0..m {
            let (p, q) = (rim[k], rim[(k + 1) % m]);
            builder.push_quad(
                [at(p, half_t), at(q, half_t), at(q, -half_t), at(p, -half_t)],
                material,
                SG_WHEEL,
            );
        }
        // The arc walls: the face runs the outer arc a1 → a0 and the inner a0 → a1, so each
        // wall's top edge runs the other way.
        if wall_out {
            for k in 0..arc_n {
                let (p, q) = (radial(arc_angles[k], r_out), radial(arc_angles[k + 1], r_out));
                builder.push_quad(
                    [at(p, half_t), at(q, half_t), at(q, -half_t), at(p, -half_t)],
                    material,
                    SG_WHEEL,
                );
            }
        }
        if wall_in {
            for k in 0..arc_n {
                let (p, q) = (radial(arc_angles[k + 1], r_in), radial(arc_angles[k], r_in));
                builder.push_quad(
                    [at(p, half_t), at(q, half_t), at(q, -half_t), at(p, -half_t)],
                    material,
                    SG_WHEEL,
                );
            }
        }
    }
    builder.build()
}

/// One hub bolt head, standing proud of the collar it fastens.
fn hub_bolt(center: Vec3) -> GeometryMesh {
    MeshBuilder::new()
        .extrude(
            center,
            ExtrudeSpec {
                section: vec![
                    Vec2::new(-0.017, -0.017),
                    Vec2::new(0.017, -0.017),
                    Vec2::new(0.017, 0.017),
                    Vec2::new(-0.017, 0.017),
                ],
                axis: Axis::X,
                half_depth: 0.011,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}

/// A band of the stamping, offset along the axle by `dish` — the step that makes a pressed disc
/// a dish instead of a washer.
fn dished_ring(r_in: f32, r_out: f32, dish: f32, half_width: f32, segments: usize) -> GeometryMesh {
    let (lo, hi) = (dish - half_width, dish + half_width);
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
