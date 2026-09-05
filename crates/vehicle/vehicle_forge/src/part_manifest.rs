//! The Forge's view of the *executable* part manifest.
//!
//! Production parts are no longer re-derived from flat blueprint fields here: the manifest comes
//! straight from the executable [`VehicleDescription`](vehicle_build::VehicleDescription), so its
//! bounds are the geometry the kernels actually emit. This is what lets the Forge report what built
//! each part (the selected generator) and prove the executable geometry has not drifted from the
//! blueprint-derived semantic graph.

use game_core::VehicleKind;
use vehicle_build::{PartManifestEntry, PartManifestError, validate_manifest};

use crate::mesh_source::authoritative_description;

/// The executable part manifest for any vehicle the seam describes (Forge 2.0 K1): the
/// benchmark's seventy-odd library parts, or a sketch's three `Recipe` parts — which is the
/// honest manifest of a vehicle nobody has built yet. `None` only if nothing describes it.
pub fn production_part_manifest(kind: VehicleKind) -> Option<Vec<PartManifestEntry>> {
    Some(authoritative_description(kind).ok()?.part_manifest())
}

/// Validate a vehicle's manifest against the production part requirements. `None` if nothing
/// describes the vehicle.
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
