use game_core::VehicleBlueprint;
use glam::Vec3;
use vehicle_geometry::MaterialRole;

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part, turret_material};

/// The KV-1's bespoke part table, derived entirely from its blueprint: the stepped bow over a
/// superstructure NARROWER than the width over tracks, the squared fender shelves bridging out
/// to the track face, the even six-station run under three return rollers, the slab-sided cast
/// loaf, and the short ZiS-5 that barely clears the nose. Every extent restates a blueprint
/// field, so the parts cannot drift from the hitbox/mount/armour source of truth.
pub(crate) fn kv1_parts(bp: &VehicleBlueprint) -> Vec<ForgePart> {
    let h = &bp.hull;
    let t = &bp.track;
    let tu = &bp.turret;
    let g = &bp.gun;

    let track_mid_y = 0.5 * (t.top_y + t.bottom_y);
    let wheel_top = track_mid_y + t.wheel_radius;
    let wheel_bottom = track_mid_y - t.wheel_radius;
    let band_min_z = -(t.end_z + t.end_radius);
    let band_max_z = t.end_z + t.end_radius;
    let roller_y = t.top_y - t.roller_radius;
    let half_run = 0.5 * (t.wheel_last_z - t.wheel_first_z);
    let wheel_axis_x = 0.5 * (t.inner_x + t.outer_x);
    // The shelf sits just clear of the top run and bridges hull side -> track face.
    let fender_y = t.top_y + 0.04;

    vec![
        part(
            ForgePartKind::Hull,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::ZERO,
            Vec3::new(-h.half_width, h.belly_y, -h.half_len),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "Blueprint hull shape: a tall box of thick plate, its armoured sides standing INSIDE \
             the width over tracks so the fender shelves can bridge out to the belts.",
        ),
        part(
            ForgePartKind::UpperGlacis,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.sponson_y + h.deck_y), h.half_len - 0.20),
            Vec3::new(-h.half_width, h.sponson_y, h.half_len - 0.55),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            format!(
                "Blueprint bow at {:.0}° from vertical: the real KV's three plates averaged into \
                 one, carrying the driver's vision port, the radio operator's DT ball and the \
                 spare-track rack ON the armour plane.",
                h.glacis_slope_deg
            ),
        ),
        part(
            ForgePartKind::LowerPlate,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.belly_y + h.sponson_y), h.half_len - 0.10),
            Vec3::new(-h.lower_half_width, h.belly_y, h.half_len - 0.40),
            Vec3::new(h.lower_half_width, h.sponson_y, h.half_len),
            "Blueprint lower nose: the derived plate raking back from the fold to the belly, with \
             the tow hooks on it.",
        ),
        part(
            ForgePartKind::Fenders,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, fender_y, 0.0),
            Vec3::new(-t.outer_x, fender_y - 0.02, -h.half_len),
            Vec3::new(t.outer_x, fender_y + 0.30, h.half_len),
            "KV squared track guards: flat full-length shelves running the whole hull, square-ended \
             — not a stepped T-34 fender or a hung Centurion skirt — carrying two stowage boxes and \
             a tow cable a side.",
        ),
        part(
            ForgePartKind::TrackRun,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, band_min_z),
            Vec3::new(t.outer_x, t.top_y, band_max_z),
            "Blueprint track belt: the run wrapped around a front idler and a REAR drive sprocket.",
        ),
        part(
            ForgePartKind::TrackBelt,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, band_min_z),
            Vec3::new(t.outer_x, t.top_y, band_max_z),
            format!(
                "The KV's 700 mm band — the widest in the game: {} heavy cast shoes a side, each \
                 with ONE stout centre guide horn and a deep transverse grouser rib.",
                t.link_count.unwrap_or(0)
            ),
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-wheel_axis_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(wheel_axis_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "Blueprint running gear: {} small 600 mm wheels per side at an EVEN pitch — no \
                 Christie gap, no T-54 stagger.",
                t.wheel_count
            ),
        ),
        part(
            ForgePartKind::RoadWheelSet,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.outer_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!(
                "KV road wheel set: {} openwork {}-spoke discs on torsion bars, each with its own \
                 trailing arm sized to this wheel rather than the T-54's.",
                t.wheel_count, t.wheel_spokes
            ),
        ),
        part(
            ForgePartKind::Idler,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, t.end_y, t.end_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, t.end_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, t.end_z + t.end_radius),
            "KV front idler: a SPOKED casting matching the road wheels, not the fleet's shared \
             smooth drum.",
        ),
        part(
            ForgePartKind::DriveSprocket,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, t.end_y, -t.end_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, -t.end_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, -t.end_z + t.end_radius),
            "Rear drive sprocket: Soviet convention, the toothed wheel at the tail.",
        ),
        part(
            ForgePartKind::ReturnRollers,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, roller_y, 0.0),
            Vec3::new(-t.outer_x, roller_y - t.roller_radius, -half_run * 0.72 - t.roller_radius),
            Vec3::new(t.outer_x, t.top_y, half_run * 0.72 + t.roller_radius),
            format!(
                "{} return rollers a side carrying the top run TAUT — the KV's third form rule, \
                 and what separates its flank from the wheel-riding T-34 and T-54.",
                t.return_rollers
            ),
        ),
        part(
            ForgePartKind::Turret,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(0.0, tu.ring_y, tu.ring_z),
            Vec3::new(-tu.plan_half_width, tu.ring_y, tu.ring_z - tu.plan_half_length),
            Vec3::new(tu.plan_half_width, tu.roof_y, tu.ring_z + tu.plan_half_length),
            format!(
                "The mod-1942 casting: a slab-sided LOAF {:.2} m long over {:.2} m wide, walls at \
                 {:.0}° and a broad flat roof. Baked as a PRISM, not a dome — a sector sweep on a \
                 circle would leave its front and rear planes inside the visible metal.",
                tu.plan_half_length * 2.0,
                tu.plan_half_width * 2.0,
                tu.side_slope_deg
            ),
        ),
        part(
            ForgePartKind::MantletSocket,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, tu.mantlet_front_z),
            Vec3::new(
                -tu.mantlet_radius * 1.10,
                g.trunnion_y - tu.mantlet_radius * 1.40,
                tu.mantlet_back_z,
            ),
            Vec3::new(
                tu.mantlet_radius * 1.10,
                g.trunnion_y + tu.mantlet_radius * 1.40,
                tu.mantlet_front_z,
            ),
            "The fixed socket the ZiS-5's mask seats in — TALLER than it is wide, the opposite \
             proportion to the T-54's flat oval.",
        ),
        part(
            ForgePartKind::MovingMantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(
                -tu.mantlet_radius * 1.10,
                g.trunnion_y - tu.mantlet_radius * 1.40,
                tu.mantlet_back_z,
            ),
            Vec3::new(
                tu.mantlet_radius * 1.10,
                g.trunnion_y + tu.mantlet_radius * 1.40,
                tu.mantlet_front_z,
            ),
            "The cast mask itself, carrying the same tall-narrow proportion as its socket so it \
             cannot walk off the seat as the gun elevates.",
        ),
        part(
            ForgePartKind::Gun,
            PartAnchor::GunTrunnion,
            MaterialRole::BarrelSteel,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-g.barrel_radius, g.trunnion_y - g.barrel_radius, g.trunnion_z),
            Vec3::new(g.barrel_radius, g.trunnion_y + g.barrel_radius, g.muzzle_z),
            format!(
                "The 76 mm ZiS-5: a clean tube with no brake and no evacuator, reaching only \
                 {:.2} m past the bow — the least overhang in the fleet by a factor of five.",
                g.muzzle_z - h.half_len
            ),
        ),
        part(
            ForgePartKind::Cupola,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(tu.cupola_x, tu.roof_y, tu.cupola_z),
            Vec3::new(tu.cupola_x - tu.cupola_radius, tu.roof_y, tu.cupola_z - tu.cupola_radius),
            Vec3::new(
                -tu.cupola_x + tu.cupola_radius,
                tu.roof_y + 0.12,
                tu.cupola_z + tu.cupola_radius,
            ),
            "NOT a cupola: twin flush roof hatches with the commander's periscope between them. \
             A drum here would make it a KV-1S, which this vehicle deliberately is not.",
        ),
        part(
            ForgePartKind::EngineDeck,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.deck_y + 0.03, -0.5 * h.half_len),
            Vec3::new(-h.lower_half_width, h.deck_y, -h.half_len + 0.30),
            Vec3::new(h.lower_half_width, h.deck_y + 0.12, -1.10),
            "The V-2K's deck: transverse Soviet louvre strips with the twin exhaust ports on the \
             sloped rear plate below them.",
        ),
    ]
}
