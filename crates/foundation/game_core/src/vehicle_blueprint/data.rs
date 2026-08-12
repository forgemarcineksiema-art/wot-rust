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
    let mut shape = VehicleBlueprint {
        kind: VehicleKind::T54_1951,
        hull: HullShape {
            half_len: 3.1175,
            // The T-54 hull is a NARROW box (~2.1 m over the vertical 80 mm side plates) riding
            // between fully exposed tracks — there are no Panther-style sponsons overhanging the
            // running gear. The tracks stand beside the hull nearly flush with its envelope, and
            // the fender shelves with their stowage carry the visual mass above them.
            // The tub is what FITS between the tracks, and that is arithmetic, not taste: gauge
            // 2.640 less track 0.580 leaves 2.060 m of clear space, so the box is 1.03 each side.
            // It was 1.05 — described in the visual as "the narrow ~2.1 m box" — which is 20 mm
            // wider than the space the documented running gear leaves for it. Register M9.
            half_width: 1.03,
            // Real ground clearance: 0.425 m. The earlier 0.10 dropped the belly to the dirt and
            // made the hull side read as a wall reaching the ground.
            belly_y: 0.425,
            // The LOW hull is the T-54's signature: the roof sits at ~1.58 (documented fire line
            // 1.79 with the gun bedded ~0.2 above the ring seat), leaving the tall ~0.7 m cast
            // dome to carry the rest of the 2.40 m silhouette.
            deck_y: 1.58,
            glacis_slope_deg: 60.0,
            pike_sweep_deg: 0.0,
            nose_rise: 0.06,
            rear_slope_deg: 17.0,
            // One box: the "tub" and the plate above the fold share the same vertical side plane.
            lower_half_width: 1.03,
            sponson_y: 1.00,
            skirt: None,
            hitbox_half_width: 1.75,
            // Realistic full height: top = center_y + half_height = 2.43 bounds the 2.40 m tank
            // (cupola apex ~2.42). Floor ~5 cm below ground (center_y = half_height - 0.05);
            // hull/turret split at world y = 1.58, the hull-roof plane.
            hitbox_half_height: 1.341,
            hitbox_half_length: 3.2675,
            hitbox_center_y: 1.19,
            hitbox_turret_min_y: 0.39,
        },
        track: TrackShape {
            // The 580 mm OMSh band on the documented 2.640 m gauge: 1.32 ± 0.29, so the outer
            // face sits at 1.61 and the fenders overhang it by 25 mm to the documented 3.27 m.
            // Road wheels are the documented 810 mm discs; the small idler and toothed sprocket
            // sit beyond the wheel span on raised axles, and the belt ramps up to them.
            // Track gauge 2.640 m (the distance between the two belts' centrelines), four sources
            // agreeing. It was 2.690. The documented set closes on itself: gauge 2.640 + track
            // 0.580 = 3.220 over the tracks, and 25 mm of fender overhang each side makes the
            // documented 3.270 over the fenders. Register M9.
            center_x: 1.32,
            belt_half_thickness: 0.13,
            top_y: 0.905,
            bottom_y: 0.045,
            // The road wheels carry the top run, so the axle IS the belt mid-height.
            axle_y: None,
            wheel_radius: 0.405,
            wheel_count: 5,
            // Ground-contact base 3.840 m: the first road wheel's centre to the last. It was
            // 3.900. The enlarged 1st-to-2nd gap keeps its ratio to the other three (1.24), which
            // is the recognition feature, not the absolute spacing.
            wheel_first_z: -1.92,
            wheel_last_z: 1.92,
            // The belt's centre line wraps at `end_radius + LINK_SEAT`, and the documented
            // engagement circle is the sprocket's 572.4 mm pitch diameter — 0.2862 m of radius.
            // 0.30 put the wrap at 0.32, which is why the tooth count came out 14 (the modernised
            // T-55 / Obj. 167 wheel) where the T-54 has 13. The count is not authored anywhere: it
            // is the number of link pitches around this circle, so fixing the circle fixes it.
            end_radius: 0.2662,
            end_z: 2.590,
            // Raised so the top run leaves the end wheels ABOVE the 0.88 m wheel tops — links
            // grinding the tyres z-fight. What is authored is that 0.96 m anchor, and it was
            // written as 0.66 against a 0.32 wrap. The wrap is the sprocket's real pitch circle
            // now, so keeping the anchor means moving this number with it: 0.96 − 0.2862. Written
            // as a bare 0.66 the anchor would have dropped 34 mm the moment the wrap was corrected,
            // silently — the middle of the span is governed by the wheels and would not have shown
            // it.
            end_y: 0.674,
            inner_x: 1.03,
            outer_x: 1.61,
            segments: 14,
            // 810 mm wheels at a ~0.92 m pitch with the T-54's signature wider gap between the
            // first and second road wheels at the front (+Z). Single source: the rendered
            // running gear and the physics contact footprint both read these stations.
            wheel_stations: Some(&[-1.92, -1.014, -0.108, 0.798, 1.92]),
            return_rollers: 0,
            roller_radius: 0.0,
            overlap_inner_dx: 0.0,
            wheel_half_width: 0.18,
            link_half_width: 0.23,
            link_count: Some(90),
            // No return rollers: the top run hangs on the road wheels and scallops between them.
            // See the note beside this number in `t54_1951.blueprint.ron`.
            top_sag_m: 0.075,
            // Twelve webs between twelve pairs of lightening holes: the obr. 1951 disc is the
            // stamped spider-web, not an openwork casting (dossier, part construction / S1b).
            wheel_spokes: 12,
            drive_front: false,
            shoe_pattern: ShoePattern::Omsh,
            wheel_face: WheelFace::SpiderWeb,
            suspension: super::SuspensionKind::TorsionBar,
            // The trailing arm's own geometry: None takes the family default
            // (0.26 / 0.13 for a torsion bar) until this vehicle's dossier says otherwise.
            arm_reach_m: None,
            arm_rise_m: None,
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
            roof_y: 2.40,
            front_slope_deg: 35.0,
            side_slope_deg: 25.0,
            rear_slope_deg: 10.0,
            cupola_x: -0.34,
            cupola_z: -0.10,
            // The commander's cupola is 624 mm across the outside (Tankograd). The model carried
            // 480 — a fifth too small on the one fitting that reads at every range, because it is
            // the thing standing above the roofline.
            cupola_radius: 0.312,
            cupola_height: Some(0.131),
            plan_half_width: 1.125,
            plan_half_length: 1.30,
            plan_front_pad: 0.0,
            // The APERTURE, not a ball: the documented opening is ~0.40 m across, so its half-
            // extent is 0.20. This used to be 0.32 (a 0.64 m mask) inflated another 20% by the
            // external-ball patch rule — a gun mount half again wider than the hole it sits in.
            mantlet_radius: 0.20,
            mantlet_back_z: 0.85,
            sector_count: 24,
            mantlet_front_z: 1.12,
        },
        gun: GunShape {
            // The documented fire line: the D-10T axis at 1.78 (~0.2 above the ring seat), and
            // the muzzle at z = 5.95 — the 2.9 m overhang past the bow (9.00 m gun forward).
            trunnion_y: 1.78,
            trunnion_z: 1.15,
            muzzle_z: 5.8475,
            barrel_radius: 0.092,
            evacuator: None,
            muzzle_brake: None,
            segments: 12,
        },
        armor: ArmorShape {
            hull_front: (60.0, 1.0),
            // The documented 80 mm side plates are VERTICAL — and the visible side carries the
            // same rake the penetration model reads ("what you see is what you shoot").
            hull_side: (0.0, 1.0),
            hull_rear: (17.0, 1.0),
            turret_front: (35.0, 1.0),
            turret_side: (25.0, 1.0),
            turret_rear: (10.0, 1.0),
            // 65 mm rear over the 160 mm side wall: the documented taper of the
            // obr. 1951 casting.
            turret_side_taper: Some(0.41),
            hull_lower_front: Some((55.0, 1.0)),
            hull_rear_knuckle: Some((1.20, 45.0)),
            glacis_ports: [Some(super::GlacisPort { x: 0.42, y: 1.15, radius_m: 0.11 }), None],
        },
        visual_detail: None,
    };
    // Derived from the shapes this very fixture declares — the hybrid tree may not restate them.
    shape.visual_detail = Some(t54_hybrid(&super::source::BlueprintFile::from_blueprint(&shape)));
    shape
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
            // The fixtures guard the SHAPES. The visual slot is attached by the loader from a
            // separate `<slug>.visual.ron` (F5.iii) and has its own locks in
            // `blueprint_source.rs` — a Rust fixture restating it would be a second source.
            assert_eq!(
                super::source::BlueprintFile::from_blueprint(&parsed),
                super::source::BlueprintFile::from_blueprint(&fixture),
                "{kind:?}: parsed RON diverges from the Rust fixture"
            );
        }
    }
}
