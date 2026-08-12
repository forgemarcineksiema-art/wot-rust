//! The inspection export must be a faithful, re-readable copy of the bake. An exporter that
//! silently drops a part, mangles a coordinate, or renumbers a face would send the
//! master-reference loop chasing deviations that only exist in the file.
//!
//! So: parse the OBJ back and hold it against the mesh it came from.

use std::collections::{BTreeSet, HashMap};

use game_core::VehicleKind;
use vehicle_forge::{authoritative_baked_vehicle, export_obj};
use vehicle_geometry::{
    GearPart, RunningGearKinematics, idler_unit_mesh, road_wheel_unit_mesh,
    running_gear_placements, sprocket_unit_mesh, swing_arm_unit_mesh, track_link_unit_mesh,
};

struct ParsedObj {
    positions: Vec<[f32; 3]>,
    normals: usize,
    faces: Vec<[usize; 3]>,
    objects: Vec<String>,
    materials: BTreeSet<String>,
    faces_per_object: HashMap<String, usize>,
}

fn parse(obj: &str) -> ParsedObj {
    let mut parsed = ParsedObj {
        positions: Vec::new(),
        normals: 0,
        faces: Vec::new(),
        objects: Vec::new(),
        materials: BTreeSet::new(),
        faces_per_object: HashMap::new(),
    };
    let mut current = String::new();
    for line in obj.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let values: Vec<f32> =
                    parts.map(|value| value.parse().expect("float coordinate")).collect();
                assert_eq!(values.len(), 3, "a vertex has three coordinates: {line}");
                parsed.positions.push([values[0], values[1], values[2]]);
            }
            Some("vn") => parsed.normals += 1,
            Some("o") => {
                current = parts.next().expect("object name").to_string();
                parsed.objects.push(current.clone());
            }
            Some("usemtl") => {
                parsed.materials.insert(parts.next().expect("material name").to_string());
            }
            Some("f") => {
                let indices: Vec<usize> = parts
                    .map(|token| {
                        // `f v//vn` — position and normal share the index in this exporter.
                        let mut halves = token.split("//");
                        let position: usize =
                            halves.next().expect("position index").parse().expect("index");
                        let normal: usize =
                            halves.next().expect("normal index").parse().expect("index");
                        assert_eq!(position, normal, "position and normal indices must agree");
                        position
                    })
                    .collect();
                assert_eq!(indices.len(), 3, "faces are triangles: {line}");
                parsed.faces.push([indices[0], indices[1], indices[2]]);
                *parsed.faces_per_object.entry(current.clone()).or_default() += 1;
            }
            _ => {}
        }
    }
    parsed
}

/// Triangles the game actually draws for this vehicle: baked submeshes plus the rest-pose
/// instanced running gear.
fn drawn_triangle_count(kind: VehicleKind) -> usize {
    let baked = authoritative_baked_vehicle(kind).expect("bake");
    let mut total: usize =
        baked.submeshes().iter().map(|submesh| submesh.mesh.triangle_count()).sum();
    if let Some(kin) = RunningGearKinematics::for_vehicle(kind) {
        let meshes = (
            road_wheel_unit_mesh(&kin),
            idler_unit_mesh(&kin),
            sprocket_unit_mesh(&kin),
            track_link_unit_mesh(&kin),
            swing_arm_unit_mesh(&kin),
            vehicle_geometry::return_roller_unit_mesh(&kin),
            vehicle_geometry::damper_unit_mesh(&kin),
        );
        for placement in running_gear_placements(&kin, 0.0, 0.0) {
            total += match placement.part {
                GearPart::RoadWheel => meshes.0.triangle_count(),
                GearPart::Idler => meshes.1.triangle_count(),
                GearPart::Sprocket => meshes.2.triangle_count(),
                GearPart::Link => meshes.3.triangle_count(),
                // The left arm mirrors the right: same count by construction.
                GearPart::SwingArm | GearPart::SwingArmLeft => meshes.4.triangle_count(),
                GearPart::ReturnRoller => meshes.5.triangle_count(),
                // Mirrored pair, same count by construction.
                GearPart::Damper | GearPart::DamperLeft => meshes.6.triangle_count(),
            };
        }
    }
    total
}

#[test]
fn the_exported_obj_round_trips_the_whole_drawn_vehicle() {
    let kind = VehicleKind::T54_1951;
    let baked = authoritative_baked_vehicle(kind).expect("bake");
    let export = export_obj(kind, &baked, "t54.mtl");
    let parsed = parse(&export.obj);

    assert_eq!(
        parsed.faces.len(),
        drawn_triangle_count(kind),
        "the export must carry every triangle the game draws — submeshes AND instanced gear"
    );
    assert_eq!(parsed.faces.len(), export.triangle_count, "the reported count must be honest");
    assert_eq!(parsed.positions.len(), export.vertex_count);
    assert_eq!(parsed.normals, parsed.positions.len(), "every vertex carries a normal");

    // Indices are 1-based and must all resolve.
    for face in &parsed.faces {
        for index in face {
            assert!(
                *index >= 1 && *index <= parsed.positions.len(),
                "face index {index} outside 1..={}",
                parsed.positions.len()
            );
        }
    }
}

#[test]
fn the_export_preserves_geometry_and_names_the_parts() {
    let kind = VehicleKind::T54_1951;
    let baked = authoritative_baked_vehicle(kind).expect("bake");
    let export = export_obj(kind, &baked, "t54.mtl");
    let parsed = parse(&export.obj);

    // Bounds survive the text round trip (mm precision is plenty for a tape measure).
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for position in &parsed.positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let hull = baked.submesh(vehicle_geometry::SubmeshKind::Hull).expect("hull").mesh.bounds();
    let hull = hull.expect("hull bounds");
    assert!(min[1] <= hull.min.y + 0.001, "the export must reach the lowest baked point");
    assert!(max[1] >= hull.max.y - 0.001);
    // The gun overhangs the bow: the export's forward reach is the muzzle, not the hull.
    let muzzle_z = baked.mounts().muzzle.translation.z;
    assert!(
        (max[2] - muzzle_z).abs() < 0.05,
        "forward reach {:.3} should be the muzzle at {muzzle_z:.3}",
        max[2]
    );

    // Parts are named per submesh and per gear group+side, so an inspector can isolate them.
    for expected in ["Hull", "Turret", "Gun", "RoadWheels_L", "RoadWheels_R", "TrackLinks_L"] {
        assert!(
            parsed.objects.iter().any(|object| object == expected),
            "the export must name {expected}; got {:?}",
            parsed.objects
        );
    }
    assert!(
        parsed.materials.contains("CastArmor") && parsed.materials.contains("TrackMetal"),
        "material roles travel as usemtl groups: {:?}",
        parsed.materials
    );
    assert!(export.mtl.contains("newmtl CastArmor"), "the MTL declares every role it uses");
    assert!(export.obj.starts_with("# t54_1951"), "the file says what it is");
    assert!(export.obj.contains("mtllib t54.mtl"), "the OBJ points at its MTL");
}

#[test]
fn the_export_is_byte_deterministic() {
    let kind = VehicleKind::T54_1951;
    let baked = authoritative_baked_vehicle(kind).expect("bake");
    let first = export_obj(kind, &baked, "t54.mtl");
    let second = export_obj(kind, &baked, "t54.mtl");
    assert_eq!(first.obj, second.obj, "the export must be reproducible for diffing");
    assert_eq!(first.mtl, second.mtl);
}

/// A vehicle with no running gear kinematics (or a different one) must still export cleanly —
/// the loop is for the whole fleet, not just the benchmark.
#[test]
fn every_playable_vehicle_exports() {
    for kind in VehicleKind::PLAYABLE {
        let baked = authoritative_baked_vehicle(kind).expect("bake");
        let export = export_obj(kind, &baked, "x.mtl");
        let parsed = parse(&export.obj);
        assert_eq!(
            parsed.faces.len(),
            drawn_triangle_count(kind),
            "{kind:?}: exported triangle count must match what is drawn"
        );
        assert!(parsed.objects.len() >= 3, "{kind:?}: hull, turret and gun at minimum");
    }
}
