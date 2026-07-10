//! Track geometry for the legacy (non-blueprint) fleet, authored as [`TrackShape`] values so the
//! German vehicles ride the SAME wrapped-band + animated-gear pipeline the blueprint vehicles do
//! — wheels that spin and shoe links that scroll — without granting them a `VehicleBlueprint`
//! (which would silently switch their hitbox and armor paths off the legacy model).
//!
//! Dimensions are carried over 1:1 from the retired fused-mesh gear (`recipes::chassis`), so the
//! silhouette the fleet has always had is what now moves: same wheel radii, stations, and band
//! extents. The loop is a plain stadium (the belt wraps at the outermost road wheels), which is
//! also historically right for the Tiger/Panther families — no raised idlers, no return rollers.

use game_core::{TrackShape, VehicleKind};

/// The authored track for a legacy vehicle, or `None` for vehicles that either have a real
/// blueprint (T-54 family, IS-3, Tiger I) or deliberately keep the fused static gear (the
/// test-only prototype medium).
pub(crate) fn legacy_track_shape(kind: VehicleKind) -> Option<TrackShape> {
    let (wheel_radius, wheel_count, wheel_y, wheel_span_z, inner_x, outer_x) = match kind {
        // TIGER_GEAR_LONG: one more station on the longer King Tiger hull.
        VehicleKind::TigerII => (0.42, 9, 0.46, 3.30, 1.46, 1.92),
        // JAGDTIGER_GEAR: the longest run in the lineup, sitting lowest.
        VehicleKind::Jagdtiger => (0.42, 9, 0.42, 3.40, 1.50, 1.96),
        // PANTHER_GEAR: the largest wheels of the four.
        VehicleKind::PantherII => (0.46, 8, 0.44, 3.00, 1.38, 1.82),
        _ => return None,
    };
    Some(TrackShape {
        center_x: (inner_x + outer_x) * 0.5,
        belt_half_thickness: (outer_x - inner_x) * 0.5,
        // A stadium loop tangent to the wheels: belt lines at the wheel top and bottom, wrapping
        // at wheel radius around the outermost stations.
        top_y: wheel_y + wheel_radius,
        bottom_y: wheel_y - wheel_radius,
        wheel_radius,
        wheel_count,
        wheel_first_z: -wheel_span_z,
        wheel_last_z: wheel_span_z,
        end_radius: wheel_radius,
        end_z: wheel_span_z,
        end_y: wheel_y,
        inner_x,
        outer_x,
        segments: 12,
        // Even spread between the end stations — exactly the retired fused layout.
        wheel_stations: None,
        return_rollers: 0,
        roller_radius: 0.0,
        overlap_inner_dx: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_ANIMATED: [VehicleKind; 3] =
        [VehicleKind::TigerII, VehicleKind::Jagdtiger, VehicleKind::PantherII];

    #[test]
    fn every_legacy_animated_vehicle_gets_a_stadium_track() {
        for kind in LEGACY_ANIMATED {
            let track = legacy_track_shape(kind).expect("authored track");
            assert!(track.wheel_count >= 8, "{kind:?}");
            // Stadium invariants: the belt is tangent to the wheels and wraps at their radius.
            assert_eq!(track.end_radius, track.wheel_radius, "{kind:?}");
            assert_eq!(track.end_z, track.wheel_last_z, "{kind:?}");
            assert!(
                (track.top_y - track.bottom_y - 2.0 * track.wheel_radius).abs() < 1.0e-6,
                "{kind:?} belt lines must be tangent to the wheels"
            );
            assert!(track.inner_x < track.outer_x, "{kind:?}");
            // Even wheel spread: stations derive from the count, not an explicit table.
            assert_eq!(track.wheel_stations().len(), track.wheel_count, "{kind:?}");
            assert!(track.bottom_y > -0.05, "{kind:?} wheels sink into the ground");
        }
    }

    #[test]
    fn blueprint_vehicles_and_the_prototype_never_take_the_legacy_table() {
        for kind in [
            VehicleKind::T54_1951,
            VehicleKind::T55A,
            VehicleKind::IS3,
            VehicleKind::TigerI,
            VehicleKind::PrototypeMedium,
        ] {
            assert!(legacy_track_shape(kind).is_none(), "{kind:?}");
        }
    }
}
