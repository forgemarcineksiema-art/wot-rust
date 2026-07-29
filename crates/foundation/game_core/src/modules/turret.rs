use serde::{Deserialize, Serialize};

/// Whether the turret traverses (a normal rotating turret) or is welded to the hull (a
/// casemate tank destroyer like the Jagdtiger).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TurretTraverse {
    Rotating { rate_rad_s: f32 },
    Fixed,
}

impl TurretTraverse {
    pub fn rate_rad_s(self) -> f32 {
        match self {
            TurretTraverse::Rotating { rate_rad_s } => rate_rad_s,
            TurretTraverse::Fixed => 0.0,
        }
    }

    pub fn is_fixed(self) -> bool {
        matches!(self, TurretTraverse::Fixed)
    }
}

/// Turret (or casemate). Provides front-armor for the assembled profile, traverse, view
/// range, and the largest gun caliber it can mount.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurretModule {
    pub name: String,
    pub mass_kg: f32,
    pub hit_points: u32,
    pub front_mm: f32,
    /// Side wall at its THICKEST — where the cheeks end and the flank begins. On a cast turret
    /// the wall then thins toward the rear (see `side_rear_mm`); on a welded box it is the whole
    /// side plate.
    pub side_mm: f32,
    pub rear_mm: f32,
    /// Turret roof (mm). `None` derives it from the front plate, the fleet-wide formula that
    /// gave the T-54 24 mm where its documents say 30.
    #[serde(default)]
    pub roof_mm: Option<f32>,
    pub traverse: TurretTraverse,
    pub view_range_m: f32,
    /// Largest gun caliber (mm) this turret accepts — the gun-mount compatibility gate.
    pub max_gun_caliber_mm: f32,
}
