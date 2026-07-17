//! The Tiger II blueprint: the second German heavy on the shape SSOT, split out of `data.rs`
//! to keep each vehicle's shape data reviewable on its own.

use super::{
    ArmorShape, GunShape, HullShape, TrackShape, TurretForm, TurretShape, VehicleBlueprint,
};
use crate::VehicleKind;

/// The Tiger II Ausf. B (Henschel turret) at its documented 1:1 anatomy: hull 7.38 m, 3.755 m
/// over the 800 mm combat tracks, 3.09 m tall, 0.50 m ground clearance. Where the Tiger I is
/// the vertical slab, the Tiger II is the SLOPE the Germans learned: the fleet's longest
/// glacis (150 mm at 50°), upper hull sides leaned 25° over the tracks, a long faceted
/// Henschel turret with a rear bustle, and the 8.8 cm KwK 43 L/71 — the longest gun in the
/// lineup — reaching z = 6.60 (10.29 m overall, gun forward). Nine overlapped 800 mm road
/// wheels per side ride the 4.1 m contact run.
///
/// This replaces the legacy hand-authored body whose box was 8.0 m long with vertical sides —
/// the migrated hitbox is the RESEARCHED body, a conscious gameplay correction.
pub(super) fn tiger_ii_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::TigerII,
        hull: HullShape {
            half_len: 3.69,
            // The superstructure spreads to the full beam at the sponson step, then LEANS: the
            // 25° upper sides are the Tiger II's answer to the Tiger I's vertical wall.
            half_width: 1.85,
            belly_y: 0.50,
            deck_y: 1.86,
            // The fleet's longest glacis: one 150 mm plate at 50° from the fold to the deck.
            glacis_slope_deg: 50.0,
            pike_sweep_deg: 0.0,
            nose_rise: 0.05,
            rear_slope_deg: 30.0,
            lower_half_width: 1.06,
            sponson_y: 0.95,
            skirt: None,
            hitbox_half_width: 1.89,
            // Realistic full height: top = center_y + half_height = 3.09 bounds the tank.
            hitbox_half_height: 1.57,
            hitbox_half_length: 3.74,
            hitbox_center_y: 1.52,
            hitbox_turret_min_y: 0.34,
        },
        track: TrackShape {
            // The 800 mm combat band: the widest belt in the German line, outer face at the
            // 3.755 m beam. Nine 800 mm wheels per side, overlapped (not interleaved): every
            // odd wheel rides a second row one disc-width inboard.
            center_x: 1.47,
            belt_half_thickness: 0.16,
            top_y: 0.86,
            bottom_y: 0.03,
            wheel_radius: 0.40,
            wheel_count: 9,
            wheel_first_z: -2.06,
            wheel_last_z: 2.06,
            end_radius: 0.30,
            end_z: 2.95,
            // Wrap top (end_y + end_radius) = 0.86 = the belt's top line; the top run rests on
            // the overlapped wheels — no return rollers on the Tiger line.
            end_y: 0.56,
            inner_x: 1.07,
            outer_x: 1.87,
            segments: 14,
            wheel_stations: None,
            return_rollers: 0,
            roller_radius: 0.0,
            overlap_inner_dx: 0.20,
            wheel_half_width: 0.09,
            link_half_width: 0.31,
            link_count: None,
            top_sag_m: 0.035,
            wheel_spokes: 6,
            drive_front: true,
        },
        turret: TurretShape {
            // The Henschel: a long faceted welded prism — leaned front plate, 21° sides
            // converging to a narrow roof, and the bustle carrying the plan far behind the ring.
            form: TurretForm::WeldedBox,
            ring_y: 1.86,
            ring_z: 0.05,
            ring_radius: 0.925,
            base_radius: 0.95,
            roof_radius: 0.40,
            roof_y: 2.75,
            front_slope_deg: 10.0,
            side_slope_deg: 21.0,
            rear_slope_deg: 20.0,
            // The low commander's cupola on the left rear roof.
            cupola_x: -0.40,
            cupola_z: -0.55,
            cupola_radius: 0.39,
            plan_half_width: 0.98,
            plan_half_length: 1.55,
            mantlet_radius: 0.30,
            mantlet_back_z: 1.35,
            mantlet_front_z: 1.66,
        },
        gun: GunShape {
            // Fire line 2.21 (~0.35 above the ring seat); the KwK 43's muzzle reaches z = 6.60
            // (10.29 m overall, gun forward) — five metres of barrel past the trunnion, tipped
            // with its double-baffle brake.
            trunnion_y: 2.21,
            trunnion_z: 1.60,
            muzzle_z: 6.60,
            barrel_radius: 0.105,
            evacuator: None,
            muzzle_brake: Some(0.18),
            segments: 12,
        },
        armor: ArmorShape {
            // The sloped school, honestly: 50° glacis, 25° leaned upper sides, 30° rear; the
            // Henschel plates carry their real 10°/21°/20° with the mantlet weakspot up front.
            hull_front: (50.0, 1.0),
            hull_side: (25.0, 1.0),
            hull_rear: (30.0, 1.0),
            turret_front: (10.0, 0.9),
            turret_side: (21.0, 1.0),
            turret_rear: (20.0, 1.0),
        },
        hybrid: None,
    }
}
