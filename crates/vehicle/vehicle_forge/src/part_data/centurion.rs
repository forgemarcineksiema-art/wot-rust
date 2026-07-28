use game_core::VehicleBlueprint;
use glam::Vec3;
use vehicle_geometry::MaterialRole;

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part, turret_material};

/// The Centurion Mk 3's bespoke part table, derived entirely from its blueprint: the ramp
/// glacis, the full-length bazooka SKIRTS (gameplay-real spaced armor, standing on the exact
/// plane `ArmorZone::Skirt` resolves on), the three Horstmann bogie pairs with their return
/// rollers under them, the cast Mk 3 dome with the bustle stowage bin, and the clean unbraked
/// 20-pounder — every extent restates a blueprint field.
pub(crate) fn centurion_parts(bp: &VehicleBlueprint) -> Vec<ForgePart> {
    let h = &bp.hull;
    let t = &bp.track;
    let tu = &bp.turret;
    let g = &bp.gun;
    let skirt = h.skirt.expect("the Centurion authors a skirt");

    let track_mid_y = 0.5 * (t.top_y + t.bottom_y);
    // The wheels hang on the AXLE line, which the rollered layout puts below the belt's middle.
    let axle_y = t.axle_y();
    let wheel_top = axle_y + t.wheel_radius;
    let wheel_bottom = axle_y - t.wheel_radius;
    let band_min_z = -(t.end_z + t.end_radius);
    let band_max_z = t.end_z + t.end_radius;
    let glacis_run = (h.deck_y - h.sponson_y).max(0.1) * h.glacis_slope_deg.to_radians().tan();
    let upper_front_z = h.half_len - glacis_run;
    let lower_run =
        (h.sponson_y - h.belly_y).max(0.1) * (h.glacis_slope_deg * 0.45).to_radians().tan();
    let skirt_outer_x = t.outer_x + skirt.standoff_m + skirt.thickness_m;
    let roller_y = t.top_y - t.roller_radius;
    let half_run = 0.5 * (t.wheel_last_z - t.wheel_first_z);
    let wheel_center_x = 0.5 * (t.inner_x + t.outer_x);
    let bin_rear_z = tu.ring_z - tu.plan_half_length;

    vec![
        part(
            ForgePartKind::Hull,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::ZERO,
            Vec3::new(-h.half_width, h.belly_y, -h.half_len),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "Blueprint hull shape: long sponson body over the narrow tub, behind the skirts.",
        ),
        part(
            ForgePartKind::UpperGlacis,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.sponson_y + h.deck_y), 0.5 * (upper_front_z + h.half_len)),
            Vec3::new(-h.half_width, h.sponson_y, upper_front_z),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            format!(
                "Blueprint ramp glacis: 76 mm at {}° from the fold to the deck — out-sloping \
                 every German plate.",
                bp.armor.hull_front.0
            ),
        ),
        part(
            ForgePartKind::LowerPlate,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.belly_y + h.sponson_y), h.half_len - 0.5 * lower_run),
            Vec3::new(-h.lower_half_width, h.belly_y, h.half_len - lower_run),
            Vec3::new(h.lower_half_width, h.sponson_y, h.half_len),
            "Blueprint lower nose below the ramp, raking back to the belly.",
        ),
        part(
            ForgePartKind::Skirts,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (skirt.top_y + skirt.bottom_y), 0.0),
            Vec3::new(-skirt_outer_x, skirt.bottom_y, skirt.rear_z),
            Vec3::new(skirt_outer_x, skirt.top_y, skirt.front_z),
            "Blueprint full-length bazooka plates: gameplay-real spaced armor — the visible \
             sheet stands on the exact plane ArmorZone::Skirt resolves on, hiding the upper \
             half of the running gear.",
        ),
        part(
            ForgePartKind::TrackRun,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, band_min_z),
            Vec3::new(t.outer_x, t.top_y, band_max_z),
            "Blueprint track belt: top/bottom runs wrapped around the raised idler and sprocket.",
        ),
        part(
            ForgePartKind::TrackBelt,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, band_min_z),
            Vec3::new(t.outer_x, t.top_y, band_max_z),
            "Centurion continuous 610 mm belt: repeated shoes carried on the return rollers.",
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, axle_y, 0.0),
            Vec3::new(-wheel_center_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(wheel_center_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "Blueprint running gear: {} wheels per side in three Horstmann bogie PAIRS — \
                 tight in-pair pitch, a real gap between bogies.",
                t.wheel_count
            ),
        ),
        part(
            ForgePartKind::RoadWheelSet,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, axle_y, 0.0),
            Vec3::new(-t.outer_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.outer_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "Centurion road wheel set: {} rubber-tyred 610 mm wheels per side on bogie arms.",
                t.wheel_count
            ),
        ),
        part(
            ForgePartKind::Idler,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, t.end_y, t.end_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, t.end_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, t.end_z + t.end_radius),
            "Centurion front idler on its raised axle, distinct from the bogie wheels.",
        ),
        part(
            ForgePartKind::DriveSprocket,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, t.end_y, -t.end_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, -t.end_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, -t.end_z + t.end_radius),
            "Centurion rear drive sprocket on its raised axle.",
        ),
        part(
            ForgePartKind::ReturnRollers,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, roller_y, 0.0),
            Vec3::new(-t.outer_x, roller_y - t.roller_radius, -half_run * 0.72 - t.roller_radius),
            Vec3::new(t.outer_x, t.top_y, half_run * 0.72 + t.roller_radius),
            format!(
                "Centurion top-run carriers: {} small return rollers per side over the bogie \
                 gaps.",
                t.return_rollers
            ),
        ),
        part(
            ForgePartKind::Turret,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(0.0, tu.ring_y, tu.ring_z),
            Vec3::new(-tu.plan_half_width, tu.ring_y, bin_rear_z),
            Vec3::new(tu.plan_half_width, tu.roof_y, tu.ring_z + tu.plan_half_length),
            "Blueprint turret shell: the cast Mk 3 dome on the 74-inch race.",
        ),
        part(
            ForgePartKind::StowageBin,
            PartAnchor::TurretRing,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, tu.ring_y + 0.44, bin_rear_z + 0.16),
            Vec3::new(-0.75, tu.ring_y + 0.22, bin_rear_z + 0.01),
            Vec3::new(0.75, tu.ring_y + 0.66, bin_rear_z + 0.31),
            "Centurion bustle stowage bin closing the rear of the turret plan — cosmetic \
             stowage, matching the recipe's construction.",
        ),
        part(
            ForgePartKind::Mantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-tu.mantlet_radius, g.trunnion_y - tu.mantlet_radius, tu.mantlet_back_z),
            Vec3::new(tu.mantlet_radius, g.trunnion_y + tu.mantlet_radius, tu.mantlet_front_z),
            "Blueprint mantlet where the 20-pounder meets the dome's front sectors.",
        ),
        part(
            ForgePartKind::Gun,
            PartAnchor::GunTrunnion,
            MaterialRole::BarrelSteel,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-g.barrel_radius, g.trunnion_y - g.barrel_radius, g.trunnion_z),
            Vec3::new(g.barrel_radius, g.trunnion_y + g.barrel_radius, g.muzzle_z),
            "Blueprint 20-pounder Type A barrel: a clean tube to the muzzle at z = 6.03 — no \
             brake, no fume extractor.",
        ),
        part(
            ForgePartKind::Cupola,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(tu.cupola_x, tu.roof_y, tu.cupola_z),
            Vec3::new(tu.cupola_x - tu.cupola_radius, tu.roof_y, tu.cupola_z - tu.cupola_radius),
            Vec3::new(
                tu.cupola_x + tu.cupola_radius,
                tu.roof_y + 0.4 * tu.cupola_radius,
                tu.cupola_z + tu.cupola_radius,
            ),
            "Blueprint commander's cupola on the left rear roof.",
        ),
        part(
            ForgePartKind::EngineDeck,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.deck_y + 0.03, -0.5 * h.half_len),
            Vec3::new(-h.lower_half_width, h.deck_y, -h.half_len + 0.35),
            Vec3::new(h.lower_half_width, h.deck_y + 0.12, -1.15),
            "Centurion rear engine deck with raised access panels (Meteor bay).",
        ),
    ]
}
