//! Per-vehicle blueprint data. Migrated vehicles return `Some`; the rest return `None` and keep the
//! legacy hand-authored hitbox/mounts/armour + recipe until they are migrated.
//!
//! The T-54 values reproduce the current gameplay hitbox, mount frames, and armour facets exactly,
//! so migrating it changes only the *visual* mesh (now built from these same numbers) — gameplay is
//! byte-for-byte unchanged.

use super::t54_hybrid::t54_hybrid;
use super::{
    ArmorShape, GunShape, HullShape, TrackShape, TurretForm, TurretShape, VehicleBlueprint,
};
use crate::VehicleKind;

pub(super) fn blueprint(kind: VehicleKind) -> Option<VehicleBlueprint> {
    match kind {
        VehicleKind::T54_1951 => Some(t54()),
        VehicleKind::T55A => Some(t55a_blueprint()),
        _ => None,
    }
}

fn t54() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::T54_1951,
        hull: HullShape {
            half_len: 3.00,
            half_width: 1.45,
            belly_y: 0.10,
            deck_y: 1.30,
            glacis_slope_deg: 60.0,
            nose_rise: 0.06,
            rear_slope_deg: 8.0,
            lower_half_width: 1.22,
            sponson_y: 0.55,
            hitbox_half_width: 1.75,
            hitbox_half_height: 1.20,
            hitbox_half_length: 3.15,
            hitbox_center_y: 1.15,
            hitbox_turret_min_y: 0.65,
        },
        track: TrackShape {
            center_x: 1.50,
            belt_half_thickness: 0.13,
            top_y: 0.84,
            bottom_y: 0.02,
            wheel_radius: 0.34,
            wheel_count: 5,
            wheel_first_z: -2.10,
            wheel_last_z: 2.10,
            end_radius: 0.41,
            inner_x: 1.40,
            outer_x: 1.55,
            segments: 14,
        },
        turret: TurretShape {
            form: TurretForm::CastDome,
            ring_y: 1.30,
            ring_z: 0.0,
            ring_radius: 0.82,
            base_radius: 1.00,
            roof_radius: 0.34,
            roof_y: 1.86,
            front_slope_deg: 35.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 10.0,
            cupola_x: -0.34,
            cupola_z: -0.10,
            cupola_radius: 0.24,
            plan_half_width: 1.00,
            plan_half_length: 1.04,
            mantlet_radius: 0.30,
            mantlet_back_z: 0.78,
            mantlet_front_z: 1.04,
        },
        gun: GunShape {
            trunnion_y: 1.70,
            trunnion_z: 1.10,
            muzzle_z: 5.20,
            barrel_radius: 0.092,
            evacuator: None,
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
        hybrid: Some(t54_hybrid()),
    }
}

/// The T-55A: the T-54's close relative on the same low Soviet medium chassis. It reuses the T-54
/// shape model (wrapped five-wheel running gear, rounded cast dome) and differs in the details the
/// references show — a slightly longer hull, a marginally smaller turret, and the family's longer
/// gun with the bore evacuator carried further forward.
///
/// The hitbox and armour facets reproduce the T-55A's current gameplay values exactly, and the
/// trunnion/muzzle mounts are unchanged; only the turret-ring *visual* pivot shifts 2 cm in Z (the
/// blueprint unifies the ring with the turret-plan centre). Migrating it changes the visible mesh —
/// most visibly correcting the running gear from six wheels to the historical five — not gameplay.
fn t55a_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::T55A,
        hull: HullShape {
            half_len: 3.05,
            half_width: 1.45,
            belly_y: 0.10,
            deck_y: 1.30,
            glacis_slope_deg: 60.0,
            nose_rise: 0.06,
            rear_slope_deg: 8.0,
            lower_half_width: 1.22,
            sponson_y: 0.55,
            hitbox_half_width: 1.75,
            hitbox_half_height: 1.19,
            hitbox_half_length: 3.20,
            hitbox_center_y: 1.14,
            hitbox_turret_min_y: 0.66,
        },
        track: TrackShape {
            center_x: 1.50,
            belt_half_thickness: 0.13,
            top_y: 0.84,
            bottom_y: 0.02,
            wheel_radius: 0.42,
            wheel_count: 5,
            wheel_first_z: -2.12,
            wheel_last_z: 2.12,
            end_radius: 0.46,
            inner_x: 1.40,
            outer_x: 1.55,
            segments: 14,
        },
        turret: TurretShape {
            form: TurretForm::CastDome,
            ring_y: 1.30,
            ring_z: 0.07,
            ring_radius: 0.78,
            base_radius: 0.95,
            roof_radius: 0.30,
            roof_y: 2.03,
            front_slope_deg: 35.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 10.0,
            cupola_x: -0.30,
            cupola_z: -0.12,
            cupola_radius: 0.23,
            plan_half_width: 0.95,
            plan_half_length: 0.97,
            mantlet_radius: 0.27,
            mantlet_back_z: 0.84,
            mantlet_front_z: 1.00,
        },
        gun: GunShape {
            trunnion_y: 1.78,
            trunnion_z: 1.05,
            muzzle_z: 5.30,
            barrel_radius: 0.092,
            evacuator: Some((0.57, 0.135)),
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
        hybrid: None,
    }
}
