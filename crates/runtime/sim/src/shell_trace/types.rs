use ::terrain::{HeightMap, StaticCoverObject};
use game_core::math::HullPose;
use game_core::{
    ArmorFacing, ArmorProfile, ArmorZone, HitboxProfile, ImpactSurface, MountFrames, TankId,
    TankSpec, VehicleArmorVolumes, VehicleKind, vehicle_armor_volumes,
};
use glam::Vec3;

/// Neutral tank hull for shell collision. The hitbox rides the FULL hull pose: a tilted hull
/// tilts its collision volumes and armor normals with it. The armor profile rides along so the
/// impact angle is measured against the TRUE plate normal, slope included; blueprint vehicles
/// additionally carry their baked convex armor volumes, which replace the box bands entirely.
#[derive(Debug, Clone, Copy)]
pub struct TraceTank {
    pub id: TankId,
    pub position: Vec3,
    pub hull: HullPose,
    pub turret_yaw_rad: f32,
    pub hitbox: HitboxProfile,
    pub turret_ring_z_m: f32,
    pub armor: ArmorProfile,
    pub armor_volumes: Option<&'static VehicleArmorVolumes>,
    /// The turret has been blown off (ammo-rack detonation wreck): the trace skips the turret box
    /// / turret armor volume, so a shot that would have struck the turret passes over the hull —
    /// the collision truth matches the picture of a decapitated wreck. Live tanks are always
    /// `false`; only wreck blockers carry it (see `shell_step::trace_split_into`).
    pub turret_detached: bool,
}

impl TraceTank {
    pub fn from_spec(
        id: TankId,
        position: Vec3,
        hull: HullPose,
        turret_yaw_rad: f32,
        spec: &TankSpec,
    ) -> Self {
        Self {
            id,
            position,
            hull,
            turret_yaw_rad,
            hitbox: spec.hitbox,
            turret_ring_z_m: spec.mounts.turret_ring.translation.z,
            armor: spec.hull,
            armor_volumes: vehicle_armor_volumes(spec.kind),
            turret_detached: false,
        }
    }

    pub fn for_kind(
        id: TankId,
        position: Vec3,
        hull: HullPose,
        turret_yaw_rad: f32,
        kind: VehicleKind,
    ) -> Self {
        Self {
            id,
            position,
            hull,
            turret_yaw_rad,
            hitbox: HitboxProfile::for_vehicle(kind),
            turret_ring_z_m: MountFrames::for_vehicle(kind).turret_ring.translation.z,
            armor: kind.spec().hull,
            armor_volumes: vehicle_armor_volumes(kind),
            turret_detached: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShellTraceWorld<'a> {
    /// Radius of the swept projectile body. Zero keeps ray queries (camera/selection) exact;
    /// ballistic shell paths pass their physical caliber radius.
    pub projectile_radius_m: f32,
    pub tanks: &'a [TraceTank],
    pub blockers: &'a [TraceTank],
    pub heightmap: Option<&'a HeightMap>,
    pub cover: &'a [StaticCoverObject],
    /// The map's standing water: a shell crossing the surface over submerged ground dies there
    /// (`ImpactSurface::Water`). `None` is a dry map. The reticle preview passes the same value
    /// as the server so a previewed splash is never a hit the server resolves differently.
    pub water: Option<::terrain::WaterBody>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentImpact {
    Tank {
        id: TankId,
        facing: ArmorFacing,
        zone: ArmorZone,
        impact_angle_degrees: f32,
        hit_position: Vec3,
        /// World-space outward normal of the struck plate (slope + hull attitude + turret yaw
        /// folded in) — the reflection plane for a ricochet continuation.
        plate_normal: Vec3,
        /// The struck plate's share of its zone's thickness (1.0 = exactly the zone's facet).
        /// A cast turret's wall tapers, so WHERE on the flank a shell lands decides how much
        /// steel is actually there; without this the whole flank would resolve against one
        /// averaged number that is right nowhere.
        thickness_scale: f32,
    },
    Obstacle {
        position: Vec3,
        surface: ImpactSurface,
    },
}

impl SegmentImpact {
    pub fn point(self) -> Vec3 {
        match self {
            Self::Tank { hit_position, .. } => hit_position,
            Self::Obstacle { position, .. } => position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TraceOutcome {
    Tank {
        id: TankId,
        facing: ArmorFacing,
        zone: ArmorZone,
        impact_angle_degrees: f32,
        hit_position: Vec3,
        distance_m: f32,
    },
    Obstacle {
        position: Vec3,
        surface: ImpactSurface,
    },
    Expired(Vec3),
}

impl TraceOutcome {
    pub fn impact_point(self) -> Vec3 {
        match self {
            Self::Tank { hit_position, .. } => hit_position,
            Self::Obstacle { position, .. } => position,
            Self::Expired(point) => point,
        }
    }
}
