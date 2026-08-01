use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{ArmorFrame, HitboxProfile, ModuleSlot, VehicleKind};

pub mod authoring;
mod centurion;
mod german;
mod intersections;
mod is3;
mod jagdtiger;
mod panther_ii;
mod t34_85;
mod t54;
mod tiger_i;
mod tiger_ii;

use authoring::HullEnvelope;

/// Stable identity of one physical component. It is intentionally separate from [`ModuleSlot`]:
/// several real objects (two fuel tanks, recoil cylinders, final drives) may feed one existing
/// HUD/repair slot without becoming one imaginary box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DamageComponentId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageComponentKind {
    Breech,
    RecoilMechanism,
    TurretDrive,
    AmmunitionRack,
    Radio,
    FuelTank,
    Engine,
    Transmission,
    FinalDrive,
    Suspension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageMaterial {
    Machinery,
    Ammunition,
    Fuel,
    Electronics,
    Driveline,
    SuspensionSteel,
}

impl DamageMaterial {
    pub fn resistance_mm(self, path_length_m: f32) -> f32 {
        let (base, per_meter) = match self {
            Self::Machinery => (9.0, 30.0),
            Self::Ammunition => (5.0, 18.0),
            Self::Fuel => (2.0, 8.0),
            Self::Electronics => (2.0, 10.0),
            Self::Driveline => (12.0, 40.0),
            Self::SuspensionSteel => (10.0, 34.0),
        };
        base + path_length_m.max(0.0) * per_meter
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamagePlane {
    /// Interior is `normal.dot(point) <= offset`.
    pub normal: Vec3,
    pub offset: f32,
}

/// Authored primitive in centered tank-local coordinates. These are real narrow-phase shapes,
/// not AABBs with nicer names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DamageShape {
    Obb { center: Vec3, half_extents: Vec3, yaw_rad: f32 },
    Cylinder { center: Vec3, axis: Vec3, half_length: f32, radius: f32 },
    Capsule { a: Vec3, b: Vec3, radius: f32 },
    Convex { planes: Vec<DamagePlane>, bounds_min: Vec3, bounds_max: Vec3 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageComponent {
    pub id: DamageComponentId,
    /// Pose frame that owns this component. Turret components are authored in the neutral turret
    /// frame and traced after the live turret pose has been removed from the shot segment.
    #[serde(default)]
    pub frame: ArmorFrame,
    pub kind: DamageComponentKind,
    pub slot: ModuleSlot,
    pub material: DamageMaterial,
    pub shape: DamageShape,
    pub priority: u8,
    pub requires_penetration: bool,
    /// Component-local damage multiplier; keeps a small radio distinct from the engine block.
    pub vulnerability: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleIntersection {
    pub component_id: DamageComponentId,
    pub kind: DamageComponentKind,
    pub slot: ModuleSlot,
    pub material: DamageMaterial,
    pub distance_t: f32,
    pub path_length_m: f32,
    pub priority: u8,
    pub vulnerability: f32,
}

/// Gameplay-owned component layout. Visual interiors consume the same transforms through
/// `components()`; Forge validates them but never invents gameplay placement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DamageLayout {
    components: Vec<DamageComponent>,
}

impl DamageLayout {
    /// The authored component layout for `kind`.
    ///
    /// Exhaustive on purpose. The wildcard this replaced handed every vehicle but the T-54 an
    /// EMPTY layout — a hull with nothing inside it, where a penetration could damage no module,
    /// start no fire and detonate no rack — and adding a tenth vehicle would have inherited that
    /// silence without anyone being asked.
    pub fn for_vehicle(kind: VehicleKind) -> Self {
        let Some(env) = HullEnvelope::of(kind) else {
            // The prototype medium is a test vessel with no blueprint and no asset file. It has
            // no interior because it has no documented anatomy to place one against — stated
            // here rather than reached by falling through a wildcard.
            debug_assert_eq!(
                kind,
                VehicleKind::PrototypeMedium,
                "every vehicle with a blueprint must own a component layout"
            );
            return Self::default();
        };
        match kind {
            VehicleKind::T54_1951 => Self::t54_1951(),
            VehicleKind::T34_85 => t34_85::layout(&env),
            VehicleKind::IS3 => is3::layout(&env),
            VehicleKind::Centurion => centurion::layout(&env),
            VehicleKind::TigerI => tiger_i::layout(&env),
            VehicleKind::TigerII => tiger_ii::layout(&env),
            VehicleKind::PantherII => panther_ii::layout(&env),
            VehicleKind::Jagdtiger => jagdtiger::layout(&env),
            VehicleKind::PrototypeMedium => Self::default(),
        }
    }

    pub fn t54_1951() -> Self {
        t54::layout()
    }

    pub fn components(&self) -> &[DamageComponent] {
        &self.components
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn impacted_module(&self, penetrated: bool, local_hit: Vec3) -> Option<ModuleSlot> {
        self.components
            .iter()
            .filter(|component| !component.requires_penetration || penetrated)
            .filter(|component| component.shape.contains(local_hit))
            .max_by_key(|component| component.priority)
            .map(|component| component.slot)
    }

    /// Intersect a complete through-flight with every authored component, nearest first. Hits are
    /// deduplicated by physical id, never by ModuleSlot: a shell may cross two racks or both a
    /// fuel tank and the engine even though the legacy HUD groups them.
    pub fn intersections(
        &self,
        penetrated: bool,
        start: Vec3,
        end: Vec3,
    ) -> Vec<ModuleIntersection> {
        self.intersections_with_turret_pose(penetrated, start, end, 0.0, Vec3::ZERO)
    }

    /// Intersect a through-flight while respecting the live turret pose. `start`, `end` and the
    /// turret pivot use centered hull-local coordinates; individual component shapes remain in
    /// their authored Hull, Turret or Mantlet frame.
    pub fn intersections_with_turret_pose(
        &self,
        penetrated: bool,
        start: Vec3,
        end: Vec3,
        turret_yaw_rad: f32,
        turret_pivot: Vec3,
    ) -> Vec<ModuleIntersection> {
        let length = (end - start).length();
        let mut hits: Vec<_> = self
            .components
            .iter()
            .filter(|component| !component.requires_penetration || penetrated)
            .filter_map(|component| {
                let (frame_start, frame_end) = component_frame_segment(
                    component.frame,
                    start,
                    end,
                    turret_yaw_rad,
                    turret_pivot,
                );
                component.shape.segment_interval(frame_start, frame_end).map(|(enter, exit)| {
                    ModuleIntersection {
                        component_id: component.id,
                        kind: component.kind,
                        slot: component.slot,
                        material: component.material,
                        distance_t: enter,
                        path_length_m: (exit - enter).max(0.0) * length,
                        priority: component.priority,
                        vulnerability: component.vulnerability,
                    }
                })
            })
            .collect();
        hits.sort_by(|a, b| {
            a.distance_t.total_cmp(&b.distance_t).then_with(|| b.priority.cmp(&a.priority))
        });
        hits.dedup_by_key(|hit| hit.component_id);
        hits
    }

    pub fn fits_within(&self, hitbox: HitboxProfile) -> bool {
        self.components.iter().all(|component| {
            let (min, max) = component.shape.bounds();
            [min, max].into_iter().all(|p| {
                p.x.abs() <= hitbox.half_width_m + 1.0e-3
                    && p.z.abs() <= hitbox.half_length_m + 1.0e-3
                    && p.y.abs() <= hitbox.half_height_m + 1.0e-3
            })
        })
    }
}

fn component_frame_segment(
    frame: ArmorFrame,
    start: Vec3,
    end: Vec3,
    turret_yaw_rad: f32,
    turret_pivot: Vec3,
) -> (Vec3, Vec3) {
    match frame {
        ArmorFrame::Hull => (start, end),
        ArmorFrame::Turret | ArmorFrame::Mantlet => {
            let inverse_pose = glam::Mat3::from_rotation_y(-turret_yaw_rad);
            (
                turret_pivot + inverse_pose * (start - turret_pivot),
                turret_pivot + inverse_pose * (end - turret_pivot),
            )
        }
    }
}
