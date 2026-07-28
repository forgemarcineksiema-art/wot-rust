//! The Centurion blueprint: the British line's first vehicle, blueprint-born from day one and
//! split out of `data.rs` to keep each vehicle's shape data reviewable on its own.

use super::{
    ArmorShape, GunShape, HullShape, ShoePattern, SkirtShape, TrackShape, TurretForm, TurretShape,
    VehicleBlueprint, WheelFace,
};
use crate::VehicleKind;

/// The Centurion Mk 3 at its documented 1:1 anatomy: hull 7.60 m, 3.38 m over the full-length
/// skirts, 2.99 m to the cupola, 0.51 m clearance. The character is the SKIRT and the bogie:
/// full-length bazooka plates hide the upper half of the running gear (and the armor volumes
/// bake them as the spaced HEAT screen they were fitted to be — the first authored skirt in
/// the fleet), while under them six 800 mm wheels ride in three Horstmann bogie PAIRS with a
/// real gap between bogies. Above, a 76 mm glacis at 57° (the steepest plate in the game) runs
/// into a cast Mk 3 turret with the stowage bin closing its bustle, and the 84 mm 20-pounder
/// Type A — no brake, no evacuator, just a clean line — reaches z = 6.03 (9.83 m overall).
pub(super) fn centurion_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::Centurion,
        hull: HullShape {
            half_len: 3.80,
            half_width: 1.60,
            belly_y: 0.51,
            deck_y: 1.88,
            // The steepest glacis in the fleet: thin 76 mm bought back with 57° of slope.
            glacis_slope_deg: 57.0,
            pike_sweep_deg: 0.0,
            nose_rise: 0.05,
            rear_slope_deg: 8.0,
            lower_half_width: 0.95,
            sponson_y: 0.95,
            // The full-length bazooka plates: from the fender line down over the wheel tops,
            // hung one standoff outside the track — the same plane the armor volumes bake as
            // a spaced screen a HEAT jet detonates against early.
            skirt: Some(SkirtShape {
                top_y: 0.98,
                bottom_y: 0.40,
                front_z: 2.90,
                rear_z: -2.90,
                standoff_m: 0.08,
                thickness_m: 0.008,
            }),
            hitbox_half_width: 1.70,
            // Realistic full height: top = center_y + half_height = 2.99 bounds the tank.
            hitbox_half_height: 1.52,
            hitbox_half_length: 3.86,
            hitbox_center_y: 1.47,
            hitbox_turret_min_y: 0.41,
        },
        track: TrackShape {
            // The 610 mm band under the skirts. Horstmann: six 800 mm wheels in three bogie
            // PAIRS — tight within a bogie, a real gap between bogies — the single source both
            // the rendered gear and the physics contact footprint read.
            center_x: 1.275,
            belt_half_thickness: 0.13,
            top_y: 0.86,
            bottom_y: 0.03,
            wheel_radius: 0.40,
            wheel_count: 6,
            wheel_first_z: -2.05,
            wheel_last_z: 2.05,
            end_radius: 0.28,
            end_z: 2.75,
            // Wrap top (end_y + end_radius) = 0.86 = the belt's top line, carried on three
            // small return rollers per side over the bogie gaps.
            end_y: 0.58,
            inner_x: 0.97,
            outer_x: 1.58,
            segments: 14,
            wheel_stations: Some(&[-2.05, -1.45, -0.30, 0.30, 1.45, 2.05]),
            return_rollers: 3,
            roller_radius: 0.10,
            overlap_inner_dx: 0.0,
            wheel_half_width: 0.12,
            link_half_width: 0.24,
            link_count: None,
            top_sag_m: 0.035,
            wheel_spokes: 6,
            shoe_pattern: ShoePattern::BritishCast,
            wheel_face: WheelFace::RubberDish,
            suspension: super::SuspensionKind::Horstmann,
            drive_front: false,
        },
        turret: TurretShape {
            // The Mk 3 casting: a rounded dome on the 74-inch race, its bustle closed by the
            // signature rear stowage bin (authored in the recipe inside this plan).
            form: TurretForm::CastDome,
            ring_y: 1.88,
            ring_z: -0.15,
            ring_radius: 0.94,
            base_radius: 1.05,
            roof_radius: 0.40,
            roof_y: 2.78,
            front_slope_deg: 40.0,
            side_slope_deg: 30.0,
            rear_slope_deg: 15.0,
            cupola_x: -0.40,
            cupola_z: -0.55,
            cupola_radius: 0.33,
            cupola_height: None,
            plan_half_width: 1.10,
            plan_half_length: 1.45,
            mantlet_radius: 0.30,
            mantlet_back_z: 1.02,
            sector_count: super::DEFAULT_TURRET_SECTORS,
            mantlet_front_z: 1.32,
        },
        gun: GunShape {
            // Fire line 2.16; the 20-pounder Type A's muzzle reaches z = 6.03 (9.83 m overall,
            // gun forward) — a clean tube: no muzzle brake, no fume extractor.
            trunnion_y: 2.16,
            trunnion_z: 1.30,
            muzzle_z: 6.03,
            barrel_radius: 0.095,
            evacuator: None,
            muzzle_brake: None,
            segments: 12,
        },
        armor: ArmorShape {
            // The gunnery school: a thin 76 mm glacis at the fleet's steepest 57°, vertical
            // 51 mm sides SCREENED by the skirts, and the cast turret's 40/30/15.
            hull_front: (57.0, 0.9),
            hull_side: (0.0, 1.0),
            hull_rear: (8.0, 1.0),
            turret_front: (40.0, 0.9),
            turret_side: (30.0, 1.0),
            turret_rear: (15.0, 1.0),
        },
        hybrid: None,
    }
}
