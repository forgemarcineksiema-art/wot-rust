//! Semantic part derivation helpers.

mod centurion;
mod is3;
mod t54;
mod tiger_i;

pub(crate) use centurion::centurion_parts;
pub(crate) use is3::is3_parts;
pub(crate) use t54::t54_family_parts;
pub(crate) use tiger_i::tiger_i_parts;

use game_core::MountFrames;
use glam::Vec3;
use vehicle_geometry::{MaterialRole, MeshBounds};

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part};

/// Build a coarse part graph from baked submesh bounds plus the reference running-gear count.
pub(crate) fn geometry_derived_parts(
    wheel_count: usize,
    traverses: bool,
    hull: MeshBounds,
    turret: MeshBounds,
    gun: MeshBounds,
    mounts: &MountFrames,
) -> Vec<ForgePart> {
    let lower_top = hull.min.y + 0.45 * (hull.max.y - hull.min.y);
    let lower_mid = 0.5 * (hull.min.y + lower_top);
    let inset = 0.92;
    let trun = mounts.gun_trunnion.translation;
    let m = 0.30;
    let cx = turret.min.x * 0.4;
    let cz = turret.min.z * 0.3;

    vec![
        part(
            ForgePartKind::Hull,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::ZERO,
            hull.min,
            hull.max,
            "Derived from baked hull bounds: rolled-armour hull tub and sponsons.",
        ),
        part(
            ForgePartKind::TrackRun,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, lower_mid, 0.0),
            Vec3::new(hull.min.x, hull.min.y, hull.min.z),
            Vec3::new(hull.max.x, lower_top, hull.max.z),
            "Derived from baked geometry: track belt wrapping the lower hull.",
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, lower_mid, 0.0),
            Vec3::new(hull.min.x * inset, hull.min.y, hull.min.z),
            Vec3::new(hull.max.x * inset, lower_top, hull.max.z),
            format!("Reference pack running gear: {wheel_count} road wheels per side."),
        ),
        part(
            ForgePartKind::Turret,
            PartAnchor::TurretRing,
            MaterialRole::RolledArmor,
            mounts.turret_ring.translation,
            turret.min,
            turret.max,
            if traverses {
                "Derived from baked turret bounds: welded turret shell on the ring."
            } else {
                "Derived from baked bounds: fixed casemate superstructure (no traverse)."
            },
        ),
        part(
            ForgePartKind::Mantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            trun,
            Vec3::new(-m, trun.y - m, turret.max.z - 2.0 * m),
            Vec3::new(m, trun.y + m, turret.max.z),
            "Derived from baked bounds: cast mantlet mask at the gun trunnion.",
        ),
        part(
            ForgePartKind::Gun,
            PartAnchor::GunTrunnion,
            MaterialRole::BarrelSteel,
            trun,
            Vec3::new(gun.min.x, trun.y - (gun.max.x - gun.min.x) * 0.5, trun.z),
            Vec3::new(gun.max.x, trun.y + (gun.max.x - gun.min.x) * 0.5, gun.max.z),
            "Derived from baked gun bounds: barrel from trunnion to muzzle.",
        ),
        part(
            ForgePartKind::Cupola,
            PartAnchor::TurretRing,
            MaterialRole::RolledArmor,
            Vec3::new(cx, turret.max.y, cz),
            Vec3::new(cx - 0.18, turret.max.y - 0.12, cz - 0.18),
            Vec3::new(cx + 0.18, turret.max.y + 0.12, cz + 0.18),
            "Derived from baked bounds: commander's cupola on the turret/casemate roof.",
        ),
    ]
}
