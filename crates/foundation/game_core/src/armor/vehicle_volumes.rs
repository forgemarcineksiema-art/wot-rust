//! Per-vehicle armor volumes baked from the [`crate::VehicleBlueprint`] — the same numbers the
//! visible plates are built from, so what you see is literally what you shoot. Migration is
//! per-vehicle exactly like the blueprint itself: vehicles without a blueprint return `None`
//! and keep the legacy hitbox-band model until they are migrated.
//!
//! All coordinates are hull-local around the HITBOX CENTER (the shell-trace frame): blueprint
//! ground-relative heights get `hitbox_center_y` subtracted at bake. The turret volume is
//! authored in the same frame and rotates about the ring axis at trace time.

use std::sync::OnceLock;

use glam::{Mat3, Vec3};

use super::ArmorZone;
use super::volumes::{ArmorPatch, ArmorVolume, TaggedPlane};
use crate::vehicle_blueprint::TurretForm;
use crate::{VehicleBlueprint, VehicleKind};

/// The armor shape of one vehicle: convex hull volumes (body plates + running gear) in the
/// hull frame, plus the turret volume that traverses about `turret_ring_z`.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleArmorVolumes {
    pub hull: Vec<ArmorVolume>,
    pub turret: ArmorVolume,
    /// The commander's cupola drum, PROUD of the turret roof as its own convex volume (turret
    /// frame, so it traverses). Before it existed, a shell aimed at the drum sailed over the
    /// casting and resolved against the ROOF at a grazing angle — auto-bounce off the most
    /// famous weakspot in tank warfare. Fleet-wide from blueprint cupola data: no vehicle can
    /// opt out of having the drum it visibly carries.
    pub cupola: ArmorVolume,
    pub turret_ring_z: f32,
}

/// The baked volumes for `kind`, or `None` for vehicles not yet migrated onto the blueprint.
/// Baked once per process — the shell trace asks for these on every segment.
pub fn vehicle_armor_volumes(kind: VehicleKind) -> Option<&'static VehicleArmorVolumes> {
    static T54: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static IS3: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static TIGER_I: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static TIGER_II: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static JAGDTIGER: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static PANTHER_II: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static CENTURION: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    static T34_85: OnceLock<Option<VehicleArmorVolumes>> = OnceLock::new();
    let cell = match kind {
        VehicleKind::T54_1951 => &T54,
        VehicleKind::IS3 => &IS3,
        VehicleKind::TigerI => &TIGER_I,
        VehicleKind::TigerII => &TIGER_II,
        VehicleKind::Jagdtiger => &JAGDTIGER,
        VehicleKind::PantherII => &PANTHER_II,
        VehicleKind::Centurion => &CENTURION,
        VehicleKind::T34_85 => &T34_85,
        // The test-only prototype has no blueprint (`blueprint_ron` returns `None` for it), so
        // there is nothing to bake. Listed EXHAUSTIVELY rather than swept up by a wildcard: a new
        // `VehicleKind` must fail to compile here until someone decides whether it has armour.
        //
        // What the wildcard cost: a vehicle missing from this table returns `None`, and
        // `sim::shell_trace::tank` then resolves it against BOX BANDS instead of its convex
        // volumes (`tank.rs:54`). No panic, no log, no red test — a hull shipping with the game's
        // central promise quietly switched off. The fleet loops that should have caught it
        // `continue` past a `None` instead of failing, so they never did.
        VehicleKind::PrototypeMedium => return None,
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
/// Dome tessellation is AUTHORED, per casting: see [`crate::TurretShape::sector_count`].
/// An EXTERNAL ball mantlet stands proud of the casting and its rim is mantlet too, so its patch
/// reaches slightly past the mantlet radius to catch the socket lip — the visible ball's
/// footprint.
const EXTERNAL_MANTLET_PATCH_SCALE: f32 = 1.2;

/// How far past the mantlet radius the gameplay patch reaches, for THIS mount.
///
/// An internal mantlet has no rim outside to catch. Its aperture edge is precisely where mantlet
/// stops and turret front starts, and inflating the patch past it would hand the gun mount 18%
/// of armour across a band of casting that is, in fact, just casting.
///
/// The two cases are told apart by the metal, not by a flag: an external mantlet's own front face
/// reaches PAST the casting around it, and an internal one does not. Blueprints with no authored
/// casting keep the external scale, which is what every welded-box mount in this fleet is.
fn mantlet_patch_scale(blueprint: &VehicleBlueprint) -> f32 {
    // Both PARTS must be authored to prove an internal mount (F5.i: the slot holds optional
    // parts): the mantlet profile says where the mount's metal ends, the loft says where the
    // casting's face is. A vehicle authoring only a gun group keeps the external scale until
    // its turret is authored too — the conservative reading of partial data.
    let (Some(gun), Some(loft)) = (
        blueprint.visual_detail().and_then(|hybrid| hybrid.gun.as_ref()),
        blueprint.visual_detail().and_then(|hybrid| hybrid.turret_loft.as_ref()),
    ) else {
        return EXTERNAL_MANTLET_PATCH_SCALE;
    };
    let face_z = blueprint.gun.trunnion_z - blueprint.turret.ring_z
        + gun.mantlet_profile.iter().map(|(z, _)| *z).fold(f32::NEG_INFINITY, f32::max);
    if face_z < loft.support(Vec3::Z) { 1.0 } else { EXTERNAL_MANTLET_PATCH_SCALE }
}

fn bake_vehicle_armor(blueprint: VehicleBlueprint) -> VehicleArmorVolumes {
    let cy = blueprint.hull.hitbox_center_y;
    let mut hull = vec![
        upper_hull(&blueprint, cy),
        lower_hull(&blueprint, cy),
        track(&blueprint, cy, 1.0, ArmorZone::RightTrack),
        track(&blueprint, cy, -1.0, ArmorZone::LeftTrack),
    ];
    if blueprint.hull.skirt.is_some() {
        hull.push(skirt(&blueprint, cy, 1.0));
        hull.push(skirt(&blueprint, cy, -1.0));
    }
    // The turret volume follows the blueprint's construction: a cast dome sweeps sector planes
    // around the casting; a welded box or fixed casemate is a faceted plate prism.
    let turret = match blueprint.turret.form {
        TurretForm::CastDome => turret_dome(&blueprint, cy),
        TurretForm::WeldedBox | TurretForm::Casemate => turret_prism(&blueprint, cy),
    };
    VehicleArmorVolumes {
        hull,
        turret,
        cupola: cupola_drum(&blueprint, cy),
        turret_ring_z: blueprint.turret.ring_z,
    }
}

/// The commander's cupola as a convex sector prism in the TURRET frame: eight wall planes
/// around the drum's authored footprint, a lid at its authored proud height, and a floor at
/// the roof line it roots into. Every number is the blueprint's — the same drum the loft
/// draws and the silhouette tests measure, so what the eye aims at is what the shell meets.
fn cupola_drum(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let turret = &blueprint.turret;
    // HULL-frame coordinates, like every turret plane here: the trace rotates the segment
    // about the ring PIVOT, which leaves z on the hull scale (T-34-85's off-zero ring caught
    // the subtraction this comment replaces).
    let center = Vec3::new(turret.cupola_x, 0.0, turret.cupola_z);
    let top_y = turret.roof_y + turret.cupola_proud_m(&blueprint.hull) - cy;
    let root_y = turret.roof_y - 0.05 - cy;
    let mut planes = Vec::with_capacity(10);
    const SECTORS: usize = 8;
    for sector in 0..SECTORS {
        let azimuth = (sector as f32 + 0.5) / SECTORS as f32 * std::f32::consts::TAU;
        let normal = Vec3::new(azimuth.cos(), 0.0, azimuth.sin());
        planes.push(TaggedPlane::new(
            normal,
            center + normal * turret.cupola_radius,
            ArmorZone::Cupola,
        ));
    }
    planes.push(TaggedPlane::new(Vec3::Y, Vec3::new(0.0, top_y, 0.0), ArmorZone::Cupola));
    planes.push(TaggedPlane::new(Vec3::NEG_Y, Vec3::new(0.0, root_y, 0.0), ArmorZone::Cupola));
    ArmorVolume { planes }
}

/// The upper hull: glacis, deck, upper sides, upper rear — everything above the sponson step.
fn upper_hull(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let hull = blueprint.hull;
    let side_slope = blueprint.armor.hull_side.0.to_radians();
    let (glacis_sin, glacis_cos) = blueprint.armor.hull_front.0.to_radians().sin_cos();
    let (rear_sin, rear_cos) = blueprint.armor.hull_rear.0.to_radians().sin_cos();
    let step_y = hull.sponson_y - cy;
    // The hull's forwardmost point at the sponson step: the fold of the two-plate front (and
    // the pike's central ridge, when the bow is a pike).
    let fold = Vec3::new(0.0, step_y, hull.half_len);
    let mut planes = Vec::with_capacity(8);
    // A pike nose is TWO glacis planes yawed about the central ridge; a flat bow is one.
    for yaw_deg in bow_sweeps(hull.pike_sweep_deg) {
        let normal =
            Mat3::from_rotation_y(yaw_deg.to_radians()) * Vec3::new(0.0, glacis_sin, glacis_cos);
        planes.push(TaggedPlane::new(normal, fold, ArmorZone::UpperGlacis));
    }
    planes.extend([
        TaggedPlane::new(Vec3::Y, Vec3::new(0.0, hull.deck_y - cy, 0.0), ArmorZone::HullDeck),
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
    ]);
    ArmorVolume { planes }
}

/// The bow's plan-view yaws: two swept faces for a pike nose, one flat plate otherwise.
fn bow_sweeps(pike_sweep_deg: f32) -> Vec<f32> {
    if pike_sweep_deg > 0.0 { vec![pike_sweep_deg, -pike_sweep_deg] } else { vec![0.0] }
}

/// The lower hull tub: nose plate, belly, tub sides, lower rear — below the sponson step.
fn lower_hull(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let hull = blueprint.hull;
    let lower_slope = (blueprint.armor.hull_front.0 * LOWER_PLATE_SLOPE_FACTOR).to_radians();
    let (rear_sin, rear_cos) = blueprint.armor.hull_rear.0.to_radians().sin_cos();
    let step_y = hull.sponson_y - cy;
    let belly_y = hull.belly_y - cy;
    let fold = Vec3::new(0.0, step_y, hull.half_len);
    let mut planes = Vec::with_capacity(8);
    // The nose rakes back going DOWN: its outward normal tips forward-and-down. A pike bow
    // carries its sweep into the lower plates too.
    for yaw_deg in bow_sweeps(hull.pike_sweep_deg) {
        let normal = Mat3::from_rotation_y(yaw_deg.to_radians())
            * Vec3::new(0.0, -lower_slope.sin(), lower_slope.cos());
        planes.push(TaggedPlane::new(normal, fold, ArmorZone::LowerPlate));
    }
    planes.extend([
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
    ]);
    ArmorVolume { planes }
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
    // The authored casting, when there is one: the armour follows the metal rather than a circle
    // drawn near it.
    let loft = blueprint.visual_detail().and_then(|hybrid| hybrid.turret_loft.as_ref());
    let mantlet = ArmorPatch {
        zone: ArmorZone::Mantlet,
        center: Vec3::new(0.0, blueprint.gun.trunnion_y - cy, turret.mantlet_front_z),
        radius_m: turret.mantlet_radius * mantlet_patch_scale(blueprint),
    };
    let sectors = turret.sector_count.max(3) as usize;
    let mut planes = Vec::with_capacity(sectors + 2);
    for sector in 0..sectors {
        let azimuth = (sector as f32 + 0.5) / sectors as f32 * std::f32::consts::TAU;
        let forward_off = crate::math::wrap_angle(azimuth).abs().to_degrees();
        let (zone, slope_deg) = if forward_off <= TURRET_FRONT_AZIMUTH_DEG {
            (ArmorZone::TurretFront, turret.front_slope_deg)
        } else if forward_off >= TURRET_REAR_AZIMUTH_DEG {
            (ArmorZone::TurretRear, turret.rear_slope_deg)
        } else {
            (ArmorZone::TurretSide, turret.side_slope_deg)
        };
        // A cast wall thins as it runs aft. Where the blueprint documents that taper, each side
        // sector carries its own share of the wall instead of the whole flank quoting one
        // number that is right nowhere: at the cheek boundary the sector is the full side, at
        // the rear boundary it has thinned to the taper.
        let thickness_scale = (zone == ArmorZone::TurretSide)
            .then_some(blueprint.armor.turret_side_taper)
            .flatten()
            .map(|taper| {
                let span = TURRET_REAR_AZIMUTH_DEG - TURRET_FRONT_AZIMUTH_DEG;
                let t = ((forward_off - TURRET_FRONT_AZIMUTH_DEG) / span).clamp(0.0, 1.0);
                1.0 + (taper - 1.0) * t
            });
        let (slope_sin, slope_cos) = slope_deg.to_radians().sin_cos();
        let direction = Vec3::new(azimuth.sin(), 0.0, azimuth.cos());
        let normal = Vec3::new(direction.x * slope_cos, slope_sin, direction.z * slope_cos);
        // Where the plane sits. When the vehicle authors its casting as a loft, ask the CASTING:
        // the support function puts the plane exactly on the furthest metal in this direction, so
        // nothing visible falls outside the armour and no armour stands in air.
        //
        // The circle it replaces was wrong in both directions at once on a T-54. Its cheeks — the
        // whole point of the vehicle — bulge to 1.37 m at the shoulder while the swept radius was
        // 1.12: a third of a metre of the most-shot-at steel in the game was outside its own
        // armour volume, and shells went through it. Meanwhile the flanks, narrower than the
        // circle, stopped shells in air.
        //
        // Vehicles with no authored loft keep the swept radius until their own dossier arrives
        // (Genialna Flota) — this is not the PR that silently moves seven vehicles' armour.
        let anchor = match loft {
            Some(loft) => normal * (loft.support(normal) - normal.y * cy),
            None => center + direction * turret.base_radius,
        };
        let mut plane = TaggedPlane::new(normal, anchor, zone);
        if let Some(scale) = thickness_scale {
            plane = plane.with_thickness_scale(scale);
        }
        if zone == ArmorZone::TurretFront && plane.normal.z > 1.0e-3 {
            // The patch center is where the GUN LINE meets this sector plane.
            //
            // It used to be the gun-axis point slid along the plane's NORMAL, which is not the
            // same thing and is not what the old comment claimed it was. A dome's sector plane is
            // tangent to the casting at its furthest point, which on a T-54 is up on the shoulder
            // — so at the gun's height the plane stands 0.29 m ahead of the metal, and sliding
            // along a normal raked 35 degrees walked the patch 0.17 m UP the plate, off the gun
            // line entirely. A shot straight down the barrel then landed 0.21 m from the centre
            // of its own mantlet, and only counted as one because the patch was 0.38 m wide.
            //
            // Follow the gun instead and the shot lands dead centre whatever the sector tilt,
            // which is what lets the patch be the size of the real aperture.
            let z = (plane.offset - plane.normal.y * mantlet.center.y) / plane.normal.z;
            let on_gun_line = Vec3::new(0.0, mantlet.center.y, z);
            plane = plane.with_patches(vec![ArmorPatch { center: on_gun_line, ..mantlet }]);
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

/// A welded box turret or a fixed casemate: four sloped plates (front, two sides, rear — angles
/// from the blueprint's turret slopes), a flat roof and the underside. Unlike the swept dome,
/// the normal a shell meets is the PLATE's normal across its whole face — flat German steel
/// behaves like flat steel, and angling the hull is what changes the presented angle. The
/// mantlet patch rides the front plate exactly like the dome's front sectors. A casemate never
/// traverses in practice (its spec clamps yaw), so the same prism serves both forms.
fn turret_prism(blueprint: &VehicleBlueprint, cy: f32) -> ArmorVolume {
    let turret = blueprint.turret;
    let ring_y = turret.ring_y - cy;
    let mantlet = ArmorPatch {
        zone: ArmorZone::Mantlet,
        center: Vec3::new(0.0, blueprint.gun.trunnion_y - cy, turret.mantlet_front_z),
        radius_m: turret.mantlet_radius * mantlet_patch_scale(blueprint),
    };
    let (front_sin, front_cos) = turret.front_slope_deg.to_radians().sin_cos();
    let (side_sin, side_cos) = turret.side_slope_deg.to_radians().sin_cos();
    let (rear_sin, rear_cos) = turret.rear_slope_deg.to_radians().sin_cos();
    let front_point = Vec3::new(0.0, ring_y, turret.ring_z + turret.plan_half_length);
    let mut front =
        TaggedPlane::new(Vec3::new(0.0, front_sin, front_cos), front_point, ArmorZone::TurretFront);
    // The patch center projected onto the front plate, so a shot down the gun line always lands
    // inside the mantlet whatever the plate's slope (the dome uses the same trick per sector).
    let on_plane =
        mantlet.center - front.normal * (front.normal.dot(mantlet.center) - front.offset);
    front = front.with_patches(vec![ArmorPatch { center: on_plane, ..mantlet }]);
    ArmorVolume {
        planes: vec![
            front,
            TaggedPlane::new(
                Vec3::new(side_cos, side_sin, 0.0),
                Vec3::new(turret.plan_half_width, ring_y, turret.ring_z),
                ArmorZone::TurretSide,
            ),
            TaggedPlane::new(
                Vec3::new(-side_cos, side_sin, 0.0),
                Vec3::new(-turret.plan_half_width, ring_y, turret.ring_z),
                ArmorZone::TurretSide,
            ),
            TaggedPlane::new(
                Vec3::new(0.0, rear_sin, -rear_cos),
                Vec3::new(0.0, ring_y, turret.ring_z - turret.plan_half_length),
                ArmorZone::TurretRear,
            ),
            TaggedPlane::new(
                Vec3::Y,
                Vec3::new(0.0, turret.roof_y - cy, turret.ring_z),
                ArmorZone::Roof,
            ),
            // The box's underside at the ring seat: reachable only from under the overhang.
            TaggedPlane::new(
                -Vec3::Y,
                Vec3::new(0.0, ring_y, turret.ring_z),
                ArmorZone::TurretSide,
            ),
        ],
    }
}

/// One side skirt as a thin spaced layer OUTSIDE the track band: full authored span, tagged
/// [`ArmorZone::Skirt`] so the resolver treats it as a standoff screen (deadly to HEAT jets),
/// never as running gear — a hit on the skirt must not degrade the track.
fn skirt(blueprint: &VehicleBlueprint, cy: f32, side: f32) -> ArmorVolume {
    let shape = blueprint.hull.skirt.expect("caller checked skirt presence");
    let outer_x = blueprint.track.outer_x + shape.standoff_m;
    let zone = ArmorZone::Skirt;
    ArmorVolume {
        planes: vec![
            TaggedPlane::new(
                Vec3::X * side,
                Vec3::new(side * (outer_x + shape.thickness_m), 0.0, 0.0),
                zone,
            ),
            TaggedPlane::new(-Vec3::X * side, Vec3::new(side * outer_x, 0.0, 0.0), zone),
            TaggedPlane::new(Vec3::Y, Vec3::new(0.0, shape.top_y - cy, 0.0), zone),
            TaggedPlane::new(-Vec3::Y, Vec3::new(0.0, shape.bottom_y - cy, 0.0), zone),
            TaggedPlane::new(Vec3::Z, Vec3::new(0.0, 0.0, shape.front_z), zone),
            TaggedPlane::new(-Vec3::Z, Vec3::new(0.0, 0.0, shape.rear_z), zone),
        ],
    }
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
    fn a_welded_box_turret_bakes_a_plate_prism_not_a_dome() {
        // The plumbing for the German fleet: switch the T-54's blueprint to a welded box and the
        // turret volume becomes four sloped PLATES (+roof/underside) whose normals carry the
        // blueprint slopes — flat steel behaves like flat steel, no swept sectors.
        let mut blueprint =
            VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        blueprint.turret.form = TurretForm::WeldedBox;
        let cy = blueprint.hull.hitbox_center_y;
        let volumes = bake_vehicle_armor(blueprint);

        assert_eq!(volumes.turret.planes.len(), 6, "front + 2 sides + rear + roof + underside");
        let headings: Vec<f32> = volumes
            .turret
            .planes
            .iter()
            .filter(|plane| plane.normal.y.abs() < 0.9)
            .map(|plane| plane.normal.x.atan2(plane.normal.z))
            .collect();
        assert_eq!(headings.len(), 4, "exactly four wall plates, not a swept sector ring");

        // The front plate carries the blueprint's front slope in its normal, and a shot down the
        // gun line still lands on the mantlet patch.
        let front = volumes
            .turret
            .planes
            .iter()
            .find(|plane| plane.zone == ArmorZone::TurretFront)
            .expect("front plate");
        let slope = front.normal.y.asin().to_degrees();
        assert!(
            (slope - blueprint.turret.front_slope_deg).abs() < 1.0e-3,
            "front plate slope from the normal: {slope}"
        );
        let gun_y = blueprint.gun.trunnion_y - cy;
        let (t, plane) = segment_volume_entry(
            Vec3::new(0.0, gun_y, 8.0),
            Vec3::new(0.0, gun_y, 0.0),
            &volumes.turret,
        )
        .expect("front plate hit");
        let hit = Vec3::new(0.0, gun_y, 8.0).lerp(Vec3::new(0.0, gun_y, 0.0), t);
        assert_eq!(volumes.turret.planes[plane].zone_at(hit), ArmorZone::Mantlet);

        // A casemate bakes the same prism (its traverse clamp lives in the spec, not the shape).
        let mut casemate = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        casemate.turret.form = TurretForm::Casemate;
        assert_eq!(bake_vehicle_armor(casemate).turret.planes.len(), 6);
    }

    #[test]
    fn an_authored_skirt_bakes_a_spaced_screen_pair_outside_the_tracks() {
        let mut blueprint =
            VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        let bare = bake_vehicle_armor(blueprint);
        assert_eq!(bare.hull.len(), 4, "no skirt volumes without an authored skirt");

        blueprint.hull.skirt = Some(crate::SkirtShape {
            top_y: 1.3,
            bottom_y: 0.6,
            front_z: 2.8,
            rear_z: -2.8,
            standoff_m: 0.12,
            thickness_m: 0.008,
        });
        let skirted = bake_vehicle_armor(blueprint);
        assert_eq!(skirted.hull.len(), 6, "a skirt adds one thin volume per side");
        let skirt = &skirted.hull[4];
        assert!(skirt.planes.iter().all(|plane| plane.zone == ArmorZone::Skirt));
        // The plate hangs OUTSIDE the track band by the authored standoff — the same plane the
        // visible sheet is built on, and thin like sheet metal.
        let outer = skirt.planes.iter().find(|plane| plane.normal.x > 0.9).expect("outer face");
        assert!(
            (outer.offset - (blueprint.track.outer_x + 0.12 + 0.008)).abs() < 1.0e-3,
            "skirt face at track outer + standoff + thickness, got {}",
            outer.offset
        );
    }

    /// The tessellation is CONTENT, and this is the proof: change the blueprint and the armour
    /// follows. It used to be `if kind == T54_1951` inside the shared bake, so a new casting
    /// could not be authored finer without editing the engine — and no author could see that the
    /// decision existed at all.
    #[test]
    fn the_dome_sweep_follows_the_blueprint_not_the_vehicle_kind() {
        let mut blueprint =
            VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        for sectors in [6_u32, 12, 24, 40] {
            blueprint.turret.sector_count = sectors;
            let volumes = bake_vehicle_armor(blueprint);
            let walls =
                volumes.turret.planes.iter().filter(|plane| plane.normal.y.abs() < 0.9).count();
            assert_eq!(
                walls, sectors as usize,
                "{sectors} authored sectors must bake {sectors} wall planes"
            );
        }
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
        let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        assert_eq!(
            headings.len(),
            blueprint.turret.sector_count as usize,
            "the sweep is as fine as the BLUEPRINT says, not as fine as the bake decided"
        );
        assert!(
            blueprint.turret.sector_count > crate::vehicle_blueprint::DEFAULT_TURRET_SECTORS,
            "the T-54 casting is authored finer than the default"
        );
        for pair in headings.windows(2) {
            assert!((pair[1] - pair[0]).abs() > 1.0e-3, "sector headings must differ");
        }
    }
}
