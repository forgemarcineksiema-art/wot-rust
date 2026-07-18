//! The Panther II blueprint: the last German off the legacy path, split out of `data.rs` to
//! keep each vehicle's shape data reviewable on its own.

use super::{
    ArmorShape, GunShape, HullShape, ShoePattern, TrackShape, TurretForm, TurretShape,
    VehicleBlueprint, WheelFace,
};
use crate::VehicleKind;

/// The Panther II (the up-armored prototype) at its documented 1:1 anatomy: hull 6.87 m,
/// 3.42 m over the 660 mm tracks, 2.99 m to the cupola, 0.54 m ground clearance — the wedge
/// of the German line. The 100 mm glacis leans 55°, the steepest German plate in the fleet
/// (the whole front is one ramp), the upper sides lean their 29°, and the narrow Schmalturm
/// carries a 20° face with sharply converging 25° cheeks. Seven overlapped steel-rimmed
/// 800 mm wheels per side (the Tiger II school, not the rubber Panther dish), and the
/// 7.5 cm KwK 42 L/70 reaching z = 5.60.
///
/// This replaces the legacy hand-authored body; the migrated hitbox is the researched
/// vehicle, a conscious gameplay correction.
pub(super) fn panther_ii_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::PantherII,
        hull: HullShape {
            half_len: 3.435,
            half_width: 1.68,
            belly_y: 0.54,
            deck_y: 1.85,
            // The ramp: 100 mm at 55° — the steepest German glacis in the fleet.
            glacis_slope_deg: 55.0,
            pike_sweep_deg: 0.0,
            nose_rise: 0.05,
            rear_slope_deg: 30.0,
            lower_half_width: 0.99,
            sponson_y: 0.98,
            skirt: None,
            hitbox_half_width: 1.73,
            // Realistic full height: top = center_y + half_height = 2.99 bounds the tank.
            hitbox_half_height: 1.52,
            hitbox_half_length: 3.49,
            hitbox_center_y: 1.47,
            hitbox_turret_min_y: 0.38,
        },
        track: TrackShape {
            // The 660 mm band: seven overlapped steel wheels per side — the Panther II traded
            // the Panther's interleaved rubber dish for the Tiger II's simpler stagger.
            center_x: 1.35,
            belt_half_thickness: 0.14,
            top_y: 0.86,
            bottom_y: 0.03,
            wheel_radius: 0.40,
            wheel_count: 7,
            wheel_first_z: -1.75,
            wheel_last_z: 1.75,
            end_radius: 0.30,
            end_z: 2.55,
            end_y: 0.56,
            inner_x: 1.00,
            outer_x: 1.70,
            segments: 14,
            wheel_stations: None,
            return_rollers: 0,
            roller_radius: 0.0,
            overlap_inner_dx: 0.20,
            wheel_half_width: 0.09,
            link_half_width: 0.26,
            link_count: None,
            top_sag_m: 0.035,
            wheel_spokes: 6,
            drive_front: true,
            shoe_pattern: ShoePattern::Kgs,
            wheel_face: WheelFace::SteelDish,
            suspension: super::SuspensionKind::TorsionBar,
        },
        turret: TurretShape {
            // The Schmalturm: deliberately NARROW — a small 20° face behind the cone mantlet,
            // 25° cheeks converging hard toward the roof, a shot trap killed by design.
            form: TurretForm::WeldedBox,
            ring_y: 1.85,
            ring_z: -0.03,
            ring_radius: 0.83,
            base_radius: 1.02,
            roof_radius: 0.35,
            roof_y: 2.72,
            front_slope_deg: 11.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 25.0,
            // The low cupola sits close to the centreline — the converged roof is narrow.
            cupola_x: -0.22,
            cupola_z: -0.55,
            cupola_radius: 0.31,
            plan_half_width: 1.02,
            plan_half_length: 1.15,
            mantlet_radius: 0.30,
            mantlet_back_z: 0.92,
            mantlet_front_z: 1.22,
        },
        gun: GunShape {
            // Fire line 2.18; the KwK 42 L/70's muzzle reaches z = 5.60 (~8.9 m overall, gun
            // forward), tipped with its small brake.
            trunnion_y: 2.18,
            trunnion_z: 1.20,
            muzzle_z: 5.43,
            barrel_radius: 0.085,
            evacuator: None,
            muzzle_brake: Some(0.14),
            segments: 12,
        },
        armor: ArmorShape {
            // The wedge, honestly: 55° ramp, 29° leaned sides, 30° rear; the Schmalturm keeps
            // the mantlet weakspot on its 20° face.
            hull_front: (55.0, 1.0),
            hull_side: (29.0, 1.0),
            hull_rear: (30.0, 1.0),
            turret_front: (20.0, 0.9),
            turret_side: (25.0, 1.0),
            turret_rear: (20.0, 1.0),
        },
        hybrid: None,
    }
}
