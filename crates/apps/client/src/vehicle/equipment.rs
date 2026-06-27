//! Optional equipment attachment points for baked vehicles (part of the runtime variation layer).
//!
//! Positions are derived from a vehicle's baked hull and turret bounds, so stowage rides on the real
//! geometry rather than on hand-tuned magic values. Split from [`crate::vehicle::variation`] to keep
//! each module small and reviewable.

use game_core::VehicleKind;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::SubmeshKind;

/// Where an optional stowage/equipment item rides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentAnchor {
    Hull,
    Turret,
}

/// A named attachment slot for optional equipment (spare track, toolbox, antenna, …). Positions are
/// derived from the baked geometry bounds so they sit on the real hull/turret, not on magic values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquipmentPoint {
    pub name: &'static str,
    pub anchor: EquipmentAnchor,
    pub local_position: [f32; 3],
}

/// The standard equipment attachment points for `kind`, derived from its baked hull and turret
/// bounds: spare track on the glacis, a toolbox on the right fender, a stowage log on the rear hull,
/// and a radio antenna at the turret rear.
pub fn equipment_points(kind: VehicleKind) -> Vec<EquipmentPoint> {
    let Ok(baked) = authoritative_baked_vehicle(kind) else {
        return Vec::new();
    };
    let hull = baked.submesh(SubmeshKind::Hull).and_then(|s| s.mesh.bounds());
    let turret = baked.submesh(SubmeshKind::Turret).and_then(|s| s.mesh.bounds());
    let mut points = Vec::new();
    if let Some(h) = hull {
        points.push(EquipmentPoint {
            name: "spare_track",
            anchor: EquipmentAnchor::Hull,
            local_position: [0.0, mid(h.min.y, h.max.y), h.max.z * 0.92],
        });
        points.push(EquipmentPoint {
            name: "toolbox",
            anchor: EquipmentAnchor::Hull,
            local_position: [h.max.x * 0.96, mid(h.min.y, h.max.y), h.min.z * 0.2],
        });
        points.push(EquipmentPoint {
            name: "stowage_log",
            anchor: EquipmentAnchor::Hull,
            local_position: [0.0, mid(h.min.y, h.max.y), h.min.z * 0.96],
        });
    }
    if let Some(t) = turret {
        points.push(EquipmentPoint {
            name: "antenna",
            anchor: EquipmentAnchor::Turret,
            local_position: [t.min.x * 0.7, t.max.y, t.min.z * 0.8],
        });
    }
    points
}

fn mid(a: f32, b: f32) -> f32 {
    0.5 * (a + b)
}
