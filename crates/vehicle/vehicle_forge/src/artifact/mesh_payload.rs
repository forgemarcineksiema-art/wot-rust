use std::io::{self, Cursor, Read, Write};

use game_core::{MountFrame, MountFrames, VehicleKind};
use glam::{Mat3, Vec2, Vec3};
use vehicle_geometry::{
    BakedVehicle, GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, Submesh, SubmeshKind,
    SurfaceMapping,
};

use super::mesh_optimize::optimize_indices_for_gpu;

const MAGIC: &[u8; 8] = b"WOTFORGE";
/// Current payload version: carries per-vertex UV0 and surface mapping mode.
const VERSION: u32 = 3;
/// The previous version, lacking UV0/mapping — decoded by supplying `(0, 0)` and `Triplanar`.
const VERSION_LEGACY_UV: u32 = 2;

pub fn encode(vehicle: &BakedVehicle) -> Result<Vec<u8>, io::Error> {
    let mut bytes = Vec::new();
    bytes.write_all(MAGIC)?;
    write_u32(&mut bytes, VERSION)?;
    write_mounts(&mut bytes, vehicle.mounts())?;
    write_u32(&mut bytes, vehicle.submeshes().len() as u32)?;
    for submesh in vehicle.submeshes() {
        write_u32(&mut bytes, submesh.kind as u32)?;
        write_u32(&mut bytes, submesh.mesh.vertex_count() as u32)?;
        write_u32(&mut bytes, submesh.mesh.indices().len() as u32)?;
        for vertex in submesh.mesh.vertices() {
            write_vertex(&mut bytes, vertex)?;
        }
        let indices = optimize_indices_for_gpu(submesh.mesh.indices(), submesh.mesh.vertex_count());
        for index in &indices {
            write_u32(&mut bytes, *index)?;
        }
    }
    Ok(bytes)
}

pub fn decode(kind: VehicleKind, bytes: &[u8]) -> Result<BakedVehicle, io::Error> {
    let mut cursor = Cursor::new(bytes);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return invalid_data("Forge mesh payload has an invalid magic header");
    }
    let version = read_u32(&mut cursor)?;
    if version != VERSION && version != VERSION_LEGACY_UV {
        return invalid_data("Forge mesh payload version is not supported");
    }
    let mounts = read_mounts(&mut cursor)?;
    let mut submeshes = Vec::new();
    let submesh_count = read_u32(&mut cursor)? as usize;
    for _ in 0..submesh_count {
        let submesh_kind = read_submesh_kind(&mut cursor)?;
        let vertex_count = read_u32(&mut cursor)? as usize;
        let index_count = read_u32(&mut cursor)? as usize;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(read_vertex(&mut cursor, version)?);
        }
        let mut indices = Vec::with_capacity(index_count);
        for _ in 0..index_count {
            indices.push(read_u32(&mut cursor)?);
        }
        submeshes.push(Submesh { kind: submesh_kind, mesh: GeometryMesh::new(vertices, indices) });
    }
    if cursor.position() as usize != bytes.len() {
        return invalid_data("Forge mesh payload has trailing bytes");
    }
    Ok(BakedVehicle::new(kind, submeshes, mounts))
}

pub fn submesh_kind_name(kind: SubmeshKind) -> &'static str {
    match kind {
        SubmeshKind::Hull => "Hull",
        SubmeshKind::Turret => "Turret",
        SubmeshKind::Gun => "Gun",
    }
}

fn write_vertex(bytes: &mut Vec<u8>, vertex: &GeometryVertex) -> Result<(), io::Error> {
    for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array()) {
        write_f32(bytes, value)?;
    }
    // v3: UV0 and the mapping mode follow the normal, before the material/smoothing/shade tail.
    write_f32(bytes, vertex.uv0.x)?;
    write_f32(bytes, vertex.uv0.y)?;
    write_u32(bytes, surface_mapping_id(vertex.mapping))?;
    write_u32(bytes, payload_material_role_id(vertex.material))?;
    write_u32(bytes, u32::from(vertex.smoothing.0))?;
    write_f32(bytes, vertex.surface_shade)?;
    Ok(())
}

fn read_vertex(cursor: &mut Cursor<&[u8]>, version: u32) -> Result<GeometryVertex, io::Error> {
    let position = Vec3::new(read_f32(cursor)?, read_f32(cursor)?, read_f32(cursor)?);
    let normal = Vec3::new(read_f32(cursor)?, read_f32(cursor)?, read_f32(cursor)?);
    // Legacy v2 payloads carry no UV/mapping — supply the triplanar default at the origin.
    let (uv0, mapping) = if version >= VERSION {
        (Vec2::new(read_f32(cursor)?, read_f32(cursor)?), read_surface_mapping(cursor)?)
    } else {
        (Vec2::ZERO, SurfaceMapping::Triplanar)
    };
    let material = read_material_role(cursor)?;
    let smoothing = SmoothingGroup(read_u32(cursor)? as u16);
    let surface_shade = read_f32(cursor)?;
    let mut vertex = GeometryVertex::new(position, normal, material, smoothing);
    vertex.uv0 = uv0;
    vertex.mapping = mapping;
    Ok(vertex.with_surface_shade(surface_shade))
}

fn surface_mapping_id(mapping: SurfaceMapping) -> u32 {
    match mapping {
        SurfaceMapping::ParametricUv => 0,
        SurfaceMapping::Triplanar => 1,
    }
}

fn read_surface_mapping(cursor: &mut Cursor<&[u8]>) -> Result<SurfaceMapping, io::Error> {
    Ok(match read_u32(cursor)? {
        0 => SurfaceMapping::ParametricUv,
        1 => SurfaceMapping::Triplanar,
        _ => return invalid_data("Forge mesh payload has an unknown surface mapping mode"),
    })
}

fn write_mounts(bytes: &mut Vec<u8>, mounts: &MountFrames) -> Result<(), io::Error> {
    for frame in [mounts.turret_ring, mounts.gun_trunnion, mounts.muzzle] {
        write_frame(bytes, frame)?;
    }
    Ok(())
}

fn read_mounts(cursor: &mut Cursor<&[u8]>) -> Result<MountFrames, io::Error> {
    Ok(MountFrames {
        turret_ring: read_frame(cursor)?,
        gun_trunnion: read_frame(cursor)?,
        muzzle: read_frame(cursor)?,
    })
}

fn write_frame(bytes: &mut Vec<u8>, frame: MountFrame) -> Result<(), io::Error> {
    for value in frame.translation.to_array() {
        write_f32(bytes, value)?;
    }
    for col in frame.basis.to_cols_array_2d() {
        for value in col {
            write_f32(bytes, value)?;
        }
    }
    Ok(())
}

fn read_frame(cursor: &mut Cursor<&[u8]>) -> Result<MountFrame, io::Error> {
    let translation = Vec3::new(read_f32(cursor)?, read_f32(cursor)?, read_f32(cursor)?);
    let mut cols = [[0.0; 3]; 3];
    for col in &mut cols {
        for value in col {
            *value = read_f32(cursor)?;
        }
    }
    Ok(MountFrame::with_basis(translation, Mat3::from_cols_array_2d(&cols)))
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) -> Result<(), io::Error> {
    bytes.write_all(&value.to_le_bytes())
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> Result<u32, io::Error> {
    let mut bytes = [0; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_f32(bytes: &mut Vec<u8>, value: f32) -> Result<(), io::Error> {
    bytes.write_all(&value.to_le_bytes())
}

fn read_f32(cursor: &mut Cursor<&[u8]>) -> Result<f32, io::Error> {
    let mut bytes = [0; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(f32::from_le_bytes(bytes))
}

fn payload_material_role_id(material: MaterialRole) -> u32 {
    match material {
        MaterialRole::RolledArmor => 0,
        MaterialRole::CastArmor => 1,
        MaterialRole::BarrelSteel => 2,
        MaterialRole::TrackMetal => 3,
        MaterialRole::Rubber => 4,
        MaterialRole::InteriorPrimer => 5,
        MaterialRole::InteriorMachinery => 6,
        MaterialRole::Ammunition => 7,
        MaterialRole::ExposedSteel => 8,
        MaterialRole::Canvas => 9,
        MaterialRole::Glass => 10,
    }
}

fn read_material_role(cursor: &mut Cursor<&[u8]>) -> Result<MaterialRole, io::Error> {
    Ok(match read_u32(cursor)? {
        0 => MaterialRole::RolledArmor,
        1 => MaterialRole::CastArmor,
        2 => MaterialRole::BarrelSteel,
        3 => MaterialRole::TrackMetal,
        4 => MaterialRole::Rubber,
        5 => MaterialRole::InteriorPrimer,
        6 => MaterialRole::InteriorMachinery,
        7 => MaterialRole::Ammunition,
        8 => MaterialRole::ExposedSteel,
        9 => MaterialRole::Canvas,
        10 => MaterialRole::Glass,
        _ => return invalid_data("Forge mesh payload has an unknown material role"),
    })
}

fn read_submesh_kind(cursor: &mut Cursor<&[u8]>) -> Result<SubmeshKind, io::Error> {
    Ok(match read_u32(cursor)? {
        0 => SubmeshKind::Hull,
        1 => SubmeshKind::Turret,
        2 => SubmeshKind::Gun,
        _ => return invalid_data("Forge mesh payload has an unknown submesh kind"),
    })
}

fn invalid_data<T>(message: &'static str) -> Result<T, io::Error> {
    Err(io::Error::new(io::ErrorKind::InvalidData, message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use vehicle_geometry::SurfaceMapping;

    fn sample_vehicle() -> BakedVehicle {
        let v =
            |p: Vec3| GeometryVertex::new(p, Vec3::Y, MaterialRole::CastArmor, SmoothingGroup(2));
        let parametric = v(Vec3::X).with_uv0(Vec2::new(0.25, 0.75));
        let mesh = GeometryMesh::new(vec![v(Vec3::ZERO), parametric, v(Vec3::Z)], vec![0, 1, 2]);
        BakedVehicle::new(
            VehicleKind::T54_1951,
            vec![Submesh { kind: SubmeshKind::Hull, mesh }],
            MountFrames::for_vehicle(VehicleKind::T54_1951),
        )
    }

    #[test]
    fn version_3_round_trips_uv_and_mapping() {
        let vehicle = sample_vehicle();
        let decoded = decode(VehicleKind::T54_1951, &encode(&vehicle).unwrap()).unwrap();
        let original = vehicle.submeshes()[0].mesh.vertices();
        let round = decoded.submeshes()[0].mesh.vertices();
        assert_eq!(round.len(), original.len());
        for (a, b) in original.iter().zip(round) {
            assert_eq!(a.uv0, b.uv0);
            assert_eq!(a.mapping, b.mapping);
            assert_eq!(a.position, b.position);
        }
        // The parametric vertex survived as parametric; the others stayed triplanar.
        assert_eq!(round[1].mapping, SurfaceMapping::ParametricUv);
        assert_eq!(round[0].mapping, SurfaceMapping::Triplanar);
    }

    /// Re-encode a vehicle in the legacy v2 layout (no UV/mapping) for the back-compat test.
    fn encode_v2(vehicle: &BakedVehicle) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.write_all(MAGIC).unwrap();
        write_u32(&mut bytes, VERSION_LEGACY_UV).unwrap();
        write_mounts(&mut bytes, vehicle.mounts()).unwrap();
        write_u32(&mut bytes, vehicle.submeshes().len() as u32).unwrap();
        for submesh in vehicle.submeshes() {
            write_u32(&mut bytes, submesh.kind as u32).unwrap();
            write_u32(&mut bytes, submesh.mesh.vertex_count() as u32).unwrap();
            write_u32(&mut bytes, submesh.mesh.indices().len() as u32).unwrap();
            for vtx in submesh.mesh.vertices() {
                for value in vtx.position.to_array().into_iter().chain(vtx.normal.to_array()) {
                    write_f32(&mut bytes, value).unwrap();
                }
                write_u32(&mut bytes, payload_material_role_id(vtx.material)).unwrap();
                write_u32(&mut bytes, u32::from(vtx.smoothing.0)).unwrap();
                write_f32(&mut bytes, vtx.surface_shade).unwrap();
            }
            for index in submesh.mesh.indices() {
                write_u32(&mut bytes, *index).unwrap();
            }
        }
        bytes
    }

    #[test]
    fn version_2_decodes_with_triplanar_defaults() {
        let vehicle = sample_vehicle();
        let decoded = decode(VehicleKind::T54_1951, &encode_v2(&vehicle)).unwrap();
        for vtx in decoded.submeshes()[0].mesh.vertices() {
            assert_eq!(
                vtx.mapping,
                SurfaceMapping::Triplanar,
                "legacy vertices default to triplanar"
            );
            assert_eq!(vtx.uv0, Vec2::ZERO, "legacy vertices have no UV");
        }
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let mut bytes = encode(&sample_vehicle()).unwrap();
        // Overwrite the version word (just after the 8-byte magic) with an unsupported value.
        bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(decode(VehicleKind::T54_1951, &bytes).is_err());
    }

    #[test]
    fn meshopt_payload_pass_preserves_vertices_materials_and_triangle_count() {
        let vehicle = sample_vehicle();
        let decoded = decode(VehicleKind::T54_1951, &encode(&vehicle).unwrap()).unwrap();
        let original = &vehicle.submeshes()[0].mesh;
        let optimized = &decoded.submeshes()[0].mesh;

        assert_eq!(optimized.vertex_count(), original.vertex_count());
        assert_eq!(optimized.indices().len(), original.indices().len());
        assert_eq!(optimized.indices().len() / 3, original.indices().len() / 3);
        for (a, b) in original.vertices().iter().zip(optimized.vertices()) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.material, b.material);
            assert_eq!(a.smoothing, b.smoothing);
        }
    }
}
