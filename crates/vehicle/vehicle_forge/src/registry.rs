//! The single registration point for forgeable vehicles.
//!
//! Adding a forgeable vehicle is one arm here: its reference pack, how its semantic part graph is
//! derived, and which review-camera set to bake. Nothing else in the crate keys forge wiring on
//! `VehicleKind`, so a new vehicle can no longer accidentally inherit another vehicle's bespoke
//! part geometry (the latent trap when every blueprint-backed vehicle ran the T-54 part table).

use game_core::{VehicleBlueprint, VehicleKind};

use crate::{ForgePart, ReferencePack, ReviewCameraSet};

/// How a vehicle's semantic part graph is derived.
#[derive(Clone, Copy)]
pub(crate) enum PartStrategy {
    /// Bespoke, blueprint-backed parts at full fidelity (carries the per-vehicle constructor).
    Blueprint(fn(&VehicleBlueprint) -> Vec<ForgePart>),
    /// Coarse parts derived from baked submesh bounds plus the reference running-gear count.
    BakedGeometry,
}

/// Everything the forge needs to bake one vehicle, resolved in one place.
pub(crate) struct VehicleForgeSpec {
    pub reference_pack: fn() -> ReferencePack,
    pub parts: PartStrategy,
    pub review_cameras: fn() -> ReviewCameraSet,
}

/// The forge spec for `kind`, or `None` for vehicles that are not benchmarked families
/// (the legacy T-55A and the placeholder prototype).
pub(crate) fn forge_spec(kind: VehicleKind) -> Option<VehicleForgeSpec> {
    let spec = match kind {
        VehicleKind::T54_1951 => VehicleForgeSpec {
            reference_pack: crate::t54_reference_pack,
            parts: PartStrategy::Blueprint(crate::part_data::t54_family_parts),
            review_cameras: ReviewCameraSet::t54_benchmark_review,
        },
        // W1 PR-T1.3: the Tiger I graduates from the coarse baked-bounds graph to a bespoke
        // blueprint part table (the IS-3/Centurion tier).
        VehicleKind::TigerI => VehicleForgeSpec {
            reference_pack: crate::tiger_i_reference_pack,
            parts: PartStrategy::Blueprint(crate::part_data::tiger_i_parts),
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        VehicleKind::TigerII => german(crate::tiger_ii_reference_pack),
        VehicleKind::Jagdtiger => german(crate::jagdtiger_reference_pack),
        VehicleKind::PantherII => german(crate::panther_ii_reference_pack),
        // Blueprint-backed bespoke part tables: every extent restates a blueprint field, so
        // the parts carry the pike/skirt/bogie identity instead of coarse baked bounds.
        VehicleKind::IS3 => VehicleForgeSpec {
            reference_pack: crate::is3_reference_pack,
            parts: PartStrategy::Blueprint(crate::part_data::is3_parts),
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        VehicleKind::Centurion => VehicleForgeSpec {
            reference_pack: crate::centurion_reference_pack,
            parts: PartStrategy::Blueprint(crate::part_data::centurion_parts),
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        // Studio-born: shares the standard cameras and a geometry-derived part graph until
        // a bespoke table earns its keep (the IS-3/Centurion pattern).
        VehicleKind::T34_85 => VehicleForgeSpec {
            reference_pack: crate::t34_85_reference_pack,
            parts: PartStrategy::BakedGeometry,
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        // Studio-born, same tier as the T-34-85: standard cameras and a geometry-derived part
        // graph until a bespoke table earns its keep.
        VehicleKind::KV1_1942 => VehicleForgeSpec {
            reference_pack: crate::kv1_reference_pack,
            parts: PartStrategy::BakedGeometry,
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        VehicleKind::PrototypeMedium => return None,
    };
    Some(spec)
}

/// The German line shares one shape: a geometry-derived part graph and the standard review set.
fn german(reference_pack: fn() -> ReferencePack) -> VehicleForgeSpec {
    VehicleForgeSpec {
        reference_pack,
        parts: PartStrategy::BakedGeometry,
        review_cameras: ReviewCameraSet::standard_vehicle_review,
    }
}
