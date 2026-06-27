//! The Forge's view of the *executable* part manifest.
//!
//! Production parts are no longer re-derived from flat blueprint fields here: the manifest comes
//! straight from the executable [`VehicleDescription`](vehicle_build::VehicleDescription), so its
//! bounds are the geometry the kernels actually emit. This is what lets the Forge report what built
//! each part (the selected generator) and prove the executable geometry has not drifted from the
//! blueprint-derived semantic graph.

use game_core::VehicleKind;
use vehicle_build::{PartManifestEntry, PartManifestError, t54_description, validate_manifest};

/// The executable part manifest for a production-migrated vehicle, or `None` for vehicles still on
/// the legacy recipe path (only the T-54 benchmark is migrated today).
pub fn production_part_manifest(kind: VehicleKind) -> Option<Vec<PartManifestEntry>> {
    match kind {
        VehicleKind::T54_1951 => Some(t54_description().part_manifest()),
        _ => None,
    }
}

/// Validate a migrated vehicle's manifest against the production part requirements. `None` if the
/// vehicle is not yet migrated.
pub fn validate_production_manifest(kind: VehicleKind) -> Option<Result<(), PartManifestError>> {
    production_part_manifest(kind).map(|m| validate_manifest(&m))
}

/// A human-readable manifest report: every executable part, the generator that built it, its
/// gameplay role and its source note. Answers "what built each part and why" directly.
pub fn part_manifest_report(kind: VehicleKind) -> Option<String> {
    let manifest = production_part_manifest(kind)?;
    let mut out = format!(
        "# {} executable part manifest\n\nProduction parts: {}\n\n",
        kind.display_name(),
        manifest.len(),
    );
    out.push_str("| Part | Group | Generator | Role | Source |\n| --- | --- | --- | --- | --- |\n");
    for e in &manifest {
        out.push_str(&format!(
            "| {} | {:?} | {} | {:?} | {} |\n",
            e.key.name,
            e.group,
            e.generator.kernel_name(),
            e.role,
            e.source,
        ));
    }
    Some(out)
}
