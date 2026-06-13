//! T-54/T-55-family part derivation: turn the flat [`VehicleBlueprint`] numbers into semantic
//! parts. Every extent here is read from the blueprint, so the graph stays a faithful restatement
//! of the single shape source rather than a second set of magic values.

use game_core::VehicleBlueprint;
use glam::Vec3;
use vehicle_geometry::MaterialRole;

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part, turret_material};

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
