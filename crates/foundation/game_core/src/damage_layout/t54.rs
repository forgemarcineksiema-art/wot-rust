//! Authoritative T-54-3 obr. 1951 component placement.

use glam::Vec3;

use super::{
    DamageComponent, DamageComponentId, DamageComponentKind as K, DamageLayout,
    DamageMaterial as M, DamageShape,
};
use crate::{ArmorFrame, ModuleSlot};

pub(super) fn layout() -> DamageLayout {
    let mut components = turret_components();
    components.extend(hull_components());
    components.extend(suspension_components());
    DamageLayout { components }
}

fn turret_components() -> Vec<DamageComponent> {
    vec![
        turret_component(
            1,
            K::Breech,
            ModuleSlot::Gun,
            M::Machinery,
            obb([0.0, 0.82, 0.58], [0.34, 0.27, 0.43], 0.0),
            40,
            1.15,
        ),
        turret_component(
            2,
            K::RecoilMechanism,
            ModuleSlot::Gun,
            M::Machinery,
            cylinder_shape([-0.25, 0.98, 0.42], Vec3::Z, 0.42, 0.095),
            38,
            1.0,
        ),
        turret_component(
            3,
            K::RecoilMechanism,
            ModuleSlot::Gun,
            M::Machinery,
            cylinder_shape([0.25, 0.98, 0.42], Vec3::Z, 0.42, 0.095),
            38,
            1.0,
        ),
        turret_component(
            4,
            K::TurretDrive,
            ModuleSlot::Turret,
            M::Driveline,
            cylinder_shape([0.62, 0.47, 0.02], Vec3::Y, 0.24, 0.20),
            34,
            1.0,
        ),
        turret_component(
            7,
            K::Radio,
            ModuleSlot::Radio,
            M::Electronics,
            // Pulled 18 cm rearward (model-logic audit #13): at z 0.98 the set's forward face
            // reached 1.18 — clear THROUGH the casting front at that azimuth (~1.05), so the
            // museum radio panel with its three dials stood OUTSIDE the tank (the user's
            // "plate with three holes"). The 10-RT lives against the turret wall, inside.
            obb([-0.64, 0.38, 0.80], [0.27, 0.22, 0.20], 0.0),
            28,
            1.4,
        ),
        // Five ready rounds clipped crosswise into the 1951 turret rear. They are turret stowage,
        // not hull stowage: the volume swings with the live turret yaw, so a rear-turret
        // penetration meets ammunition exactly where the period loadout put it.
        // Raised and compacted (model-logic audit follow-up): the old 0.78 m ladder reached
        // 0.23 m BELOW the ring plane — brass visibly poking out from under the casting skirt
        // onto the deck in raised rear views (fits_within is hitbox-blind, the same class as
        // the bow rack #9 and the radio #13). The clips sit against the bustle wall, wholly
        // inside the casting between skirt and roof.
        turret_component(
            6,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([0.0, 0.86, -0.62], [0.36, 0.26, 0.12], 0.0),
            32,
            1.35,
        ),
    ]
}

fn hull_components() -> Vec<DamageComponent> {
    vec![
        // The twenty-round 4x5 skeleton rack in the bow, to the loader's side of the driver.
        // T-54 main stowage lives here, on the RIGHT — there is deliberately no rack on the
        // left hull; that side holds the driver and the radio operator's legacy space.
        hull_component(
            5,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            // Centre lowered 12 cm (model-logic audit #9): at 0.0 the rack topped out at a
            // world 1.67 m — 9 cm PROUD of the 1.58 m foredeck, so the museum rounds hanging
            // off it lay visibly on the glacis. The real 20-round rack stands on the hull
            // floor and stops under the deck; -0.12 puts the top at 1.55, a plate under it.
            obb([0.65, -0.12, 1.20], [0.37, 0.48, 0.36], 0.0),
            32,
            1.35,
        ),
        // Four shin-level rounds clipped low along the loader's hull wall. (The mask that once
        // capped this at id 16 is a u32 as of protocol v27 — ids 17+ own real bits now, so the
        // remaining loader-wall clips and the bulkhead round are free to become damage
        // components whenever the interior program wants them.)
        hull_component(
            16,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            obb([0.88, -0.335, 0.0], [0.10, 0.18, 0.69], 0.0),
            32,
            1.35,
        ),
        hull_component(
            8,
            K::FuelTank,
            ModuleSlot::Engine,
            M::Fuel,
            obb([-0.79, 0.03, -1.43], [0.25, 0.43, 0.65], 0.0),
            25,
            1.1,
        ),
        hull_component(
            9,
            K::FuelTank,
            ModuleSlot::Engine,
            M::Fuel,
            obb([0.79, 0.03, -1.43], [0.25, 0.43, 0.65], 0.0),
            25,
            1.1,
        ),
        hull_component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            // Roof pulled under the deck plane (model-logic follow-up): at half-y 0.48 the
            // block topped 7 cm PROUD of the 1.58 m deck, and the interior parts anchored to
            // it (intake spine, radiators) poked visible primer through the louvres.
            obb([0.0, -0.02, -1.90], [0.63, 0.42, 0.58], 0.0),
            30,
            0.9,
        ),
        hull_component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            obb([0.0, -0.02, -2.65], [0.68, 0.42, 0.30], 0.0),
            30,
            0.9,
        ),
        hull_component(
            12,
            K::FinalDrive,
            ModuleSlot::Suspension,
            M::Driveline,
            cylinder_shape([-1.08, -0.34, -2.77], Vec3::X, 0.18, 0.24),
            28,
            1.0,
        ),
        hull_component(
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
        frame: ArmorFrame::Hull,
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

fn turret_component(
    id: u16,
    kind: K,
    slot: ModuleSlot,
    material: M,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    component(id, ArmorFrame::Turret, kind, slot, material, shape, priority, vulnerability)
}

fn hull_component(
    id: u16,
    kind: K,
    slot: ModuleSlot,
    material: M,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    component(id, ArmorFrame::Hull, kind, slot, material, shape, priority, vulnerability)
}

#[allow(clippy::too_many_arguments)]
fn component(
    id: u16,
    frame: ArmorFrame,
    kind: K,
    slot: ModuleSlot,
    material: M,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    DamageComponent {
        id: DamageComponentId(id),
        frame,
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
