//! Per-vehicle blueprint data. Migrated vehicles return `Some`; the rest return `None` and keep the
//! legacy hand-authored hitbox/mounts/armour + recipe until they are migrated.
//!
//! The T-54 values reproduce the current gameplay hitbox, mount frames, and armour facets exactly,
//! so migrating it changes only the *visual* mesh (now built from these same numbers) — gameplay is
//! byte-for-byte unchanged.

use super::{
    ArmorShape, GunShape, HullShape, TrackShape, TurretForm, TurretShape, VehicleBlueprint,
};
use crate::VehicleKind;

pub(super) fn blueprint(kind: VehicleKind) -> Option<VehicleBlueprint> {
    match kind {
        VehicleKind::T54_1951 => Some(t54()),
        _ => None,
    }
}

fn t54() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::T54_1951,
        hull: HullShape {
            half_len: 3.00,
            half_width: 1.34,
            belly_y: 0.30,
            deck_y: 1.30,
            glacis_slope_deg: 60.0,
            nose_rise: 0.08,
            rear_slope_deg: 8.0,
            sponson_overhang: 0.16,
            hitbox_half_width: 1.75,
            hitbox_half_height: 1.20,
            hitbox_half_length: 3.15,
            hitbox_center_y: 1.15,
            hitbox_turret_min_y: 0.65,
        },
        track: TrackShape {
            center_x: 1.51,
            belt_half_thickness: 0.24,
            top_y: 0.77,
            bottom_y: -0.06,
            wheel_radius: 0.32,
            wheel_count: 6,
            wheel_first_z: -2.25,
            wheel_last_z: 2.25,
            end_radius: 0.46,
            inner_x: 1.66,
            outer_x: 1.74,
            segments: 14,
        },
        turret: TurretShape {
            form: TurretForm::CastDome,
            ring_y: 1.30,
            ring_z: 0.0,
            ring_radius: 0.82,
            base_radius: 1.00,
            roof_radius: 0.34,
            roof_y: 2.18,
            front_slope_deg: 35.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 10.0,
            cupola_x: -0.34,
            cupola_z: -0.10,
            cupola_radius: 0.18,
            plan_half_width: 1.00,
            plan_half_length: 1.04,
            mantlet_radius: 0.29,
            mantlet_back_z: 0.88,
            mantlet_front_z: 1.22,
        },
        gun: GunShape {
            trunnion_y: 1.80,
            trunnion_z: 1.10,
            muzzle_z: 5.20,
            barrel_radius: 0.092,
            evacuator: Some((0.79, 0.135)),
            muzzle_brake: None,
            segments: 12,
        },
        armor: ArmorShape {
            hull_front: (60.0, 0.82),
            hull_side: (10.0, 1.0),
            hull_rear: (5.0, 1.0),
            turret_front: (35.0, 0.9),
            turret_side: (25.0, 1.0),
            turret_rear: (10.0, 1.0),
        },
    }
}
