//! The per-vehicle surface-bake record carried in the Forge manifest (R8 / Task 25).
//!
//! The bake-time ambient-contact cavities live in the executable vehicle description; the artifact
//! records *which* named signals it baked and a stable config hash, so a baked artifact documents
//! its surface bake and invalidates intentionally when the cavity configuration changes. (The mesh
//! payload's `source_hash` already moves when the resulting `surface_shade` does; this is the human-
//! and tooling-readable summary of that bake.)

use game_core::VehicleKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceBakeManifest {
    signals: Vec<String>,
    config_hash: u64,
}

impl SurfaceBakeManifest {
    /// The named contact signals baked into this artifact, in author order.
    pub fn signals(&self) -> &[String] {
        &self.signals
    }

    /// The stable hash of the cavity configuration the artifact baked.
    pub fn config_hash(&self) -> u64 {
        self.config_hash
    }
}

/// The surface-bake summary for `vehicle`, or `None` for vehicles that bake flat (no cavities).
pub fn surface_bake_manifest(vehicle: VehicleKind) -> Option<SurfaceBakeManifest> {
    let bake = match vehicle {
        VehicleKind::T54_1951 => vehicle_build::t54_description().surface_bake,
        _ => return None,
    };
    if bake.is_empty() {
        return None;
    }
    Some(SurfaceBakeManifest {
        signals: bake.signals().iter().map(|s| (*s).to_string()).collect(),
        config_hash: bake.config_hash(),
    })
}
