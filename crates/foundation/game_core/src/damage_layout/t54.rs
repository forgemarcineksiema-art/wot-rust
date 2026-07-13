//! Authoritative T-54-3 obr. 1951 component placement.

use glam::Vec3;

use super::{
    DamageComponent, DamageComponentId, DamageComponentKind as K, DamageLayout,
    DamageMaterial as M, DamageShape,
};
use crate::ModuleSlot;

pub(super) fn layout() -> DamageLayout {
    let mut components = turret_components();
    components.extend(hull_components());
    components.extend(suspension_components());
    DamageLayout { components }
}

fn turret_components() -> Vec<DamageComponent> {
    vec![
        component(
            1,
            K::Breech,
            ModuleSlot::Gun,
            M::Machinery,
            obb([0.0, 0.82, 0.58], [0.34, 0.27, 0.43], 0.0),
            40,
            1.15,
        ),
        component(
            2,
            K::RecoilMechanism,
            ModuleSlot::Gun,
            M::Machinery,
            cylinder_shape([-0.25, 0.98, 0.42], Vec3::Z, 0.42, 0.095),
            38,
            1.0,
        ),
        component(
            3,
            K::RecoilMechanism,
            ModuleSlot::Gun,
            M::Machinery,
            cylinder_shape([0.25, 0.98, 0.42], Vec3::Z, 0.42, 0.095),
            38,
            1.0,
        ),
        component(
            4,
            K::TurretDrive,
            ModuleSlot::Turret,
            M::Driveline,
            cylinder_shape([0.62, 0.47, 0.02], Vec3::Y, 0.24, 0.20),
            34,
            1.0,
        ),
        component(
            7,
            K::Radio,
            ModuleSlot::Radio,
            M::Electronics,
            obb([-0.64, 0.38, 0.98], [0.27, 0.22, 0.20], 0.0),
            28,
            1.4,
        ),
    ]
}

fn hull_components() -> Vec<DamageComponent> {
    vec![
        component(
            5,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([-0.63, -0.04, 0.42], [0.23, 0.48, 0.72], -0.08),
            32,
            1.35,
        ),
        component(
            6,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([0.60, 0.18, -0.28], [0.24, 0.36, 0.56], 0.04),
            32,
            1.35,
        ),
        component(
            8,
            K::FuelTank,
            ModuleSlot::Engine,
            M::Fuel,
            obb([-0.79, 0.03, -1.43], [0.25, 0.43, 0.65], 0.0),
            25,
            1.1,
        ),
        component(
            9,
            K::FuelTank,
            ModuleSlot::Engine,
            M::Fuel,
            obb([0.79, 0.03, -1.43], [0.25, 0.43, 0.65], 0.0),
            25,
            1.1,
        ),
        component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            obb([0.0, 0.02, -1.90], [0.63, 0.48, 0.58], 0.0),
            30,
            0.9,
        ),
        component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            obb([0.0, -0.02, -2.65], [0.68, 0.42, 0.30], 0.0),
            30,
            0.9,
        ),
        component(
            12,
            K::FinalDrive,
            ModuleSlot::Suspension,
            M::Driveline,
            cylinder_shape([-1.08, -0.34, -2.77], Vec3::X, 0.18, 0.24),
            28,
            1.0,
        ),
        component(
            13,
            K::FinalDrive,
            ModuleSlot::Suspension,
            M::Driveline,
            cylinder_shape([1.08, -0.34, -2.77], Vec3::X, 0.18, 0.24),
            28,
            1.0,
        ),
    ]
}

fn suspension_components() -> [DamageComponent; 2] {
    [-1.24, 1.24].map(|x| DamageComponent {
        id: DamageComponentId(if x < 0.0 { 14 } else { 15 }),
        kind: K::Suspension,
        slot: ModuleSlot::Suspension,
        material: M::SuspensionSteel,
        shape: DamageShape::Capsule {
            a: Vec3::new(x, -0.70, -2.65),
            b: Vec3::new(x, -0.70, 2.65),
            radius: 0.18,
        },
        priority: 20,
        requires_penetration: false,
        vulnerability: 0.8,
    })
}

fn component(
    id: u16,
    kind: K,
    slot: ModuleSlot,
    material: M,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    DamageComponent {
        id: DamageComponentId(id),
        kind,
        slot,
        material,
        shape,
        priority,
        requires_penetration: true,
        vulnerability,
    }
}

fn obb(center: [f32; 3], half_extents: [f32; 3], yaw_rad: f32) -> DamageShape {
    DamageShape::Obb {
        center: Vec3::from_array(center),
        half_extents: Vec3::from_array(half_extents),
        yaw_rad,
    }
}

fn cylinder_shape(center: [f32; 3], axis: Vec3, half_length: f32, radius: f32) -> DamageShape {
    DamageShape::Cylinder { center: Vec3::from_array(center), axis, half_length, radius }
}
