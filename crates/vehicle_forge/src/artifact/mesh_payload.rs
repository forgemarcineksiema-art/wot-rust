use std::io::{self, Write};

use vehicle_geometry::{BakedVehicle, GeometryVertex, MaterialRole, SubmeshKind};

pub fn encode(vehicle: &BakedVehicle) -> Result<Vec<u8>, io::Error> {
    let mut bytes = Vec::new();
    bytes.write_all(b"WOTFORGE")?;
    write_u32(&mut bytes, 1)?;
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

fn write_u32(bytes: &mut Vec<u8>, value: u32) -> Result<(), io::Error> {
    bytes.write_all(&value.to_le_bytes())
}

fn write_f32(bytes: &mut Vec<u8>, value: f32) -> Result<(), io::Error> {
    bytes.write_all(&value.to_le_bytes())
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
