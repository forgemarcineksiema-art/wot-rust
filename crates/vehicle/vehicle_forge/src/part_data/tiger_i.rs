use game_core::VehicleBlueprint;
use glam::Vec3;
use vehicle_geometry::MaterialRole;

use crate::part_graph::{ForgePart, ForgePartKind, PartAnchor, part, turret_material};

/// The Tiger I's bespoke part table, derived entirely from its blueprint: the face-honest slab
/// hull with its near-vertical 9° driver's plate, the interleaved eight-axle Schachtellaufwerk
/// (three visual planes, no return rollers), the welded horseshoe turret whose rear armor plane
/// IS the Rommelkiste face, the drum cupola on the left, and the braked KwK 36 — every extent
/// restates a blueprint field, so the parts cannot drift from the hitbox/mount/armor source.
/// (W1 PR-T1.3; dossier: docs/vehicles/panzerkampfwagen-vi-tiger.md.)
pub(crate) fn tiger_i_parts(bp: &VehicleBlueprint) -> Vec<ForgePart> {
    let h = &bp.hull;
    let t = &bp.track;
    let tu = &bp.turret;
    let g = &bp.gun;

    let track_mid_y = 0.5 * (t.top_y + t.bottom_y);
    let wheel_top = track_mid_y + t.wheel_radius;
    let wheel_bottom = track_mid_y - t.wheel_radius;
    let band_min_z = -(t.end_z + t.end_radius);
    let band_max_z = t.end_z + t.end_radius;
    // Which end each wheel sits at, straight off the blueprint's drive layout.
    let drive_z = if t.drive_front { t.end_z } else { -t.end_z };
    let idler_z = -drive_z;

    // The driver's plate stands glacis_slope_deg from vertical, pivoting at the sponson fold —
    // its run over the plate's height is what the glacis part spans in Z.
    let glacis_run = (h.deck_y - h.sponson_y).max(0.0) * h.glacis_slope_deg.to_radians().tan();

    vec![
        part(
            ForgePartKind::Hull,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::ZERO,
            Vec3::new(-h.half_width, h.belly_y, -h.half_len),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "Blueprint slab hull: full-width sponsons over the tracks, the deepest body in the \
             German line (0.47 m clearance to the 1.90 m deck).",
        ),
        part(
            ForgePartKind::UpperGlacis,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.sponson_y + h.deck_y), h.half_len - 0.5 * glacis_run),
            Vec3::new(-h.half_width, h.sponson_y, h.half_len - glacis_run),
            Vec3::new(h.half_width, h.deck_y, h.half_len),
            "The near-vertical 9° driver's plate — the visible face IS the armor glacis plane \
             (the Tiger's honesty lock pattern).",
        ),
        part(
            ForgePartKind::LowerPlate,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (h.belly_y + h.sponson_y), h.half_len - 0.1),
            Vec3::new(-h.lower_half_width, h.belly_y, h.half_len - 0.35),
            Vec3::new(h.lower_half_width, h.sponson_y, h.half_len),
            "Blueprint lower nose: the derived lower-plate slope down to the 0.47 m belly.",
        ),
        part(
            ForgePartKind::TrackRun,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, band_min_z),
            Vec3::new(t.outer_x, t.top_y, band_max_z),
            "Blueprint track band: the 725 mm combat belt whose outer face IS the documented \
             3.705 m width over tracks.",
        ),
        part(
            ForgePartKind::TrackBelt,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, t.bottom_y, band_min_z),
            Vec3::new(t.outer_x, t.top_y, band_max_z),
            "Tiger continuous belt: shoes fill the band edge to edge, sprocket teeth inside the \
             band (the honest-width contract).",
        ),
        part(
            ForgePartKind::RoadWheels,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.outer_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            format!("Blueprint running gear: {} interleaved 800 mm axles per side.", t.wheel_count),
        ),
        part(
            ForgePartKind::RoadWheelSet,
            PartAnchor::Hull,
            MaterialRole::Rubber,
            Vec3::new(0.0, track_mid_y, 0.0),
            Vec3::new(-t.outer_x, wheel_bottom, t.wheel_first_z - t.wheel_radius),
            Vec3::new(t.outer_x, wheel_top, t.wheel_last_z + t.wheel_radius),
            "Schachtellaufwerk stagger: even axles ride the outer row and odd axles the inner \
             row — two visual planes, one wheel unit per axle, no return rollers.",
        ),
        // Front drive (`drive_front`): the sprocket is the FORWARD end wheel and the idler the
        // rear one — the German line's layout, and the same end the mesh places them at. The
        // table used to carry the Soviet arrangement, so the labelled parts named the wrong
        // wheels while the model drove correctly (model-logic finding, Tiger I).
        part(
            ForgePartKind::DriveSprocket,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, t.end_y, drive_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, drive_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, drive_z + t.end_radius),
            "Front drive sprocket at the transmission end; its toothed rings stay inside the \
             band's outer face.",
        ),
        part(
            ForgePartKind::Idler,
            PartAnchor::Hull,
            MaterialRole::TrackMetal,
            Vec3::new(0.0, t.end_y, idler_z),
            Vec3::new(-t.outer_x, t.end_y - t.end_radius, idler_z - t.end_radius),
            Vec3::new(t.outer_x, t.end_y + t.end_radius, idler_z + t.end_radius),
            "Rear idler at the belt's trailing wrap.",
        ),
        part(
            ForgePartKind::Fenders,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.sponson_y + 0.03, 0.0),
            // The shelf runs wrap to wrap; the hinged flaps carry it past both ends and droop
            // below the fender line, so the part's box has to reach where they actually hang.
            Vec3::new(-t.outer_x, h.sponson_y - 0.25, -(t.end_z + 0.57)),
            Vec3::new(t.outer_x, h.sponson_y + 0.05, t.end_z + 0.57),
            "Track guards over the belt run with hinged flaps at both ends — the fender line the \
             3.705 m beam needs once the belts, not the sponsons, carry the width.",
        ),
        part(
            ForgePartKind::Turret,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(0.0, tu.ring_y, tu.ring_z),
            Vec3::new(-tu.plan_half_width, tu.ring_y, tu.ring_z - tu.plan_half_length),
            Vec3::new(tu.plan_half_width, tu.roof_y, tu.ring_z + tu.plan_half_length),
            "Blueprint welded horseshoe: a low broad band on the tall superstructure, 100 mm \
             all-round in the Ausf. E scheme.",
        ),
        part(
            ForgePartKind::StowageBin,
            PartAnchor::TurretRing,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, 0.5 * (tu.ring_y + tu.roof_y), tu.ring_z - tu.plan_half_length + 0.14),
            Vec3::new(-tu.plan_half_width * 0.8, tu.ring_y + 0.10, tu.ring_z - tu.plan_half_length),
            Vec3::new(
                tu.plan_half_width * 0.8,
                tu.roof_y - 0.06,
                tu.ring_z - tu.plan_half_length + 0.28,
            ),
            "The Rommelkiste: it CLOSES the turret's rear armor plane — a shell into the bin \
             resolves on the plate it stands on, not on cosmetic air.",
        ),
        part(
            ForgePartKind::Mantlet,
            PartAnchor::GunTrunnion,
            MaterialRole::CastArmor,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-tu.mantlet_radius, g.trunnion_y - tu.mantlet_radius, tu.mantlet_back_z),
            Vec3::new(tu.mantlet_radius, g.trunnion_y + tu.mantlet_radius, tu.mantlet_front_z),
            "Blueprint box mantlet where the 8.8 cm meets the turret face.",
        ),
        part(
            ForgePartKind::Gun,
            PartAnchor::GunTrunnion,
            MaterialRole::BarrelSteel,
            Vec3::new(0.0, g.trunnion_y, g.trunnion_z),
            Vec3::new(-g.barrel_radius, g.trunnion_y - g.barrel_radius, g.trunnion_z),
            Vec3::new(g.barrel_radius, g.trunnion_y + g.barrel_radius, g.muzzle_z),
            "Blueprint KwK 36 L/56: trunnion at the 2.17 m fire line (documented firing height \
             2.195 m) to the braked muzzle at 5.29 m.",
        ),
        part(
            ForgePartKind::Cupola,
            PartAnchor::TurretRing,
            turret_material(tu.form),
            Vec3::new(tu.cupola_x, tu.roof_y, tu.cupola_z),
            Vec3::new(tu.cupola_x - tu.cupola_radius, tu.roof_y, tu.cupola_z - tu.cupola_radius),
            Vec3::new(
                tu.cupola_x + tu.cupola_radius,
                tu.roof_y + tu.cupola_proud_m(h),
                tu.cupola_z + tu.cupola_radius,
            ),
            "The drum cupola on the left rear roof — 0.115 m of drum over the 2.885 m roof, \
             which together make the documented 3.00 m silhouette.",
        ),
        part(
            ForgePartKind::EngineDeck,
            PartAnchor::Hull,
            MaterialRole::RolledArmor,
            Vec3::new(0.0, h.deck_y + 0.02, -0.5 * h.half_len),
            Vec3::new(-h.lower_half_width, h.deck_y, -h.half_len + 0.12),
            Vec3::new(h.lower_half_width, h.deck_y + 0.10, -0.9),
            "Rear engine deck with the shielded twin exhaust stacks standing on the tail plate \
             (late-E signature; Feifel cleaners deliberately absent).",
        ),
    ]
}
