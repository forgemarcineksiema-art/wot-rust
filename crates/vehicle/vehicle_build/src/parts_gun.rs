//! The gun group as library parts for any vehicle whose visual file authors a `GunVisual`
//! (Forge 2.0 K3, step 4c): the bore-honest barrel between the trunnion and the tube's end, the
//! moving mantlet on the trunnion, and the muzzle brake's baffles — three parts with their own
//! keys, materials and smoothing, from the mounts the blueprint derives. The recipe fleet drew
//! the same pieces merged into one submesh (`armament::authored_gun_group`); the library keeps
//! them apart so the inventory sees a barrel, a mantlet and the muzzle furniture.

use game_core::VehicleBlueprint;
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};

/// The gun group for `bp`, or `None` when its visual file authors no gun.
pub fn gun_parts_for_blueprint(bp: &VehicleBlueprint) -> Option<Vec<VehiclePart>> {
    let gun = bp.visual_detail()?.gun?;
    let mounts = bp.mount_frames();
    let trunnion = mounts.gun_trunnion.translation;
    let muzzle = mounts.muzzle.translation;
    let tube_end = match gun.muzzle_brake {
        Some(brake) => muzzle - Vec3::new(0.0, 0.0, brake.length),
        None => muzzle,
    };
    let mut parts = vec![
        VehiclePart {
            key: PartKey::new("gun_barrel"),
            submesh: SubmeshKind::Gun,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup(4),
            shape: PartShape::Mesh(revolve::gun_barrel_between(trunnion, tube_end, &gun)),
            lod: PartLod::MountCritical,
            generator: GeneratorKind::Revolve,
        },
        VehiclePart {
            key: PartKey::new("gun_mantlet"),
            submesh: SubmeshKind::Gun,
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup(2),
            shape: PartShape::Mesh(revolve::moving_mantlet(trunnion, &gun)),
            lod: PartLod::MountCritical,
            generator: GeneratorKind::Revolve,
        },
    ];
    if let Some(piece) = revolve::muzzle_brake(muzzle, &gun) {
        parts.push(VehiclePart {
            key: PartKey::new("muzzle_brake"),
            submesh: SubmeshKind::Gun,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup(4),
            shape: PartShape::Mesh(piece),
            lod: PartLod::Detail,
            generator: GeneratorKind::Revolve,
        });
    }
    Some(parts)
}
