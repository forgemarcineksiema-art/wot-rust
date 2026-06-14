//! T-54/T-55-family part derivation: turn the flat [`VehicleBlueprint`] numbers into semantic
//! parts. Every extent here is read from the blueprint, so the graph stays a faithful restatement
//! of the single shape source rather than a second set of magic values.

use game_core::{MountFrames, VehicleBlueprint};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, MeshBounds};

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part, turret_material};

/// Build a coarse part graph from a vehicle's baked submesh bounds plus its running-gear count.
/// Used for families that have a reference pack but not yet a full blueprint (the German line).
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

pub(crate) fn t54_family_parts(bp: &VehicleBlueprint) -> Vec<ForgePart> {
    let h = &bp.hull;
    let t = &bp.track;
    let tu = &bp.turret;
    let g = &bp.gun;

    let track_mid_y = 0.5 * (t.top_y + t.bottom_y);
    let wheel_top = track_mid_y + t.wheel_radius;
    let wheel_bottom = track_mid_y - t.wheel_radius;

    vec![
        part(
            ForgePartKind::Hull,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::ZERO,
            Vec3::new(-h.half_width, h.belly_y, -h.half_len),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "Blueprint hull shape: lower tub stepping out to the wide sponson over the tracks.",
        ),
        part(
            ForgePartKind::TrackRun,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, -(t.wheel_last_z.abs() + t.end_radius)),
            Vec3::new(t.outer_x, t.top_y, t.wheel_last_z.abs() + t.end_radius),
            "Blueprint track belt: top/bottom runs wrapped around rounded ends.",
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.center_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.center_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "Blueprint running gear: {} road wheels per side — the family's strong side cue.",
                t.wheel_count
            ),
        ),
        part(
            ForgePartKind::Turret,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(0.0, tu.ring_y, tu.ring_z),
            Vec3::new(-tu.base_radius, tu.ring_y, tu.ring_z - tu.plan_half_length),
            Vec3::new(tu.base_radius, tu.roof_y, tu.ring_z + tu.plan_half_length),
            "Blueprint turret shell: rounded cast dome on the turret ring.",
        ),
        part(
            ForgePartKind::Mantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-tu.mantlet_radius, g.trunnion_y - tu.mantlet_radius, tu.mantlet_back_z),
            Vec3::new(tu.mantlet_radius, g.trunnion_y + tu.mantlet_radius, tu.mantlet_front_z),
            "Blueprint mantlet socket: cast mask shared between turret front and gun.",
        ),
        part(
            ForgePartKind::Gun,
            PartAnchor::GunTrunnion,
            MaterialRole::BarrelSteel,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-g.barrel_radius, g.trunnion_y - g.barrel_radius, g.trunnion_z),
            Vec3::new(g.barrel_radius, g.trunnion_y + g.barrel_radius, g.muzzle_z),
            "Blueprint D-10 barrel: trunnion to muzzle along the bore axis.",
        ),
        part(
            ForgePartKind::Cupola,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(tu.cupola_x, tu.roof_y, tu.cupola_z),
            Vec3::new(tu.cupola_x - tu.cupola_radius, tu.roof_y, tu.cupola_z - tu.cupola_radius),
            Vec3::new(
                tu.cupola_x + tu.cupola_radius,
                tu.roof_y + 0.5 * tu.cupola_radius,
                tu.cupola_z + tu.cupola_radius,
            ),
            "Blueprint commander's cupola seated on the turret roof.",
        ),
    ]
}
