//! WHAT YOU SEE THROUGH A ROAD WHEEL.
//!
//! A road wheel face is not decoration. It is the single most-looked-at part of a tank — ten of
//! them per vehicle, at eye height, turning — and the pattern of metal and hole is how anyone who
//! knows the vehicle recognises it. Getting it wrong is not a texture nit; it is drawing a
//! different wheel.
//!
//! The T-54 obr. 1951 rolled on the stamped **spider-web** disc: two solid bands with twelve small
//! lightening holes punched around the hub collar and twelve large ones further out, the radial
//! webs between them carrying the load to the rim (part-construction dossier, `docs/vehicles/t-54.md`).
//! The model used to draw the **openwork** casting instead — one open face crossed by six heavy
//! arms — which is a different wheel entirely (and the 5-arm ⌀830 starfish it was named after
//! belongs to the 500 mm-track era, so it is not even interchangeable).
//!
//! These tests do not inspect how the generator assembles its pieces. They MEASURE the finished
//! mesh the way an eye does: fire rays down the axle at a ring of radii and ask what fraction of
//! that ring is metal. A solid band answers 1.0, a ring of holes answers well under half, and no
//! rearrangement of the generator's internals can fake either answer.

use game_core::{VehicleKind, WheelFace};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, RunningGearKinematics, road_wheel_unit_mesh};

/// Fraction of a ring at `radius` (in the wheel's face plane) that is covered by metal, measured
/// by firing rays along the axle and asking whether they hit the mesh at all.
///
/// Samples are offset off the rib centreline so the answer is not decided by whether a sample
/// angle happens to land exactly on a web.
fn ring_occupancy(mesh: &GeometryMesh, radius: f32) -> f32 {
    const SAMPLES: usize = 360;
    let vertices = mesh.vertices();
    let indices = mesh.indices();
    let mut hits = 0;
    for sample in 0..SAMPLES {
        let angle = (sample as f32 + 0.37) / SAMPLES as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let point = Vec3::new(0.0, radius * sin, radius * cos);
        if ray_along_axle_hits(vertices, indices, point) {
            hits += 1;
        }
    }
    hits as f32 / SAMPLES as f32
}

/// Does a ray parallel to the axle, passing through `point`, cross any triangle of the mesh?
/// (Triangles parallel to the axle are missed by construction — which is what we want: only
/// material actually facing the viewer counts as "covering" the ring.)
fn ray_along_axle_hits(
    vertices: &[vehicle_geometry::GeometryVertex],
    indices: &[u32],
    point: Vec3,
) -> bool {
    let origin = Vec3::new(-10.0, point.y, point.z);
    let direction = Vec3::X;
    for triangle in indices.chunks_exact(3) {
        let a = vertices[triangle[0] as usize].position;
        let b = vertices[triangle[1] as usize].position;
        let c = vertices[triangle[2] as usize].position;
        let edge1 = b - a;
        let edge2 = c - a;
        let h = direction.cross(edge2);
        let det = edge1.dot(h);
        if det.abs() < 1.0e-9 {
            continue;
        }
        let inv_det = 1.0 / det;
        let s = origin - a;
        let u = s.dot(h) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(edge1);
        let v = direction.dot(q) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        if edge2.dot(q) * inv_det > 0.0 {
            return true;
        }
    }
    false
}

fn wheel(kind: VehicleKind) -> (GeometryMesh, f32) {
    let kin = RunningGearKinematics::for_vehicle(kind).expect("vehicle has running gear");
    (road_wheel_unit_mesh(&kin), kin.wheel_radius)
}

/// The T-54's disc is a FRAME: solid where the bands are, open where the holes are punched.
#[test]
fn the_t54_disc_is_two_solid_bands_with_two_rings_of_holes_between_them() {
    let (mesh, r) = wheel(VehicleKind::T54_1951);

    // The hub collar and the mid band are continuous metal all the way round.
    for (name, radius) in [("hub collar", r * 0.29), ("mid band", r * 0.48)] {
        let occupancy = ring_occupancy(&mesh, radius);
        assert!(
            occupancy > 0.99,
            "the {name} is a solid ring of the stamping, but {:.0}% of it is open air",
            (1.0 - occupancy) * 100.0
        );
    }

    // Between them — and outside the mid band — the disc is punched through. Mostly hole, with
    // the webs crossing it: that ratio IS the spider-web read.
    for (name, radius) in [("small-hole ring", r * 0.39), ("large-hole ring", r * 0.58)] {
        let occupancy = ring_occupancy(&mesh, radius);
        assert!(
            occupancy < 0.55,
            "the {name} must be mostly hole — {:.0}% of it is metal, which reads as a plate",
            occupancy * 100.0
        );
        assert!(
            occupancy > 0.15,
            "the {name} must still be crossed by its webs — only {:.0}% metal reads as a void",
            occupancy * 100.0
        );
    }
}

/// Twelve holes, not six, not twenty: the count is the disc's identity and it is authored in the
/// blueprint, so a wheel that quietly grew arms has to say so here.
#[test]
fn the_t54_disc_has_the_twelve_webs_its_blueprint_declares() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("running gear");
    assert_eq!(kin.wheel_face, WheelFace::SpiderWeb, "the T-54 rolls on the stamped disc");
    assert_eq!(kin.wheel_spokes, 12, "twelve webs between twelve pairs of lightening holes");

    // Count the webs the mesh actually draws: runs of metal around the large-hole ring.
    let mesh = road_wheel_unit_mesh(&kin);
    const SAMPLES: usize = 720;
    let radius = kin.wheel_radius * 0.58;
    let mut covered = Vec::with_capacity(SAMPLES);
    for sample in 0..SAMPLES {
        let angle = (sample as f32 + 0.37) / SAMPLES as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        covered.push(ray_along_axle_hits(
            mesh.vertices(),
            mesh.indices(),
            Vec3::new(0.0, radius * sin, radius * cos),
        ));
    }
    let runs =
        (0..SAMPLES).filter(|&i| covered[i] && !covered[(i + SAMPLES - 1) % SAMPLES]).count();
    assert_eq!(
        runs, 12,
        "the large-hole ring must be crossed by exactly twelve webs, found {runs} runs of metal"
    );
}

/// The two faces are genuinely different constructions, not one mesh with a different comment.
/// The openwork wheel's recessed web is SOLID across the same radii the spider-web punches out.
#[test]
fn the_openwork_face_is_solid_where_the_spider_web_is_punched() {
    let (openwork, r) = wheel(VehicleKind::IS3);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::IS3).expect("running gear");
    assert_eq!(kin.wheel_face, WheelFace::Openwork, "the IS-3 keeps the openwork casting");

    for radius in [r * 0.39, r * 0.58] {
        let occupancy = ring_occupancy(&openwork, radius);
        assert!(
            occupancy > 0.99,
            "the openwork wheel's recessed web is continuous metal at {:.2} r — measured {:.0}% \
             (if this drops, the two faces have merged into one and the T-54 test above is \
             measuring nothing)",
            radius / r,
            occupancy * 100.0
        );
    }
}

/// The disc is the wheel's face, so it must be INSIDE the tyre: no web, band or hub may poke out
/// past the rubber, on any vehicle. A wheel whose face stands proud of its own tread would grind
/// on the track horn.
#[test]
fn no_wheel_face_stands_proud_of_its_own_tyre() {
    for kind in VehicleKind::PLAYABLE {
        let Some(kin) = RunningGearKinematics::for_vehicle(kind) else { continue };
        let mesh = road_wheel_unit_mesh(&kin);
        let widest =
            mesh.vertices().iter().map(|vertex| vertex.position.x.abs()).fold(0.0_f32, f32::max);
        assert!(
            widest <= kin.wheel_half_width * 1.10 + 1.0e-4,
            "{kind:?}: the wheel reaches {widest:.3} m off its centreline but the tyre is only \
             {:.3} m half-wide — something on the face is sticking out into the track",
            kin.wheel_half_width
        );
    }
}
