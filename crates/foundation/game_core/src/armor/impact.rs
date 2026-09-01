//! One resolver for a traced impact (Inny Poziom A1).
//!
//! The server's verdict and the reticle's hint are the SAME function over the SAME query. Before
//! this the reticle called the bare zone resolver — no thickness scale, no spaced stack on a
//! track or skirt hit — so a 20 mm track band read green while the server charged band + belt +
//! side plate behind it: "green, then 0". Now whoever traced the contact (the sim from its
//! `TankState`, the client from the snapshot) gathers the same eight facts into a
//! [`TracedImpact`] and both hand it here. The seam that is left — snapshot against state — is
//! measured by the client's parity test over ten thousand traced impacts.

use glam::Vec3;

use super::resolve::{PenetrationResult, resolve_penetration_through_screens};
use super::zone::resolve_penetration_at_distance_on_zone_scaled;
use crate::math::{HullPose, plate_normal};
use crate::{ArmorFacing, ArmorProfile, ArmorZone, ShellSpec, TrackSide};

/// Everything the armour model needs to know about one contact, gathered by whoever traced it.
#[derive(Debug, Clone, Copy)]
pub struct TracedImpact<'a> {
    pub shell: &'a ShellSpec,
    pub armor: &'a ArmorProfile,
    /// The struck hull's attitude — the side plate's angle for the track stack lives in it.
    pub hull: HullPose,
    pub zone: ArmorZone,
    pub impact_angle_degrees: f32,
    pub distance_m: f32,
    /// The struck plate's share of its zone's thickness: 1.0 is exactly the zone's facet; a
    /// cast wall thins as it runs aft, so a flank hit resolves against the metal THERE.
    pub thickness_scale: f32,
    /// The shell's world direction at contact.
    pub direction: Vec3,
    /// Which belts still stand, `[left, right]` — a thrown belt lies on the ground beside the
    /// hull and stops screening; the sim never charges armour for steel the eye can see is gone.
    pub belts_present: [bool; 2],
}

/// Which flank a track/skirt contact met. The track zones say so outright; the skirt pair
/// shares one zone, so the struck plate is whichever side faces the shell's approach, resolved
/// in the hull frame.
pub fn struck_flank(hull: HullPose, direction: Vec3, zone: ArmorZone) -> TrackSide {
    match zone {
        ArmorZone::LeftTrack => TrackSide::Left,
        ArmorZone::RightTrack => TrackSide::Right,
        _ => {
            let local = hull.basis().transpose() * direction.normalize_or_zero();
            if local.x < 0.0 { TrackSide::Right } else { TrackSide::Left }
        }
    }
}

/// The `[left, right]` slot a side owns — the order `TrackHealth::hp_pair` and the snapshot's
/// `track_hp` both use.
pub const fn belt_index(side: TrackSide) -> usize {
    match side {
        TrackSide::Left => 0,
        TrackSide::Right => 1,
    }
}

/// The armour test for one traced contact. Ordinary zones test their single plate at the
/// struck spot's thickness; the track zones are a SPACED-ARMOR stack — the skirt (if struck)
/// and the belt (if it still stands) screen the hull side plate behind them, and each layer is
/// measured against its own true 3D normal, exactly like a direct side hit would be.
pub fn resolve_traced_impact(impact: &TracedImpact<'_>) -> PenetrationResult {
    if !matches!(impact.zone, ArmorZone::LeftTrack | ArmorZone::RightTrack | ArmorZone::Skirt) {
        return resolve_penetration_at_distance_on_zone_scaled(
            impact.shell,
            impact.armor,
            impact.zone,
            impact.impact_angle_degrees,
            impact.distance_m,
            impact.thickness_scale,
        );
    }
    let side = struck_flank(impact.hull, impact.direction, impact.zone);
    let side_sign = match side {
        TrackSide::Left => -1.0,
        TrackSide::Right => 1.0,
    };
    let side_slope = impact.armor.facet(ArmorFacing::HullSide).slope_degrees;
    let side_normal = plate_normal(impact.hull, 0.0, ArmorZone::HullSide, side_sign, side_slope);
    let direction = impact.direction.normalize_or_zero();
    let side_angle_degrees = (-direction).dot(side_normal).clamp(-1.0, 1.0).acos().to_degrees();

    // The spaced stack standing off this flank, OUTERMOST FIRST — the honest geometry of what
    // the shell actually crosses: a skirt hangs outside the belt, so a skirt hit still has the
    // belt behind it; a thrown belt is not there any more; an empty stack is a bare side plate.
    let belt_zone = match side {
        TrackSide::Left => ArmorZone::LeftTrack,
        TrackSide::Right => ArmorZone::RightTrack,
    };
    let mut screens = [ArmorZone::Skirt; 2];
    let mut count = 0;
    if impact.zone == ArmorZone::Skirt {
        screens[count] = ArmorZone::Skirt;
        count += 1;
    }
    if impact.belts_present[belt_index(side)] {
        screens[count] = belt_zone;
        count += 1;
    }
    resolve_penetration_through_screens(
        impact.shell,
        impact.armor,
        &screens[..count],
        impact.impact_angle_degrees,
        side_angle_degrees,
        impact.distance_m,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TankSpec, VehicleKind};

    fn t54() -> TankSpec {
        VehicleKind::T54_1951.spec()
    }

    fn impact<'a>(spec: &'a TankSpec, zone: ArmorZone, belts: [bool; 2]) -> TracedImpact<'a> {
        TracedImpact {
            shell: &spec.gun.shell,
            armor: &spec.hull,
            hull: HullPose::level(0.0),
            zone,
            impact_angle_degrees: 20.0,
            distance_m: 200.0,
            thickness_scale: 1.0,
            // Travelling toward +X in the hull frame: the shell meets the +X flank.
            direction: Vec3::X,
            belts_present: belts,
        }
    }

    /// A thrown belt stops screening: the same track-zone contact resolves against LESS steel
    /// once the belt lies on the ground, and against exactly the bare side plate.
    #[test]
    fn a_thrown_belt_stops_screening_the_side_plate() {
        let spec = t54();
        let standing = resolve_traced_impact(&impact(&spec, ArmorZone::LeftTrack, [true, true]));
        let thrown = resolve_traced_impact(&impact(&spec, ArmorZone::LeftTrack, [false, false]));
        assert!(
            thrown.effective_armor_mm < standing.effective_armor_mm,
            "belt gone: {} mm must be under belt standing: {} mm",
            thrown.effective_armor_mm,
            standing.effective_armor_mm
        );
        let bare = resolve_penetration_through_screens(
            &spec.gun.shell,
            &spec.hull,
            &[],
            20.0,
            {
                let normal = plate_normal(
                    HullPose::level(0.0),
                    0.0,
                    ArmorZone::HullSide,
                    -1.0,
                    spec.hull.facet(ArmorFacing::HullSide).slope_degrees,
                );
                (-Vec3::X).dot(normal).clamp(-1.0, 1.0).acos().to_degrees()
            },
            200.0,
        );
        assert_eq!(thrown.effective_armor_mm, bare.effective_armor_mm);
    }

    /// A skirt hit still has the belt behind it: the skirt stack is thicker than the bare belt
    /// stack on the same flank.
    #[test]
    fn a_skirt_hit_stacks_the_skirt_on_the_belt() {
        let spec = t54();
        let skirt = resolve_traced_impact(&impact(&spec, ArmorZone::Skirt, [true, true]));
        let belt = resolve_traced_impact(&impact(&spec, ArmorZone::LeftTrack, [true, true]));
        assert!(skirt.effective_armor_mm >= belt.effective_armor_mm);
    }

    /// The thickness scale is the metal at the struck spot: half the scale, less armour.
    #[test]
    fn the_thickness_scale_thins_the_plate_at_the_spot() {
        let spec = t54();
        let full = resolve_traced_impact(&impact(&spec, ArmorZone::TurretSide, [true, true]));
        let thin = resolve_traced_impact(&TracedImpact {
            thickness_scale: 0.5,
            ..impact(&spec, ArmorZone::TurretSide, [true, true])
        });
        assert!(thin.effective_armor_mm < full.effective_armor_mm);
    }

    /// The skirt's struck flank is whichever side faces the shell's approach, in the hull frame:
    /// a shell travelling toward +X meets the +X (left) flank, and the hull's yaw is honoured.
    #[test]
    fn the_skirts_flank_is_the_one_facing_the_shell() {
        let level = HullPose::level(0.0);
        assert_eq!(struck_flank(level, Vec3::X, ArmorZone::Skirt), TrackSide::Left);
        assert_eq!(struck_flank(level, -Vec3::X, ArmorZone::Skirt), TrackSide::Right);
        // Yawed a half turn, the same world direction meets the other flank.
        let turned = HullPose::level(std::f32::consts::PI);
        assert_eq!(struck_flank(turned, Vec3::X, ArmorZone::Skirt), TrackSide::Right);
        // The track zones name their flank outright, whatever the approach.
        assert_eq!(struck_flank(level, Vec3::X, ArmorZone::RightTrack), TrackSide::Right);
    }
}
