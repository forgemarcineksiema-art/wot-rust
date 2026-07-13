//! Damage-ready T-54 interior. The primary assemblies are generated from `DamageLayout`, so the
//! object seen through a perforation and the object intersected by the authoritative ray cannot
//! drift into two different tanks.

use game_core::{
    DamageComponent, DamageComponentKind, DamageLayout, DamageMaterial, DamageShape,
    VehicleBlueprint, VehicleKind,
};
use glam::{Mat3, Mat4, Quat, Vec3};
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

pub(crate) fn t54_interior_parts() -> Vec<VehiclePart> {
    let spec = VehicleKind::T54_1951.spec();
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let visual = blueprint.hybrid().expect("T-54 hybrid visual");
    let center_y = spec.hitbox.center_y_m;
    let mut parts: Vec<_> = DamageLayout::t54_1951()
        .components()
        .iter()
        .filter(|component| component.kind == DamageComponentKind::FuelTank)
        .map(|component| component_part(component, center_y))
        .collect();

    parts.push(VehiclePart {
        key: PartKey::new("turret_inner_skin"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::InteriorPrimer,
        smoothing: SmoothingGroup(22),
        shape: PartShape::Mesh(turret_inner_skin(&crate::t54_turret_loft::t54_turret_loft(
            &visual.turret_loft,
        ))),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::CastLoft,
    });

    // Inner painted envelope and the fighting/engine bulkhead. These thin shells sit inside the
    // real armor and are cut by the same analytic aperture depth as the exterior skin.
    for (index, (center, half)) in [
        (Vec3::new(0.0, center_y - 0.63, 0.0), Vec3::new(0.98, 0.025, 2.20)),
        (Vec3::new(-1.12, center_y - 0.18, 0.0), Vec3::new(0.025, 0.42, 2.20)),
        (Vec3::new(1.12, center_y - 0.18, 0.0), Vec3::new(0.025, 0.42, 2.20)),
        (Vec3::new(0.0, center_y - 0.18, 2.22), Vec3::new(0.98, 0.42, 0.025)),
        (Vec3::new(0.0, center_y - 0.18, -1.02), Vec3::new(0.98, 0.42, 0.035)),
    ]
    .into_iter()
    .enumerate()
    {
        parts.push(box_part(
            PartKey::indexed("interior_liner", index as u16),
            SubmeshKind::Hull,
            center,
            half,
            MaterialRole::InteriorPrimer,
            PartLod::Silhouette,
        ));
    }

    // Driver's station and the coarse rotating fighting-compartment basket.
    parts.push(box_part(
        PartKey::new("driver_seat"),
        SubmeshKind::Hull,
        Vec3::new(-0.55, center_y - 0.20, 1.72),
        Vec3::new(0.24, 0.36, 0.27),
        MaterialRole::InteriorMachinery,
        PartLod::Silhouette,
    ));
    parts.push(drum_part(
        PartKey::new("turret_basket_floor"),
        SubmeshKind::Turret,
        Vec3::new(0.0, spec.mounts.turret_ring.translation.y - 0.57, 0.0),
        Vec3::Y,
        (0.035, 0.78),
        MaterialRole::InteriorPrimer,
        PartLod::Silhouette,
    ));
    for (index, x) in [-0.66_f32, 0.66].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("turret_basket_strut", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, spec.mounts.turret_ring.translation.y - 0.22, 0.0),
            Vec3::new(0.025, 0.38, 0.025),
            MaterialRole::InteriorMachinery,
            PartLod::Silhouette,
        ));
    }

    parts.extend(crate::t54_interior_detail::t54_museum_detail_parts(center_y));
    parts
}

fn turret_inner_skin(outer: &GeometryMesh) -> GeometryMesh {
    let vertices = outer
        .vertices()
        .iter()
        .map(|vertex| {
            let n = vertex.normal.normalize_or_zero();
            let thickness = if n.y.abs() > 0.68 {
                0.035
            } else if n.z > 0.45 {
                0.18
            } else if n.z < -0.55 {
                0.08
            } else {
                0.105
            };
            GeometryVertex {
                position: vertex.position - n * thickness,
                normal: -n,
                material: MaterialRole::InteriorPrimer,
                surface_shade: 0.72,
                ..*vertex
            }
        })
        .collect();
    let mut indices = outer.indices().to_vec();
    for triangle in indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    GeometryMesh::new(vertices, indices)
}

fn component_part(component: &DamageComponent, center_y: f32) -> VehiclePart {
    let submesh = match component.kind {
        DamageComponentKind::Breech
        | DamageComponentKind::RecoilMechanism
        | DamageComponentKind::TurretDrive
        | DamageComponentKind::Radio => SubmeshKind::Turret,
        _ => SubmeshKind::Hull,
    };
    let material = match component.material {
        DamageMaterial::Ammunition => MaterialRole::Ammunition,
        DamageMaterial::Electronics => MaterialRole::InteriorPrimer,
        _ => MaterialRole::InteriorMachinery,
    };
    let mesh = component_mesh(&component.shape, center_y, material);
    VehiclePart {
        key: PartKey::indexed("damage_component", component.id.0),
        submesh,
        material,
        smoothing: SmoothingGroup(21),
        shape: PartShape::Mesh(mesh),
        lod: PartLod::Silhouette,
        generator: GeneratorKind::Revolve,
    }
}

fn component_mesh(shape: &DamageShape, center_y: f32, material: MaterialRole) -> GeometryMesh {
    match shape {
        DamageShape::Obb { center, half_extents, yaw_rad } => {
            let center = *center + Vec3::Y * center_y;
            let mesh = solid::ConvexSolid::box_at(center, *half_extents)
                .to_mesh(material, SmoothingGroup::hard_edges())
                .expect("component OBB");
            transform_mesh(
                &mesh,
                Mat4::from_translation(center)
                    * Mat4::from_rotation_y(*yaw_rad)
                    * Mat4::from_translation(-center),
            )
        }
        DamageShape::Cylinder { center, axis, half_length, radius } => {
            drum_mesh(*center + Vec3::Y * center_y, *axis, *half_length, *radius, material)
        }
        DamageShape::Capsule { a, b, radius } => {
            capsule_mesh(*a + Vec3::Y * center_y, *b + Vec3::Y * center_y, *radius, material)
        }
        DamageShape::Convex { bounds_min, bounds_max, .. } => {
            let min = *bounds_min + Vec3::Y * center_y;
            let max = *bounds_max + Vec3::Y * center_y;
            solid::ConvexSolid::box_at((min + max) * 0.5, (max - min) * 0.5)
                .to_mesh(material, SmoothingGroup::hard_edges())
                .expect("component convex bounds")
        }
    }
}

pub(crate) fn box_part(
    key: PartKey,
    submesh: SubmeshKind,
    center: Vec3,
    half: Vec3,
    material: MaterialRole,
    lod: PartLod,
) -> VehiclePart {
    VehiclePart {
        key,
        submesh,
        material,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Plates(solid::ConvexSolid::box_at(center, half)),
        lod,
        generator: GeneratorKind::Solid,
    }
}

pub(crate) fn drum_part(
    key: PartKey,
    submesh: SubmeshKind,
    center: Vec3,
    axis: Vec3,
    dimensions: (f32, f32),
    material: MaterialRole,
    lod: PartLod,
) -> VehiclePart {
    let (half_length, radius) = dimensions;
    VehiclePart {
        key,
        submesh,
        material,
        smoothing: SmoothingGroup(21),
        shape: PartShape::Mesh(drum_mesh(center, axis, half_length, radius, material)),
        lod,
        generator: GeneratorKind::Revolve,
    }
}

fn drum_mesh(
    center: Vec3,
    axis: Vec3,
    half_length: f32,
    radius: f32,
    material: MaterialRole,
) -> GeometryMesh {
    let base = revolve::drum(Vec3::ZERO, radius, half_length, 16, material, SmoothingGroup(21));
    let rotation = Quat::from_rotation_arc(Vec3::Y, axis.normalize_or_zero());
    transform_mesh(&base, Mat4::from_rotation_translation(rotation, center))
}

fn capsule_mesh(a: Vec3, b: Vec3, radius: f32, material: MaterialRole) -> GeometryMesh {
    let center = (a + b) * 0.5;
    let axis = b - a;
    // Closed rounded visual proxy sharing the authored axis, endpoints and radius.
    drum_mesh(center, axis, axis.length() * 0.5 + radius * 0.35, radius, material)
}

fn transform_mesh(mesh: &GeometryMesh, transform: Mat4) -> GeometryMesh {
    let normal_matrix = Mat3::from_mat4(transform);
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| GeometryVertex {
            position: transform.transform_point3(vertex.position),
            normal: (normal_matrix * vertex.normal).normalize_or_zero(),
            ..*vertex
        })
        .collect();
    GeometryMesh::new(vertices, mesh.indices().to_vec())
}
