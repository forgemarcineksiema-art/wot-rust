//! The running-gear contact footprint: where a vehicle's road wheels actually touch the ground,
//! in hull-local coordinates. The physics support envelope (trench bridging, crest overhang,
//! authoritative hull attitude) samples terrain at these stations, and they come from the same
//! blueprint `TrackShape` the rendered running gear is placed by — one source, so the wheels the
//! player sees are the wheels the hull rides on.

use crate::vehicle_blueprint::{TrackShape, VehicleBlueprint};
use crate::{HitboxProfile, VehicleKind};

/// Fallback station count for vehicles without a blueprint-authored running gear.
const FALLBACK_STATIONS: usize = 5;
/// Fallback wheelbase fraction of the hitbox half-length the stations span (tracks stop short of
/// the bumper-to-bumper hull ends).
const FALLBACK_RUN_FRACTION: f32 = 0.78;
/// Fallback road-wheel radius (m) — a medium-tank wheel.
const FALLBACK_WHEEL_RADIUS: f32 = 0.40;

/// Hull-local ground-contact stations of one vehicle's running gear, mirrored to both sides at
/// `±half_gauge_x`. Deterministic, renderer-free data the physics contact model consumes.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactFootprint {
    /// Z of each road-wheel axle (front positive), in `TrackShape::wheel_stations` order.
    pub station_zs: Vec<f32>,
    /// |x| of each track's contact centre line.
    pub half_gauge_x: f32,
    /// Road-wheel radius: the contact point sits this far below the axle line.
    pub wheel_radius: f32,
}

impl ContactFootprint {
    /// Footprint from a blueprint running gear — the authored path.
    pub fn from_track(track: &TrackShape) -> Self {
        Self {
            station_zs: track.wheel_stations(),
            half_gauge_x: (track.inner_x + track.outer_x) * 0.5,
            wheel_radius: track.wheel_radius,
        }
    }

    /// Footprint estimated from the collision hitbox — the fallback for vehicles whose running
    /// gear has not been blueprint-authored yet. Behaves like today's centre-probe hull with a
    /// reasonable five-wheel spread.
    pub fn from_hitbox(hitbox: &HitboxProfile) -> Self {
        let half_run = hitbox.half_length_m * FALLBACK_RUN_FRACTION;
        let step = 2.0 * half_run / (FALLBACK_STATIONS - 1) as f32;
        Self {
            station_zs: (0..FALLBACK_STATIONS).map(|i| -half_run + step * i as f32).collect(),
            half_gauge_x: hitbox.half_width_m * 0.85,
            wheel_radius: FALLBACK_WHEEL_RADIUS,
        }
    }

    /// Footprint for a vehicle kind: blueprint track when authored, hitbox estimate otherwise.
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        match VehicleBlueprint::for_vehicle(kind) {
            Some(blueprint) => Self::from_track(&blueprint.track),
            None => Self::from_hitbox(&HitboxProfile::for_vehicle(kind)),
        }
    }

    /// Half the distance between the first and last station — the support half-span the crest
    /// overhang and bridging behaviors are scaled by.
    pub fn half_run(&self) -> f32 {
        match (self.station_zs.first(), self.station_zs.last()) {
            (Some(first), Some(last)) => (last - first).abs() * 0.5,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t54_footprint_reads_the_blueprint_stations() {
        let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
        assert_eq!(footprint.station_zs, vec![-1.95, -1.03, -0.11, 0.81, 1.95]);
        assert!((footprint.half_gauge_x - 1.345).abs() < 1.0e-6);
        assert!((footprint.wheel_radius - 0.405).abs() < 1.0e-6);
        assert!((footprint.half_run() - 1.95).abs() < 1.0e-6);
    }

    #[test]
    fn an_even_spread_covers_first_to_last_wheel() {
        let footprint = ContactFootprint::for_vehicle(VehicleKind::T55A);
        assert_eq!(footprint.station_zs.len(), 5);
        assert!((footprint.station_zs[0] + 2.12).abs() < 1.0e-6);
        assert!((footprint.station_zs[4] - 2.12).abs() < 1.0e-6);
        // Even pitch between stations.
        let pitch = footprint.station_zs[1] - footprint.station_zs[0];
        for pair in footprint.station_zs.windows(2) {
            assert!((pair[1] - pair[0] - pitch).abs() < 1.0e-5);
        }
    }

    #[test]
    fn a_vehicle_without_a_blueprint_gets_a_hitbox_estimate() {
        let footprint = ContactFootprint::for_vehicle(VehicleKind::PrototypeMedium);
        let hitbox = HitboxProfile::for_vehicle(VehicleKind::PrototypeMedium);
        assert_eq!(footprint.station_zs.len(), 5);
        assert!(footprint.half_run() < hitbox.half_length_m);
        assert!(footprint.half_gauge_x < hitbox.half_width_m);
        assert!(footprint.wheel_radius > 0.0);
        // Stations are symmetric around the hull origin.
        let mid = footprint.station_zs[2];
        assert!(mid.abs() < 1.0e-5, "middle station centred, got {mid}");
    }
}
