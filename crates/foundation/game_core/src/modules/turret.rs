use serde::{Deserialize, Serialize};

/// Whether the turret traverses (a normal rotating turret), is welded to the hull with no
/// gun traverse at all (`Fixed`), or is a casemate whose GUN lays inside an arc about the
/// hull line (`Limited`, S17: the Jagdtiger's ball mount gives ±10°). Append-only: the
/// variants ride the module data.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TurretTraverse {
    Rotating {
        rate_rad_s: f32,
    },
    Fixed,
    /// A casemate: the superstructure never turns, the gun lays `half_arc_rad` to either
    /// side of the hull line at `rate_rad_s`; beyond the arc the hull pivots.
    Limited {
        half_arc_rad: f32,
        rate_rad_s: f32,
    },
}

impl TurretTraverse {
    pub fn rate_rad_s(self) -> f32 {
        match self {
            TurretTraverse::Rotating { rate_rad_s } => rate_rad_s,
            TurretTraverse::Fixed => 0.0,
            TurretTraverse::Limited { rate_rad_s, .. } => rate_rad_s,
        }
    }

    pub fn is_fixed(self) -> bool {
        matches!(self, TurretTraverse::Fixed)
    }

    /// The gun's yaw arc about the hull line: `None` for a turret (the full circle), `Some`
    /// for a casemate — zero when it cannot lay at all.
    pub fn half_arc_rad(self) -> Option<f32> {
        match self {
            TurretTraverse::Rotating { .. } => None,
            TurretTraverse::Fixed => Some(0.0),
            TurretTraverse::Limited { half_arc_rad, .. } => Some(half_arc_rad.max(0.0)),
        }
    }

    /// A casemate: the superstructure is the hull's, whatever the gun can do inside it.
    pub fn is_casemate(self) -> bool {
        self.half_arc_rad().is_some()
    }
}

/// Turret (or casemate). Provides front-armor for the assembled profile, traverse, and the
/// largest gun caliber it can mount. It carries NO view range: spotting is per-vehicle by design
/// (`TankSpec::view_range_m`, v29) — a per-turret number here was dead data that contradicted
/// a settled rule, and dead data is where the next dishonest UI number comes from.
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
    /// Vertical gun stabilizer, 0 (none) to 1 (holds the gun's world elevation against every
    /// hull pitch change, within the arc). Inny Poziom A12: the Centurion Mk 3 carried one;
    /// the wartime hulls and the T-54 obr. 1951 did not (the T-54A's STP-1 came in 1955).
    #[serde(default)]
    pub vertical_stabilizer: f32,
    /// Largest gun caliber (mm) this turret accepts — the gun-mount compatibility gate.
    pub max_gun_caliber_mm: f32,
}
