//! Per-vehicle blueprint data. The SOURCE is the RON file per vehicle
//! (`crates/foundation/game_core/blueprints/<slug>.blueprint.ron`, loaded by `source.rs`);
//! migrated vehicles return `Some`, the rest return `None` and keep the legacy hand-authored
//! hitbox/mounts/armour + recipe until they are migrated.
//!
//! The old Rust constructors survive below as `#[cfg(test)]` GOLDEN FIXTURES: the transitional
//! `parsed_ron_equals_rust_fixture` test proves every parsed file is field-for-field identical
//! to the literals it was exported from, and the bake-hash goldens in `vehicle_geometry` are
//! the outer judge. Once the fleet has lived on RON for a while, the fixtures can be deleted.

use super::{VehicleBlueprint, source};
use crate::VehicleKind;

pub(super) fn blueprint(kind: VehicleKind) -> Option<VehicleBlueprint> {
    source::load_blueprint(kind)
}

// The T-54 obr. 1951 at its documented 1:1 dimensions (blueprint-verified): hull 6.04 m long,
// 3.27 m over the tracks, 2.40 m to the cupola top, 0.425 m ground clearance, 810 mm road wheels,
// 580 mm track, a ~2.25 m cast turret seated on the 1.75 m hull roof, and the D-10T reaching
// z = 5.95 (9.0 m overall length, gun forward). Every consumer reads these numbers, so the visible
// tank, the hitbox and the armour all describe the same vehicle the references show.
#[cfg(test)]
fn t54() -> VehicleBlueprint {
    use super::t54_hybrid::t54_hybrid;
    use super::{
        ArmorShape, GunShape, HullShape, ShoePattern, TrackShape, TurretForm, TurretShape,
        WheelFace,
    };
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
            pike_sweep_deg: 0.0,
            nose_rise: 0.06,
            rear_slope_deg: 8.0,
            // One box: the "tub" and the plate above the fold share the same vertical side plane.
            lower_half_width: 1.05,
            sponson_y: 1.00,
            skirt: None,
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
            // The road wheels carry the top run, so the axle IS the belt mid-height.
            axle_y: None,
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
            return_rollers: 0,
            roller_radius: 0.0,
            overlap_inner_dx: 0.0,
            wheel_half_width: 0.18,
            link_half_width: 0.23,
            link_count: Some(90),
            top_sag_m: 0.05,
            wheel_spokes: 6,
            drive_front: false,
            shoe_pattern: ShoePattern::Omsh,
            wheel_face: WheelFace::Openwork,
            suspension: super::SuspensionKind::TorsionBar,
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

#[cfg(test)]
mod golden_fixtures {
    use super::super::centurion::centurion_blueprint;
    use super::super::is3::is3_blueprint;
    use super::super::jagdtiger::jagdtiger_blueprint;
    use super::super::panther_ii::panther_ii_blueprint;
    use super::super::tiger_i::tiger_i_blueprint;
    use super::super::tiger_ii::tiger_ii_blueprint;
    use super::*;

    /// TRANSITIONAL migration lock: every parsed RON blueprint must equal, field for field, the
    /// Rust literal it was exported from. Together with the golden bake hashes this proves the
    /// RON flip changed no geometry. Delete the fixtures (and this test) once the fleet has
    /// lived on RON long enough that the files are the undisputed source.
    #[test]
    fn parsed_ron_equals_rust_fixture() {
        let fixtures: [(VehicleKind, VehicleBlueprint); 7] = [
            (VehicleKind::T54_1951, super::t54()),
            (VehicleKind::IS3, is3_blueprint()),
            (VehicleKind::TigerI, tiger_i_blueprint()),
            (VehicleKind::TigerII, tiger_ii_blueprint()),
            (VehicleKind::Jagdtiger, jagdtiger_blueprint()),
            (VehicleKind::PantherII, panther_ii_blueprint()),
            (VehicleKind::Centurion, centurion_blueprint()),
        ];
        for (kind, fixture) in fixtures {
            let parsed =
                blueprint(kind).unwrap_or_else(|| panic!("{kind:?} must load from its RON source"));
            assert_eq!(parsed, fixture, "{kind:?}: parsed RON diverges from the Rust fixture");
        }
    }
}
