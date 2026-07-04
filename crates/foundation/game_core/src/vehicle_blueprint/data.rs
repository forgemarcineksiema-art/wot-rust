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

// The T-54 obr. 1951 at its documented 1:1 dimensions (blueprint-verified): hull 6.04 m long,
// 3.27 m over the tracks, 2.40 m to the cupola top, 0.425 m ground clearance, 810 mm road wheels,
// 580 mm track, a ~2.25 m cast turret seated on the 1.75 m hull roof, and the D-10T reaching
// z = 5.95 (9.0 m overall length, gun forward). Every consumer reads these numbers, so the visible
// tank, the hitbox and the armour all describe the same vehicle the references show.
fn t54() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::T54_1951,
        hull: HullShape {
            half_len: 3.00,
            // The T-54 hull is a NARROW box (~2.1 m over the vertical 80 mm side plates) riding
            // between fully exposed tracks — there are no Panther-style sponsons overhanging the
            // running gear. The tracks stand beside the hull nearly flush with its envelope, and
            // the fender shelves with their stowage carry the visual mass above them.
            half_width: 1.05,
            // Real ground clearance: 0.425 m. The earlier 0.10 dropped the belly to the dirt and
            // made the hull side read as a wall reaching the ground.
            belly_y: 0.43,
            // The LOW hull is the T-54's signature: the roof sits at ~1.58 (documented fire line
            // 1.79 with the gun bedded ~0.2 above the ring seat), leaving the tall ~0.7 m cast
            // dome to carry the rest of the 2.40 m silhouette.
            deck_y: 1.58,
            glacis_slope_deg: 60.0,
            nose_rise: 0.06,
            rear_slope_deg: 8.0,
            // One box: the "tub" and the plate above the fold share the same vertical side plane.
            lower_half_width: 1.05,
            sponson_y: 1.00,
            hitbox_half_width: 1.75,
            // Realistic full height: top = center_y + half_height = 2.43 bounds the 2.40 m tank
            // (cupola apex ~2.42). Floor ~5 cm below ground (center_y = half_height - 0.05);
            // hull/turret split at world y = 1.58, the hull-roof plane.
            hitbox_half_height: 1.24,
            hitbox_half_length: 3.15,
            hitbox_center_y: 1.19,
            hitbox_turret_min_y: 0.39,
        },
        track: TrackShape {
            // The 580 mm OMSh band: outer face flush with the 3.27 m overall width (1.63), inner
            // face at the tub side (1.06). Road wheels are the documented 810 mm discs; the small
            // idler and toothed sprocket sit beyond the wheel span on raised axles, and the belt
            // ramps up to them as the references show.
            center_x: 1.345,
            belt_half_thickness: 0.13,
            top_y: 0.905,
            bottom_y: 0.045,
            wheel_radius: 0.405,
            wheel_count: 5,
            wheel_first_z: -1.95,
            wheel_last_z: 1.95,
            end_radius: 0.30,
            end_z: 2.66,
            // Raised so the top run (end_y + end_radius = 0.96) carries the link plates CLEAR of
            // the 0.88 wheel tops through the full sag — links grinding the tires z-fight.
            end_y: 0.66,
            inner_x: 1.06,
            outer_x: 1.63,
            segments: 14,
            // 810 mm wheels at a ~0.92 m pitch with the T-54's signature wider gap between the
            // first and second road wheels at the front (+Z). Single source: the rendered
            // running gear and the physics contact footprint both read these stations.
            wheel_stations: Some(&[-1.95, -1.03, -0.11, 0.81, 1.95]),
        },
        turret: TurretShape {
            form: TurretForm::CastDome,
            // The tall hemispherical casting (~0.96 m in the factory drawings, ~0.7 visible above
            // the seat) sits on the LOW 1.58 hull roof and carries the silhouette to 2.27, the
            // cupola topping out the documented 2.40 m. The ring is the real 1.816 m race; the
            // dome overhangs it out to the ~2.25 m casting diameter.
            ring_y: 1.58,
            ring_z: 0.0,
            ring_radius: 0.91,
            base_radius: 1.12,
            roof_radius: 0.42,
            roof_y: 2.27,
            front_slope_deg: 35.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 10.0,
            cupola_x: -0.34,
            cupola_z: -0.10,
            cupola_radius: 0.24,
            plan_half_width: 1.125,
            plan_half_length: 1.17,
            mantlet_radius: 0.32,
            mantlet_back_z: 0.88,
            mantlet_front_z: 1.16,
        },
        gun: GunShape {
            // The documented fire line: the D-10T axis at 1.78 (~0.2 above the ring seat), and
            // the muzzle at z = 5.95 — the 2.9 m overhang past the bow (9.00 m gun forward).
            trunnion_y: 1.78,
            trunnion_z: 1.15,
            muzzle_z: 5.95,
            barrel_radius: 0.092,
            evacuator: None,
            muzzle_brake: None,
            segments: 12,
        },
        armor: ArmorShape {
            hull_front: (60.0, 0.82),
            // The documented 80 mm side plates are VERTICAL — and the visible side carries the
            // same rake the penetration model reads ("what you see is what you shoot").
            hull_side: (0.0, 1.0),
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
            // Degenerate end-wheel placement: the T-55A still wraps its belt at the wheel span on
            // the axle line (the legacy stadium loop), unchanged until its own reference pass.
            end_z: 2.12,
            end_y: 0.43,
            inner_x: 1.40,
            outer_x: 1.55,
            segments: 14,
            wheel_stations: None,
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
