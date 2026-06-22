use ::terrain::{HeightMap, StaticCoverObject};
use game_core::{
    ArmorFacing, ArmorZone, HitboxProfile, ImpactSurface, MountFrames, TankId, TankSpec,
    VehicleKind,
};
use glam::Vec3;

/// Neutral tank hull for shell collision.
#[derive(Debug, Clone, Copy)]
pub struct TraceTank {
    pub id: TankId,
    pub position: Vec3,
    pub yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub hitbox: HitboxProfile,
    pub turret_ring_z_m: f32,
}

impl TraceTank {
    pub fn from_spec(
        id: TankId,
        position: Vec3,
        yaw_rad: f32,
        turret_yaw_rad: f32,
        spec: &TankSpec,
    ) -> Self {
        Self {
            id,
            position,
            yaw_rad,
            turret_yaw_rad,
            hitbox: spec.hitbox,
            turret_ring_z_m: spec.mounts.turret_ring.translation.z,
        }
    }

    pub fn for_kind(
        id: TankId,
        position: Vec3,
        yaw_rad: f32,
        turret_yaw_rad: f32,
        kind: VehicleKind,
    ) -> Self {
        Self {
            id,
            position,
            yaw_rad,
            turret_yaw_rad,
            hitbox: HitboxProfile::for_vehicle(kind),
            turret_ring_z_m: MountFrames::for_vehicle(kind).turret_ring.translation.z,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShellTraceWorld<'a> {
    pub tanks: &'a [TraceTank],
    pub blockers: &'a [TraceTank],
    pub heightmap: Option<&'a HeightMap>,
    pub cover: &'a [StaticCoverObject],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentImpact {
    Tank {
        id: TankId,
        facing: ArmorFacing,
        zone: ArmorZone,
        impact_angle_degrees: f32,
        hit_position: Vec3,
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
