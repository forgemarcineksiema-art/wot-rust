//! The IS-3 blueprint: the era's Soviet heavy, split out of `data.rs` to keep each vehicle's
//! shape data reviewable on its own.

use super::{
    ArmorShape, GunShape, HullShape, ShoePattern, TrackShape, TurretForm, TurretShape,
    VehicleBlueprint, WheelFace,
};
use crate::VehicleKind;

/// The IS-3: the era's Soviet heavy and the pike-nose archetype. Documented 1:1 anatomy: hull
/// 6.77 m, 3.15 m over the 650 mm tracks, 2.44 m to the turret roof, six 550 mm road wheels per
/// side, the "shchuchy nos" bow (two 110 mm plates at ~56° from vertical swept ~38° in plan
/// meeting at a central ridge), a wide flattened cast dome overhanging its ring, and the 122 mm
/// D-25T reaching 9.85 m overall. The pike is the whole point: born on the blueprint, its two
/// bow plates ARE the armor volumes' front planes — angling the hull flattens one face.
pub(super) fn is3_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::IS3,
        hull: HullShape {
            half_len: 3.385,
            // The upper hull spreads over the tracks (sponsons); the tub between the 650 mm
            // belts is narrow.
            half_width: 1.55,
            belly_y: 0.46,
            // The stepped-down heavy silhouette: hull roof at 1.62, the flattened dome carrying
            // the 2.44 m total with the cupola.
            deck_y: 1.62,
            glacis_slope_deg: 56.0,
            pike_sweep_deg: 38.0,
            nose_rise: 0.05,
            rear_slope_deg: 18.0,
            lower_half_width: 0.92,
            sponson_y: 0.90,
            skirt: None,
            hitbox_half_width: 1.575,
            hitbox_half_height: 1.27,
            hitbox_half_length: 3.44,
            hitbox_center_y: 1.22,
            hitbox_turret_min_y: 0.40,
        },
        track: TrackShape {
            center_x: 1.25,
            belt_half_thickness: 0.12,
            top_y: 0.78,
            bottom_y: 0.03,
            wheel_radius: 0.275,
            wheel_count: 6,
            wheel_first_z: -2.30,
            wheel_last_z: 2.30,
            end_radius: 0.28,
            end_z: 2.90,
            // Wrap top (end_y + end_radius + link seat) = 0.78 = the belt's top line = the
            // roller tops = the static band: one height, everything meets.
            end_y: 0.48,
            inner_x: 0.93,
            outer_x: 1.575,
            segments: 14,
            wheel_stations: None,
            // The IS family carries its top run on three small return rollers per side — the
            // heavy's look against the T-54's wheel-riding run.
            return_rollers: 3,
            roller_radius: 0.11,
            overlap_inner_dx: 0.0,
            wheel_half_width: 0.14,
            link_half_width: 0.26,
            link_count: Some(104),
            top_sag_m: 0.012,
            wheel_spokes: 12,
            drive_front: false,
            shoe_pattern: ShoePattern::Omsh,
            wheel_face: WheelFace::Openwork,
            suspension: super::SuspensionKind::TorsionBar,
        },
        turret: TurretShape {
            form: TurretForm::CastDome,
            ring_y: 1.62,
            ring_z: -0.10,
            ring_radius: 0.90,
            base_radius: 1.15,
            roof_radius: 0.35,
            roof_y: 2.39,
            // The IS-3 casting is a genuine frying-pan dome: steep everywhere, steepest at the
            // face — the whole turret reads as one continuous glancing surface.
            front_slope_deg: 55.0,
            side_slope_deg: 45.0,
            rear_slope_deg: 30.0,
            cupola_x: -0.35,
            cupola_z: -0.35,
            cupola_radius: 0.27,
            plan_half_width: 1.15,
            plan_half_length: 1.30,
            mantlet_radius: 0.35,
            mantlet_back_z: 0.95,
            sector_count: super::DEFAULT_TURRET_SECTORS,
            mantlet_front_z: 1.25,
        },
        gun: GunShape {
            // Fire line ~0.3 above the ring seat; the D-25T's muzzle reaches z = 6.46 (9.85 m
            // overall, gun forward), tipped with its double-baffle brake.
            trunnion_y: 1.93,
            trunnion_z: 1.05,
            muzzle_z: 6.46,
            barrel_radius: 0.105,
            evacuator: None,
            muzzle_brake: Some(0.155),
            segments: 12,
        },
        armor: ArmorShape {
            hull_front: (56.0, 0.9),
            hull_side: (15.0, 1.0),
            hull_rear: (18.0, 1.0),
            turret_front: (55.0, 0.9),
            turret_side: (45.0, 1.0),
            turret_rear: (30.0, 1.0),
        },
        hybrid: None,
    }
}
