use std::io::{self, Cursor, Read, Write};

use game_core::{MountFrame, MountFrames, VehicleKind};
use glam::{Mat3, Vec3};
use vehicle_geometry::{
    BakedVehicle, GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, Submesh, SubmeshKind,
};

const MAGIC: &[u8; 8] = b"WOTFORGE";
const VERSION: u32 = 2;

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
        for index in submesh.mesh.indices() {
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
    if version != VERSION {
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
            vertices.push(read_vertex(&mut cursor)?);
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
    write_u32(bytes, payload_material_role_id(vertex.material))?;
    write_u32(bytes, u32::from(vertex.smoothing.0))?;
    write_f32(bytes, vertex.surface_shade)?;
    Ok(())
}

fn read_vertex(cursor: &mut Cursor<&[u8]>) -> Result<GeometryVertex, io::Error> {
    let position = Vec3::new(read_f32(cursor)?, read_f32(cursor)?, read_f32(cursor)?);
    let normal = Vec3::new(read_f32(cursor)?, read_f32(cursor)?, read_f32(cursor)?);
    let material = read_material_role(cursor)?;
    let smoothing = SmoothingGroup(read_u32(cursor)? as u16);
    let surface_shade = read_f32(cursor)?;
    Ok(GeometryVertex { position, normal, material, smoothing, surface_shade })
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
    }
}

fn read_material_role(cursor: &mut Cursor<&[u8]>) -> Result<MaterialRole, io::Error> {
    Ok(match read_u32(cursor)? {
        0 => MaterialRole::RolledArmor,
        1 => MaterialRole::CastArmor,
        2 => MaterialRole::BarrelSteel,
        3 => MaterialRole::TrackMetal,
        4 => MaterialRole::Rubber,
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
