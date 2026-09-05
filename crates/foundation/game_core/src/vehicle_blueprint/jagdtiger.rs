//! The Jagdtiger blueprint: the first CASEMATE on the shape SSOT, split out of `data.rs` to
//! keep each vehicle's shape data reviewable on its own.

use super::{
    ArmorShape, GunShape, HullShape, ShoePattern, TrackShape, TurretForm, TurretShape,
    VehicleBlueprint, WheelFace,
};
use crate::VehicleKind;

/// The Jagdtiger at its documented 1:1 anatomy: the Tiger II chassis stretched to a 7.80 m
/// hull, 3.625 m over the 800 mm combat tracks, 2.95 m to the superstructure roof, with the
/// fighting compartment welded INTO the hull — the casemate's 25° side walls are the SAME
/// planes as the hull's leaned upper sides, one unbroken slope from sponson to roof. The
/// 250 mm face leans only 15° (thickness over slope, like the Tiger I school), and the
/// 12.8 cm PaK 44 L/55 — the longest, fattest barrel in the lineup — reaches z = 6.75
/// (10.65 m overall), overhanging the bow by almost three metres. A casemate never
/// traverses: the sim clamps its yaw, and the armor volume is a fixed prism.
pub(super) fn jagdtiger_blueprint() -> VehicleBlueprint {
    VehicleBlueprint {
        kind: VehicleKind::Jagdtiger,
        hull: HullShape {
            half_len: 3.90,
            half_width: 1.78,
            belly_y: 0.50,
            deck_y: 1.86,
            // The Tiger II hull school: one long 50° glacis into the casemate face.
            glacis_slope_deg: 50.0,
            pike_sweep_deg: 0.0,
            nose_rise: 0.05,
            rear_slope_deg: 30.0,
            lower_half_width: 0.99,
            sponson_y: 0.95,
            skirt: None,
            hitbox_half_width: 1.82,
            // Realistic full height: top = center_y + half_height = 2.95 bounds the tank.
            hitbox_half_height: 1.50,
            hitbox_half_length: 3.95,
            hitbox_center_y: 1.45,
            hitbox_turret_min_y: 0.41,
        },
        track: TrackShape {
            // The 800 mm band on the stretched chassis: nine overlapped wheels per side over
            // the lineup's longest contact run.
            center_x: 1.40,
            belt_half_thickness: 0.16,
            top_y: 0.86,
            bottom_y: 0.03,
            // The road wheels carry the top run, so the axle IS the belt mid-height.
            axle_y: None,
            wheel_radius: 0.40,
            wheel_count: 9,
            wheel_first_z: -2.10,
            wheel_last_z: 2.10,
            end_radius: 0.30,
            end_z: 3.05,
            end_y: 0.56,
            inner_x: 1.00,
            outer_x: 1.80,
            segments: 14,
            wheel_stations: None,
            damper_stations: &[],
            return_rollers: 0,
            roller_radius: 0.0,
            overlap_inner_dx: 0.20,
            wheel_half_width: 0.09,
            link_half_width: 0.31,
            link_count: Some(92), // the Tiger II track it shares
            top_sag_m: 0.035,
            wheel_spokes: 6,
            end_front: None,
            drive_front: true,
            shoe_pattern: ShoePattern::Kgs,
            wheel_face: WheelFace::SteelDish,
            suspension: super::SuspensionKind::TorsionBar,
            // The trailing arm's own geometry: None takes the family default
            // (0.26 / 0.13 for a torsion bar) until this vehicle's dossier says otherwise.
            arm_reach_m: None,
            arm_rise_m: None,
        },
        turret: TurretShape {
            // The fixed fighting compartment. Its side "plan" width is chosen so the casemate
            // wall lies ON the hull's own 25° side plane — one continuous slope, exactly like
            // the welded original (locked by the benchmark cage).
            form: TurretForm::Casemate,
            ring_y: 1.86,
            ring_z: -0.30,
            ring_radius: 0.90,
            base_radius: 1.00,
            roof_radius: 0.40,
            roof_y: 2.88,
            front_slope_deg: 15.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 5.0,
            // No cupola on a Jagdtiger: the "cupola" slot carries the commander's periscope
            // housing, a low box on the right roof.
            cupola_x: -0.45,
            cupola_z: -0.90,
            cupola_radius: 0.14,
            cupola_height: None,
            plan_half_width: 1.356,
            plan_half_length: 1.60,
            // The cast collar around the 12.8 cm stands 0.22 m proud of the 250 mm face
            // (dossier JT.3). That casting is armor a shell must defeat, so the casemate's hit
            // volume reaches it — the only vehicle in the lineup that needs the pad.
            plan_front_pad: 0.22,
            mantlet_radius: 0.36,
            mantlet_back_z: 1.06,
            sector_count: super::DEFAULT_TURRET_SECTORS,
            mantlet_front_z: 1.38,
        },
        gun: GunShape {
            // Fire line 2.25 mid-face; the PaK 44's muzzle reaches z = 6.75 (10.65 m overall,
            // gun forward) — the longest reach in the lineup, muzzle honestly plain.
            trunnion_y: 2.25,
            trunnion_z: 1.30,
            muzzle_z: 6.75,
            barrel_radius: 0.135,
            evacuator: None,
            // None (dossier PR-JT.1): the Aberdeen reference and the bulk of service photos
            // show the PaK 44 with a PLAIN muzzle — the fleet brake pass (#234) had fitted a
            // double-baffle on spec alone. The recessed bore stays.
            muzzle_brake: None,
            segments: 14,
        },
        armor: ArmorShape {
            // The 250 mm face buys its protection with thickness, not slope: only 15° at the
            // casemate front, the hull glacis at 50°, and the unbroken 25° flank.
            hull_front: (50.0, 1.0),
            hull_side: (25.0, 1.0),
            hull_rear: (30.0, 1.0),
            turret_front: (15.0, 1.0),
            turret_side: (25.0, 1.0),
            turret_rear: (5.0, 1.0),
            turret_side_taper: None,
            hull_lower_front: None,
            hull_rear_knuckle: None,
            hull_bow_shelf: None,
            glacis_ports: [None, None],
        },
        visual_detail: None,
    }
}
