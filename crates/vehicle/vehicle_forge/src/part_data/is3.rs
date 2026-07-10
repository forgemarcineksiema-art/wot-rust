use game_core::VehicleBlueprint;
use glam::Vec3;
use vehicle_geometry::MaterialRole;

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part, turret_material};

/// The IS-3's bespoke part table, derived entirely from its blueprint: the pike bow pair, the
/// small-wheel six-station run with its three return rollers, the overhanging turtle dome, the
/// braked 122, and the rear-fender fuel drums — every extent restates a blueprint field, so
/// the parts cannot drift from the hitbox/mount/armor source of truth.
pub(crate) fn is3_parts(bp: &VehicleBlueprint) -> Vec<ForgePart> {
    let h = &bp.hull;
    let t = &bp.track;
    let tu = &bp.turret;
    let g = &bp.gun;

    let track_mid_y = 0.5 * (t.top_y + t.bottom_y);
    let wheel_top = track_mid_y + t.wheel_radius;
    let wheel_bottom = track_mid_y - t.wheel_radius;
    let band_min_z = -(t.end_z + t.end_radius);
    let band_max_z = t.end_z + t.end_radius;

    // The pike's plane geometry, reproduced from the armor volumes' construction: the upper
    // faces pivot at the sponson-step fold and sweep ±pike_sweep_deg in plan, so the bow's
    // rearmost reach is where a swept face meets the deck at the full hull width.
    let sweep = h.pike_sweep_deg.to_radians();
    let glacis = bp.armor.hull_front.0.to_radians();
    let upper_n = Vec3::new(sweep.sin() * glacis.cos(), glacis.sin(), sweep.cos() * glacis.cos());
    let fold = Vec3::new(0.0, h.sponson_y, h.half_len);
    let deck_corner_z =
        (upper_n.dot(fold) - upper_n.x * h.half_width - upper_n.y * h.deck_y) / upper_n.z;
    let lower = (bp.armor.hull_front.0 * 0.45).to_radians();
    let lower_n = Vec3::new(sweep.sin() * lower.cos(), -lower.sin(), sweep.cos() * lower.cos());
    let belly_corner_z =
        (lower_n.dot(fold) - lower_n.x * h.lower_half_width + lower_n.y * h.belly_y) / lower_n.z;

    // The fender shelf and the fuel drums, matching the recipe's construction lines.
    let fender_y = h.sponson_y + 0.015;
    let drum_radius = 0.145;
    let drum_axis_x = 0.5 * (t.inner_x + t.outer_x);
    let drum_axis_y = h.sponson_y + 0.05 + drum_radius;
    let roller_y = t.top_y - t.roller_radius;
    let half_run = 0.5 * (t.wheel_last_z - t.wheel_first_z);

    vec![
        part(
            ForgePartKind::Hull,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::ZERO,
            Vec3::new(-h.half_width, h.belly_y, -h.half_len),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "Blueprint hull shape: sponsons over the tracks, narrow tub between the 650 mm belts.",
        ),
        part(
            ForgePartKind::PikeBow,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.sponson_y + h.deck_y), 0.5 * (deck_corner_z + h.half_len)),
            Vec3::new(-h.half_width, h.sponson_y, deck_corner_z),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            format!(
                "Blueprint pike bow: two upper plates swept ±{}° about the central ridge — the \
                 shchuchy nos the armor volumes shoot against.",
                h.pike_sweep_deg
            ),
        ),
        part(
            ForgePartKind::LowerPlate,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.belly_y + h.sponson_y), 0.5 * (belly_corner_z + h.half_len)),
            Vec3::new(-h.lower_half_width, h.belly_y, belly_corner_z),
            Vec3::new(h.lower_half_width, h.sponson_y, h.half_len),
            "Blueprint lower pike faces: the derived lower-plate slope carrying the ±38° sweep \
             down to the belly.",
        ),
        part(
            ForgePartKind::Fenders,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, fender_y, 0.0),
            Vec3::new(-t.outer_x, h.sponson_y, -h.half_len + 0.15),
            Vec3::new(t.outer_x, fender_y + 0.035, t.end_z + t.end_radius + 0.20),
            "IS-3 fender line: full-length shelf, sloping front mudguards over the idlers, rear \
             flaps over the sprockets.",
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
            "IS-3 continuous 650 mm belt: repeated shoes carried on the return rollers.",
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-drum_axis_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(drum_axis_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!("Blueprint running gear: {} small 550 mm wheels per side.", t.wheel_count),
        ),
        part(
            ForgePartKind::RoadWheelSet,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.outer_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "IS-3 road wheel set: {} narrow twelve-rib cast discs per side.",
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
            "IS-3 front idler on its raised axle, distinct from the road-wheel set.",
        ),
        part(
            ForgePartKind::DriveSprocket,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, t.end_y, -t.end_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, -t.end_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, -t.end_z + t.end_radius),
            "IS-3 rear drive sprocket on its raised axle.",
        ),
        part(
            ForgePartKind::ReturnRollers,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, roller_y, 0.0),
            Vec3::new(-t.outer_x, roller_y - t.roller_radius, -half_run * 0.72 - t.roller_radius),
            Vec3::new(t.outer_x, t.top_y, half_run * 0.72 + t.roller_radius),
            format!(
                "IS family top-run carriers: {} small return rollers per side — the heavy's \
                 look against the wheel-riding mediums.",
                t.return_rollers
            ),
        ),
        part(
            ForgePartKind::Turret,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(0.0, tu.ring_y, tu.ring_z),
            Vec3::new(-tu.base_radius, tu.ring_y, tu.ring_z - tu.plan_half_length),
            Vec3::new(tu.base_radius, tu.roof_y, tu.ring_z + tu.plan_half_length),
            "Blueprint turret shell: the flattened cast dome overhanging its 1.8 m ring.",
        ),
        part(
            ForgePartKind::Mantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-tu.mantlet_radius, g.trunnion_y - tu.mantlet_radius, tu.mantlet_back_z),
            Vec3::new(tu.mantlet_radius, g.trunnion_y + tu.mantlet_radius, tu.mantlet_front_z),
            "Blueprint mantlet ball where the 122 meets the dome's front sectors.",
        ),
        part(
            ForgePartKind::Gun,
            PartAnchor::GunTrunnion,
            MaterialRole::BarrelSteel,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-g.barrel_radius, g.trunnion_y - g.barrel_radius, g.trunnion_z),
            Vec3::new(g.barrel_radius, g.trunnion_y + g.barrel_radius, g.muzzle_z),
            "Blueprint D-25T barrel: trunnion to the double-baffle muzzle at z = 6.46.",
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
            "Blueprint commander's cupola, low on the flattened dome roof.",
        ),
        part(
            ForgePartKind::EngineDeck,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.deck_y + 0.03, -0.5 * h.half_len),
            Vec3::new(-h.lower_half_width, h.deck_y, -h.half_len + 0.35),
            Vec3::new(h.lower_half_width, h.deck_y + 0.12, -1.15),
            "IS-3 stepped rear engine deck with raised access panels.",
        ),
        part(
            ForgePartKind::FuelDrums,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, drum_axis_y, -h.half_len + 0.75),
            Vec3::new(-(drum_axis_x + drum_radius), drum_axis_y - drum_radius, -h.half_len + 0.35),
            Vec3::new(drum_axis_x + drum_radius, drum_axis_y + drum_radius, -h.half_len + 1.15),
            "IS family external fuel drums lying along the rear fender shelves — cosmetic \
             stowage the armor volumes rightly ignore.",
        ),
    ]
}
