//! Wavefront OBJ export of a baked vehicle — the repo's window into external inspection tools.
//!
//! The Forge's own review tiles are deterministic and cheap, but they are 512x320 pixels of a
//! CPU rasterizer: they cannot be orbited, sectioned, measured with a ruler, or overlaid on a
//! scale drawing. The Model Idealny program's master-reference loop (Blender) needs the actual
//! mesh, so this writes one — text-only, no new dependencies, and renderer-free (the crate rule
//! bars renderer BACKENDS, not file writers; `mesh_payload` already writes bytes next door).
//!
//! What travels: the baked submeshes at rest pose plus the rest-pose running gear, each as its
//! own `o` object, with `usemtl` groups per [`MaterialRole`] so an inspector can isolate the
//! cast turret from the rolled hull or the rubber from the track steel.
//!
//! The direction of travel is one-way by policy: geometry leaves for inspection, and only
//! NUMBERS come back into the blueprint. Nothing in the game is ever authored from an imported
//! mesh (no-clones / procedural-only).

use std::fmt::Write as _;

use game_core::VehicleKind;
use glam::Mat4;
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, MaterialRole, RunningGearKinematics, SubmeshKind,
    idler_unit_mesh, return_roller_unit_mesh, road_wheel_unit_mesh, running_gear_placements,
    sprocket_unit_mesh, swing_arm_unit_mesh, track_link_unit_mesh,
};

/// The OBJ text plus its companion MTL text. Both are plain files; the OBJ references the MTL
/// by the `mtllib` name the caller writes it under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjExport {
    pub obj: String,
    pub mtl: String,
    /// Objects written, in order — the names an inspector sees in the outliner.
    pub objects: Vec<String>,
    pub triangle_count: usize,
    pub vertex_count: usize,
}

fn material_name(role: MaterialRole) -> &'static str {
    match role {
        MaterialRole::RolledArmor => "RolledArmor",
        MaterialRole::CastArmor => "CastArmor",
        MaterialRole::BarrelSteel => "BarrelSteel",
        MaterialRole::TrackMetal => "TrackMetal",
        MaterialRole::Rubber => "Rubber",
        MaterialRole::InteriorPrimer => "InteriorPrimer",
        MaterialRole::InteriorMachinery => "InteriorMachinery",
        MaterialRole::Ammunition => "Ammunition",
        MaterialRole::ExposedSteel => "ExposedSteel",
        MaterialRole::Canvas => "Canvas",
        MaterialRole::Glass => "Glass",
        MaterialRole::Timber => "Timber",
    }
}

/// Review colours only — a neutral, readable stand-in so parts are distinguishable in an
/// external viewer. The game's real materials are the baked family maps, not these.
fn material_colour(role: MaterialRole) -> [f32; 3] {
    match role {
        MaterialRole::RolledArmor => [0.33, 0.36, 0.25],
        MaterialRole::CastArmor => [0.30, 0.33, 0.24],
        MaterialRole::BarrelSteel => [0.28, 0.30, 0.28],
        MaterialRole::TrackMetal => [0.16, 0.16, 0.17],
        MaterialRole::Rubber => [0.05, 0.05, 0.05],
        MaterialRole::InteriorPrimer => [0.85, 0.83, 0.72],
        MaterialRole::InteriorMachinery => [0.35, 0.37, 0.40],
        MaterialRole::Ammunition => [0.65, 0.52, 0.28],
        MaterialRole::ExposedSteel => [0.45, 0.46, 0.48],
        MaterialRole::Canvas => [0.52, 0.51, 0.44],
        MaterialRole::Glass => [0.80, 0.86, 0.88],
        MaterialRole::Timber => [0.42, 0.33, 0.23],
    }
}

const ROLES: [MaterialRole; 12] = [
    MaterialRole::RolledArmor,
    MaterialRole::CastArmor,
    MaterialRole::BarrelSteel,
    MaterialRole::TrackMetal,
    MaterialRole::Rubber,
    MaterialRole::InteriorPrimer,
    MaterialRole::InteriorMachinery,
    MaterialRole::Ammunition,
    MaterialRole::ExposedSteel,
    MaterialRole::Canvas,
    MaterialRole::Glass,
    MaterialRole::Timber,
];

fn submesh_object_name(kind: SubmeshKind) -> &'static str {
    match kind {
        SubmeshKind::Hull => "Hull",
        SubmeshKind::Turret => "Turret",
        SubmeshKind::Gun => "Gun",
    }
}

fn gear_object_name(part: GearPart, side: f32) -> String {
    let base = match part {
        GearPart::RoadWheel => "RoadWheels",
        GearPart::Idler => "Idlers",
        GearPart::Sprocket => "Sprockets",
        GearPart::Link => "TrackLinks",
        // One outliner group either side: the left arm is the same assembly mirrored, and
        // the side suffix below already tells the two apart.
        GearPart::SwingArm | GearPart::SwingArmLeft => "SwingArms",
        GearPart::ReturnRoller => "ReturnRollers",
    };
    // Side by the world convention (+X is the vehicle's right) so an inspector can check the
    // asymmetric fittings against a photograph without guessing which belt is which.
    format!("{base}_{}", if side >= 0.0 { "R" } else { "L" })
}

struct Writer {
    obj: String,
    objects: Vec<String>,
    vertex_offset: u32,
    triangles: usize,
    vertices: usize,
}

impl Writer {
    /// One `o` object built from any number of transformed instances of the same or different
    /// meshes, faces grouped by material.
    fn object(&mut self, name: &str, instances: &[(&GeometryMesh, Mat4)]) {
        let mut faces: Vec<(MaterialRole, [u32; 3])> = Vec::new();
        let mut body = String::new();
        for (mesh, transform) in instances {
            // Normals need the inverse transpose; the gear transforms are rigid today, but a
            // scaled instance must not ship inverted lighting to whoever opens the file.
            let normal_matrix = transform.inverse().transpose();
            for vertex in mesh.vertices() {
                let position = transform.transform_point3(vertex.position);
                let normal = normal_matrix.transform_vector3(vertex.normal).normalize_or_zero();
                let _ = writeln!(body, "v {:.6} {:.6} {:.6}", position.x, position.y, position.z);
                let _ = writeln!(body, "vn {:.4} {:.4} {:.4}", normal.x, normal.y, normal.z);
            }
            let vertices = mesh.vertices();
            for triangle in mesh.indices().chunks_exact(3) {
                let role = vertices[triangle[0] as usize].material;
                faces.push((
                    role,
                    [
                        self.vertex_offset + triangle[0] + 1,
                        self.vertex_offset + triangle[1] + 1,
                        self.vertex_offset + triangle[2] + 1,
                    ],
                ));
                self.triangles += 1;
            }
            self.vertex_offset += vertices.len() as u32;
            self.vertices += vertices.len();
        }
        if faces.is_empty() {
            return;
        }
        let _ = writeln!(self.obj, "o {name}");
        self.obj.push_str(&body);
        // Deterministic material order: the file must be byte-stable across runs.
        for role in ROLES {
            let mut wrote_group = false;
            for (face_role, indices) in faces.iter().filter(|(r, _)| *r == role) {
                if !wrote_group {
                    let _ = writeln!(self.obj, "usemtl {}", material_name(*face_role));
                    wrote_group = true;
                }
                let _ = writeln!(
                    self.obj,
                    "f {0}//{0} {1}//{1} {2}//{2}",
                    indices[0], indices[1], indices[2]
                );
            }
        }
        self.objects.push(name.to_string());
    }
}

/// Export `baked` (plus `kind`'s rest-pose running gear, when it has any) as OBJ + MTL text.
///
/// Coordinates are the model's own: +X right, +Y up, +Z forward, origin on the ground plane.
/// Import into Blender with forward `Z`, up `Y` to land the tank upright and facing away.
pub fn export_obj(kind: VehicleKind, baked: &BakedVehicle, mtl_name: &str) -> ObjExport {
    let mut writer = Writer {
        obj: format!(
            "# {} — baked by vehicle_forge (inspection export)\nmtllib {mtl_name}\n",
            kind.slug()
        ),
        objects: Vec::new(),
        vertex_offset: 0,
        triangles: 0,
        vertices: 0,
    };

    for submesh in baked.submeshes() {
        writer.object(submesh_object_name(submesh.kind), &[(&submesh.mesh, Mat4::IDENTITY)]);
    }

    if let Some(kin) = RunningGearKinematics::for_vehicle(kind) {
        let road_wheel = road_wheel_unit_mesh(&kin);
        let idler = idler_unit_mesh(&kin);
        let sprocket = sprocket_unit_mesh(&kin);
        let link = track_link_unit_mesh(&kin);
        let swing_arm = swing_arm_unit_mesh(&kin);
        let swing_arm_left = vehicle_geometry::swing_arm_unit_mesh_left(&kin);
        let return_roller = return_roller_unit_mesh(&kin);
        // Group instances per part and side so the outliner reads like the real assembly.
        let mut groups: Vec<(String, Vec<(&GeometryMesh, Mat4)>)> = Vec::new();
        for placement in running_gear_placements(&kin, 0.0, 0.0) {
            let mesh = match placement.part {
                GearPart::RoadWheel => &road_wheel,
                GearPart::Idler => &idler,
                GearPart::Sprocket => &sprocket,
                GearPart::Link => &link,
                GearPart::SwingArm => &swing_arm,
                GearPart::SwingArmLeft => &swing_arm_left,
                GearPart::ReturnRoller => &return_roller,
            };
            let name = gear_object_name(placement.part, placement.transform.w_axis.x);
            match groups.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, instances)) => instances.push((mesh, placement.transform)),
                None => groups.push((name, vec![(mesh, placement.transform)])),
            }
        }
        for (name, instances) in &groups {
            writer.object(name, instances);
        }
    }

    let mut mtl = String::from("# review materials — neutral stand-ins, not the game's maps\n");
    for role in ROLES {
        let colour = material_colour(role);
        let _ = writeln!(mtl, "\nnewmtl {}", material_name(role));
        let _ = writeln!(mtl, "Kd {:.3} {:.3} {:.3}", colour[0], colour[1], colour[2]);
        let _ = writeln!(mtl, "Ka 0.000 0.000 0.000");
    }

    ObjExport {
        obj: writer.obj,
        mtl,
        objects: writer.objects,
        triangle_count: writer.triangles,
        vertex_count: writer.vertices,
    }
}
