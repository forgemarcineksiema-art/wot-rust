//! Per-vehicle armor volumes baked from the [`crate::VehicleBlueprint`] — the same numbers the
//! visible plates are built from, so what you see is literally what you shoot. Migration is
//! per-vehicle exactly like the blueprint itself: vehicles without a blueprint return `None`
//! and keep the legacy hitbox-band model until they are migrated.
//!
//! All coordinates are hull-local around the HITBOX CENTER (the shell-trace frame): blueprint
//! ground-relative heights get `hitbox_center_y` subtracted at bake. The turret volume is
//! authored in the same frame and rotates about the ring axis at trace time.

use std::sync::OnceLock;

use glam::Vec3;

use super::ArmorZone;
use super::volumes::{ArmorPatch, ArmorVolume, TaggedPlane};
use crate::{VehicleBlueprint, VehicleKind};

/// The armor shape of one vehicle: convex hull volumes (body plates + running gear) in the
/// hull frame, plus the turret volume that traverses about `turret_ring_z`.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleArmorVolumes {
    pub hull: Vec<ArmorVolume>,
    pub turret: ArmorVolume,
    pub turret_ring_z: f32,
}

/// The baked volumes for `kind`, or `None` for vehicles not yet migrated onto the blueprint.
/// Baked once per process — the shell trace asks for these on every segment.
pub fn vehicle_armor_volumes(kind: VehicleKind) -> Option<&'static VehicleArmorVolumes> {
    static T54: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static T55A: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    let cell = match kind {
        VehicleKind::T54_1951 => &T54,
        VehicleKind::T55A => &T55A,
        _ => return None,
    };
    cell.get_or_init(|| VehicleBlueprint::for_vehicle(kind).map(bake_vehicle_armor)).as_ref()
}

/// The lower-plate slope derives from the glacis exactly like the facet model's
/// [`crate::ArmorProfile::plate`] does, so both descriptions of the nose agree.
const LOWER_PLATE_SLOPE_FACTOR: f32 = 0.45;
/// Azimuth (degrees off the turret's forward axis) where the dome's front sectors end and the
/// rear sectors begin.
const TURRET_FRONT_AZIMUTH_DEG: f32 = 60.0;
const TURRET_REAR_AZIMUTH_DEG: f32 = 120.0;
/// Dome tessellation: one tagged plane per sector, so the impact normal sweeps around the
/// casting instead of snapping between three flat faces.
const TURRET_SECTORS: usize = 12;
/// The mantlet patch reaches slightly past the casting radius so the socket rim counts as
/// mantlet, matching the visible ball's footprint.
const MANTLET_PATCH_SCALE: f32 = 1.2;

fn bake_vehicle_armor(blueprint: VehicleBlueprint) -> VehicleArmorVolumes {
    let cy = blueprint.hull.hitbox_center_y;
    VehicleArmorVolumes {
        hull: vec![
            upper_hull(&blueprint, cy),
            lower_hull(&blueprint, cy),
            track(&blueprint, cy, 1.0, ArmorZone::RightTrack),
            track(&blueprint, cy, -1.0, ArmorZone::LeftTrack),
        ],
        turret: turret_dome(&blueprint, cy),
        turret_ring_z: blueprint.turret.ring_z,
    }
}

/// The upper hull: glacis, deck, upper sides, upper rear — everything above the sponson step.
fn upper_hull(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let hull = blueprint.hull;
    let side_slope = blueprint.armor.hull_side.0.to_radians();
    let (glacis_sin, glacis_cos) = blueprint.armor.hull_front.0.to_radians().sin_cos();
    let (rear_sin, rear_cos) = blueprint.armor.hull_rear.0.to_radians().sin_cos();
    let step_y = hull.sponson_y - cy;
    // The hull's forwardmost point at the sponson step: the fold of the two-plate front.
    let fold = Vec3::new(0.0, step_y, hull.half_len);
    ArmorVolume {
        planes: vec![
            TaggedPlane::new(Vec3::new(0.0, glacis_sin, glacis_cos), fold, ArmorZone::UpperGlacis),
            TaggedPlane::new(Vec3::Y, Vec3::new(0.0, hull.deck_y - cy, 0.0), ArmorZone::Roof),
            TaggedPlane::new(
                Vec3::new(side_slope.cos(), side_slope.sin(), 0.0),
                Vec3::new(hull.half_width, step_y, 0.0),
                ArmorZone::HullSide,
            ),
            TaggedPlane::new(
                Vec3::new(-side_slope.cos(), side_slope.sin(), 0.0),
                Vec3::new(-hull.half_width, step_y, 0.0),
                ArmorZone::HullSide,
            ),
            TaggedPlane::new(
                Vec3::new(0.0, rear_sin, -rear_cos),
                Vec3::new(0.0, step_y, -hull.half_len),
                ArmorZone::HullRear,
            ),
            // The sponson underside: reachable only from below the overhang.
            TaggedPlane::new(-Vec3::Y, Vec3::new(0.0, step_y, 0.0), ArmorZone::HullSide),
        ],
    }
}

/// The lower hull tub: nose plate, belly, tub sides, lower rear — below the sponson step.
fn lower_hull(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let hull = blueprint.hull;
    let lower_slope = (blueprint.armor.hull_front.0 * LOWER_PLATE_SLOPE_FACTOR).to_radians();
    let (rear_sin, rear_cos) = blueprint.armor.hull_rear.0.to_radians().sin_cos();
    let step_y = hull.sponson_y - cy;
    let belly_y = hull.belly_y - cy;
    let fold = Vec3::new(0.0, step_y, hull.half_len);
    ArmorVolume {
        planes: vec![
            // The nose rakes back going DOWN: its outward normal tips forward-and-down.
            TaggedPlane::new(
                Vec3::new(0.0, -lower_slope.sin(), lower_slope.cos()),
                fold,
                ArmorZone::LowerPlate,
            ),
            TaggedPlane::new(-Vec3::Y, Vec3::new(0.0, belly_y, 0.0), ArmorZone::LowerPlate),
            TaggedPlane::new(
                Vec3::X,
                Vec3::new(hull.lower_half_width, belly_y, 0.0),
                ArmorZone::HullSide,
            ),
            TaggedPlane::new(
                -Vec3::X,
                Vec3::new(-hull.lower_half_width, belly_y, 0.0),
                ArmorZone::HullSide,
            ),
            TaggedPlane::new(
                Vec3::new(0.0, -rear_sin, -rear_cos),
                Vec3::new(0.0, step_y, -hull.half_len),
                ArmorZone::HullRear,
            ),
            TaggedPlane::new(Vec3::Y, Vec3::new(0.0, step_y, 0.0), ArmorZone::HullSide),
        ],
    }
}

/// One track band as its real box from [`crate::TrackShape`]: between the tub and the outer
/// face, from ground contact to the belt's top run, over the full idler-to-sprocket span.
fn track(blueprint: &VehicleBlueprint, cy: f32, side: f32, zone: ArmorZone) -> ArmorVolume {
    let track = blueprint.track;
    let half_len = track.end_z + track.end_radius;
    let top_y = track.top_y + track.belt_half_thickness - cy;
    let bottom_y = track.bottom_y - cy;
    let outer = Vec3::new(side * track.outer_x, 0.0, 0.0);
    let inner = Vec3::new(side * track.inner_x, 0.0, 0.0);
    ArmorVolume {
        planes: vec![
            TaggedPlane::new(Vec3::X * side, outer, zone),
            TaggedPlane::new(-Vec3::X * side, inner, zone),
            TaggedPlane::new(Vec3::Y, Vec3::new(0.0, top_y, 0.0), zone),
            TaggedPlane::new(-Vec3::Y, Vec3::new(0.0, bottom_y, 0.0), zone),
            TaggedPlane::new(Vec3::Z, Vec3::new(0.0, 0.0, half_len), zone),
            TaggedPlane::new(-Vec3::Z, Vec3::new(0.0, 0.0, -half_len), zone),
        ],
    }
}

/// The cast dome: a ring of sloped sector planes around the ring center (front/side/rear slopes
/// from the blueprint), a flat roof, and the mantlet as a circular patch where the gun meets the
/// front sectors. The normal a shell meets sweeps around the casting — dome edges auto-glance,
/// the center presents its true slope.
fn turret_dome(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let turret = blueprint.turret;
    let ring_y = turret.ring_y - cy;
    let center = Vec3::new(0.0, ring_y, turret.ring_z);
    let mantlet = ArmorPatch {
        zone: ArmorZone::Mantlet,
        center: Vec3::new(0.0, blueprint.gun.trunnion_y - cy, turret.mantlet_front_z),
        radius_m: turret.mantlet_radius * MANTLET_PATCH_SCALE,
    };
    let mut planes = Vec::with_capacity(TURRET_SECTORS + 2);
    for sector in 0..TURRET_SECTORS {
        let azimuth = (sector as f32 + 0.5) / TURRET_SECTORS as f32 * std::f32::consts::TAU;
        let forward_off = crate::math::wrap_angle(azimuth).abs().to_degrees();
        let (zone, slope_deg) = if forward_off <= TURRET_FRONT_AZIMUTH_DEG {
            (ArmorZone::TurretFront, turret.front_slope_deg)
        } else if forward_off >= TURRET_REAR_AZIMUTH_DEG {
            (ArmorZone::TurretRear, turret.rear_slope_deg)
        } else {
            (ArmorZone::TurretSide, turret.side_slope_deg)
        };
        let (slope_sin, slope_cos) = slope_deg.to_radians().sin_cos();
        let direction = Vec3::new(azimuth.sin(), 0.0, azimuth.cos());
        let normal = Vec3::new(direction.x * slope_cos, slope_sin, direction.z * slope_cos);
        let mut plane = TaggedPlane::new(normal, center + direction * turret.base_radius, zone);
        if zone == ArmorZone::TurretFront {
            // The patch center is the gun-axis point PROJECTED onto this sector plane, so a
            // shot down the gun line always lands inside the mantlet, whatever the sector tilt.
            let on_plane =
                mantlet.center - plane.normal * (plane.normal.dot(mantlet.center) - plane.offset);
            plane = plane.with_patches(vec![ArmorPatch { center: on_plane, ..mantlet }]);
        }
        planes.push(plane);
    }
    planes.push(TaggedPlane::new(
        Vec3::Y,
        Vec3::new(0.0, turret.roof_y - cy, turret.ring_z),
        ArmorZone::Roof,
    ));
    // The casting's underside at the ring seat: reachable only from under the overhang.
    planes.push(TaggedPlane::new(-Vec3::Y, center, ArmorZone::TurretSide));
    ArmorVolume { planes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment_volume_entry;

    #[test]
    fn every_blueprint_vehicle_bakes_armor_volumes_and_legacy_does_not() {
        for spec in crate::known_tank_specs() {
            let has_blueprint = VehicleBlueprint::for_vehicle(spec.kind).is_some();
            assert_eq!(
                vehicle_armor_volumes(spec.kind).is_some(),
                has_blueprint,
                "{:?}: armor volumes must exist exactly for blueprint vehicles",
                spec.kind
            );
        }
    }

    #[test]
    fn the_t54_bake_reads_the_blueprint_plates() {
        let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("baked");
        let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        let cy = blueprint.hull.hitbox_center_y;

        // The upper hull's glacis plane carries the blueprint's 60° slope in its normal.
        let glacis = volumes.hull[0]
            .planes
            .iter()
            .find(|plane| plane.zone == ArmorZone::UpperGlacis)
            .expect("glacis plane");
        let slope = glacis.normal.y.asin().to_degrees();
        assert!(
            (slope - blueprint.armor.hull_front.0).abs() < 1.0e-3,
            "glacis slope from the normal: {slope}"
        );

        // The hull side sits at the REAL narrow hull, not at the hitbox width.
        let side = volumes.hull[0]
            .planes
            .iter()
            .find(|plane| plane.zone == ArmorZone::HullSide && plane.normal.x > 0.9)
            .expect("right side plane");
        assert!(
            (side.offset - blueprint.hull.half_width).abs() < 1.0e-3,
            "side plane at the 1.05 m hull wall, got offset {}",
            side.offset
        );

        // The track volume spans the blueprint's real belt box.
        let track_outer = volumes.hull[2]
            .planes
            .iter()
            .find(|plane| plane.zone == ArmorZone::RightTrack && plane.normal.x > 0.9)
            .expect("track outer face");
        assert!((track_outer.offset - blueprint.track.outer_x).abs() < 1.0e-3);

        // A flat shot down the gun line into the dome front lands on the mantlet patch; the
        // same shot a half meter to the side lands on plain turret front.
        let gun_y = blueprint.gun.trunnion_y - cy;
        let (t, plane) = segment_volume_entry(
            Vec3::new(0.0, gun_y, 8.0),
            Vec3::new(0.0, gun_y, 0.0),
            &volumes.turret,
        )
        .expect("dome front hit");
        let hit = Vec3::new(0.0, gun_y, 8.0).lerp(Vec3::new(0.0, gun_y, 0.0), t);
        assert_eq!(volumes.turret.planes[plane].zone_at(hit), ArmorZone::Mantlet);

        let (t, plane) = segment_volume_entry(
            Vec3::new(0.75, gun_y, 8.0),
            Vec3::new(0.75, gun_y, 0.0),
            &volumes.turret,
        )
        .expect("dome cheek hit");
        let hit = Vec3::new(0.75, gun_y, 8.0).lerp(Vec3::new(0.75, gun_y, 0.0), t);
        assert_eq!(volumes.turret.planes[plane].zone_at(hit), ArmorZone::TurretFront);
    }

    #[test]
    fn dome_sector_normals_sweep_around_the_casting() {
        let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("baked");
        // Every sector plane's horizontal direction points outward at its own azimuth: collect
        // the set of distinct horizontal headings — a swept casting, not three flat faces.
        let headings: Vec<f32> = volumes
            .turret
            .planes
            .iter()
            .filter(|plane| plane.normal.y < 0.9 && plane.normal.y > -0.9)
            .map(|plane| plane.normal.x.atan2(plane.normal.z))
            .collect();
        assert_eq!(headings.len(), TURRET_SECTORS);
        for pair in headings.windows(2) {
            assert!((pair[1] - pair[0]).abs() > 1.0e-3, "sector headings must differ");
        }
    }
}
