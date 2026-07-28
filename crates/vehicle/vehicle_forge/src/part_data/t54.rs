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
    let track_min_z = t.wheel_first_z - t.end_radius;
    let track_max_z = t.wheel_last_z + t.end_radius;
    let glacis_run =
        (h.deck_y - h.belly_y - h.nose_rise).max(0.1) * h.glacis_slope_deg.to_radians().tan();
    let upper_front_z = h.half_len - glacis_run;

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
            ForgePartKind::UpperGlacis,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.deck_y + h.sponson_y), 0.5 * (h.half_len + upper_front_z)),
            Vec3::new(-h.half_width, h.sponson_y, upper_front_z),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "Named T-54 hull plate: steep upper glacis defining the low front wedge.",
        ),
        part(
            ForgePartKind::LowerPlate,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.belly_y + h.sponson_y), h.half_len - 0.24),
            Vec3::new(-h.lower_half_width, h.belly_y, h.half_len - 0.48),
            Vec3::new(h.lower_half_width, h.sponson_y, h.half_len),
            "Named T-54 lower nose plate below the upper glacis.",
        ),
        part(
            ForgePartKind::Fenders,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.sponson_y + 0.08, 0.0),
            Vec3::new(-h.hitbox_half_width, h.sponson_y, -h.half_len),
            Vec3::new(h.hitbox_half_width, h.sponson_y + 0.16, h.half_len),
            "Named T-54 side fenders over the exposed track run.",
        ),
        part(
            ForgePartKind::TrackRun,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, track_min_z),
            Vec3::new(t.outer_x, t.top_y, track_max_z),
            "Blueprint track belt: top/bottom runs wrapped around rounded ends.",
        ),
        part(
            ForgePartKind::TrackBelt,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, track_min_z),
            Vec3::new(t.outer_x, t.top_y, track_max_z),
            "T-54 continuous track belt: repeated shoes around the road wheels, idler, and sprocket.",
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.center_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.center_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!("Blueprint running gear: {} road wheels per side.", t.wheel_count),
        ),
        part(
            ForgePartKind::RoadWheelSet,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.outer_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "T-54 road wheel set: {} large rubber-tyred wheels per side with hubs.",
                t.wheel_count
            ),
        ),
        part(
            ForgePartKind::Idler,
            PartAnchor::Hull,
            // Cast steel with STEEL tyres on the T-54 (unlike the rubber-tyred road wheels) —
            // the semantic part table said rubber, which is a material lie in the review data.
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, t.wheel_last_z),
            Vec3::new(-t.outer_x, track_mid_y - t.end_radius, t.wheel_last_z - t.end_radius),
            Vec3::new(t.outer_x, track_mid_y + t.end_radius, t.wheel_last_z + t.end_radius),
            "T-54 front idler, distinct from the road-wheel set.",
        ),
        part(
            ForgePartKind::DriveSprocket,
            PartAnchor::Hull,
            // Hadfield-steel tooth rings bolted to a steel disc — never rubber.
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, t.wheel_first_z),
            Vec3::new(-t.outer_x, track_mid_y - t.end_radius, t.wheel_first_z - t.end_radius),
            Vec3::new(t.outer_x, track_mid_y + t.end_radius, t.wheel_first_z + t.end_radius),
            "T-54 rear drive sprocket, distinct from the idler and road wheels.",
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
            ForgePartKind::TurretCheeks,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(0.0, g.trunnion_y, tu.mantlet_back_z),
            Vec3::new(-tu.base_radius, tu.ring_y + 0.12, tu.mantlet_back_z - 0.18),
            Vec3::new(tu.base_radius, tu.roof_y - 0.10, tu.ring_z + tu.plan_half_length),
            "T-54 cast front cheek mass, integrated with the turret shell around the mantlet socket.",
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
            ForgePartKind::MantletSocket,
            PartAnchor::TurretRing,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, tu.mantlet_back_z),
            Vec3::new(
                -tu.mantlet_radius * 1.35,
                g.trunnion_y - tu.mantlet_radius * 0.82,
                tu.mantlet_back_z - 0.04,
            ),
            Vec3::new(
                tu.mantlet_radius * 1.35,
                g.trunnion_y + tu.mantlet_radius * 0.82,
                tu.mantlet_front_z + 0.04,
            ),
            "T-54 recessed mantlet socket cut into the front cheek mass.",
        ),
        part(
            ForgePartKind::MovingMantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(
                -tu.mantlet_radius * 1.45,
                g.trunnion_y - tu.mantlet_radius * 0.62,
                tu.mantlet_back_z,
            ),
            Vec3::new(
                tu.mantlet_radius * 1.45,
                g.trunnion_y + tu.mantlet_radius * 0.62,
                tu.mantlet_front_z,
            ),
            "T-54 moving mantlet: wide flattened oval mask, not a round ball on the turret front.",
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
        part(
            ForgePartKind::EngineDeck,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.deck_y + 0.03, -0.5 * h.half_len),
            Vec3::new(-h.lower_half_width, h.deck_y, -h.half_len + 0.35),
            Vec3::new(h.lower_half_width, h.deck_y + 0.12, -1.15),
            "Named T-54 rear engine deck with raised access-panel surface detail.",
        ),
    ]
}
