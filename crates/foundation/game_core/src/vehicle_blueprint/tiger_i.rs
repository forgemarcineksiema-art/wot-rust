//! The Tiger I blueprint: the first German heavy moved onto the shape SSOT, split out of
//! `data.rs` to keep each vehicle's shape data reviewable on its own.

use super::{
    ArmorShape, GunShape, HullShape, TrackShape, TurretForm, TurretShape, VehicleBlueprint,
};
use crate::VehicleKind;

/// The Tiger I Ausf. E at its documented 1:1 anatomy: hull 6.316 m, 3.705 m over the 725 mm
/// combat tracks, 3.00 m to the cupola top, 0.47 m ground clearance. The whole point of the
/// Tiger is the SLAB: nearly vertical plates everywhere (driver's plate ~9° from vertical,
/// genuinely vertical sides, ~8° rear), a broad horseshoe turret of vertical walls standing on
/// the tall superstructure, and eight interleaved 800 mm road wheels per side — the antithesis
/// of the sloped Soviet school on the same battlefield. The 8.8 cm KwK 36 L/56 reaches
/// z = 5.29 (8.45 m overall, gun forward), tipped with its double-baffle brake.
///
/// This replaces the legacy hand-authored Tiger (hitbox fractions + magic-number turret) whose
/// box was 7.2 m long and 2.92 m tall — the migrated hitbox is the RESEARCHED body, a conscious
/// gameplay correction: shorter, narrower, and taller, exactly like the references.
pub(super) fn tiger_i_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::TigerI,
        hull: HullShape {
            half_len: 3.16,
            // The superstructure spreads over the tracks to the FULL 3.705 m beam — the Tiger's
            // signature sponson overhang — while the lower hull tub rides between the belts.
            half_width: 1.85,
            belly_y: 0.47,
            // The tall slab: superstructure roof at 1.90, the turret walls carrying the
            // silhouette to 2.72 and the drum cupola topping out the documented 3.00 m.
            deck_y: 1.90,
            // Nearly vertical: the driver's plate stands at ~9° from vertical. The Tiger's
            // protection is raw thickness, not slope — angling the hull is the crew's job.
            glacis_slope_deg: 9.0,
            pike_sweep_deg: 0.0,
            nose_rise: 0.05,
            rear_slope_deg: 8.0,
            lower_half_width: 1.12,
            sponson_y: 0.95,
            skirt: None,
            hitbox_half_width: 1.87,
            // Realistic full height: top = center_y + half_height = 3.01 bounds the 3.00 m tank.
            // Floor ~5 cm below ground; hull/turret split at world y = 1.90, the deck plane.
            hitbox_half_height: 1.53,
            hitbox_half_length: 3.22,
            hitbox_center_y: 1.48,
            hitbox_turret_min_y: 0.42,
        },
        track: TrackShape {
            // The 725 mm combat band: outer face at the 3.705 m beam (1.84 with a hair of fender
            // lip), inner face at the tub side. Eight 800 mm interleaved road wheels per side on
            // the documented 3.6 m contact run; sprocket and idler wrap inside the hull ends.
            center_x: 1.485,
            belt_half_thickness: 0.16,
            top_y: 0.86,
            bottom_y: 0.03,
            wheel_radius: 0.40,
            wheel_count: 8,
            wheel_first_z: -1.80,
            wheel_last_z: 1.80,
            end_radius: 0.30,
            end_z: 2.60,
            // Wrap top (end_y + end_radius) = 0.86 = the belt's top line: one height, the top
            // run resting on the big interleaved wheels — no return rollers on a Tiger.
            end_y: 0.56,
            inner_x: 1.13,
            outer_x: 1.84,
            segments: 14,
            wheel_stations: None,
            return_rollers: 0,
            roller_radius: 0.0,
            // The Schachtellaufwerk: every odd wheel rides a genuinely separate inner row, deep
            // enough that the two rows of discs read as the Tiger's interleaved wall of wheels.
            overlap_inner_dx: 0.22,
        },
        turret: TurretShape {
            // The horseshoe: one bent side wall around a flat front plate, all of it VERTICAL —
            // a welded box to the armor model, with the plate prism's flat normals.
            form: TurretForm::WeldedBox,
            ring_y: 1.90,
            ring_z: -0.10,
            ring_radius: 0.915,
            base_radius: 1.00,
            roof_radius: 0.40,
            roof_y: 2.72,
            front_slope_deg: 8.0,
            side_slope_deg: 0.0,
            rear_slope_deg: 0.0,
            // The drum cupola sits on the LEFT rear roof (the commander's side).
            cupola_x: -0.55,
            cupola_z: -0.45,
            cupola_radius: 0.26,
            plan_half_width: 1.00,
            // Reaches the Rommelkiste stowage bin's back face — the bin IS the rear armor plane.
            plan_half_length: 1.25,
            mantlet_radius: 0.34,
            mantlet_back_z: 0.92,
            mantlet_front_z: 1.22,
        },
        gun: GunShape {
            // Fire line 2.17 (~0.27 above the ring seat); the KwK 36's muzzle reaches z = 5.29
            // (8.45 m overall, gun forward), tipped with its double-baffle brake.
            trunnion_y: 2.17,
            trunnion_z: 1.15,
            muzzle_z: 5.29,
            barrel_radius: 0.10,
            evacuator: None,
            muzzle_brake: Some(0.17),
            segments: 12,
        },
        armor: ArmorShape {
            // The slab, honestly: 9° driver's plate, VERTICAL 80 mm sides, 8° rear; the turret
            // walls stand straight up and only the front plate leans its 8°.
            hull_front: (9.0, 0.9),
            hull_side: (0.0, 1.0),
            hull_rear: (8.0, 1.0),
            turret_front: (8.0, 0.92),
            turret_side: (0.0, 1.0),
            turret_rear: (0.0, 1.0),
        },
        hybrid: None,
    }
}
