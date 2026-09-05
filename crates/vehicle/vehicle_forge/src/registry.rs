//! The single registration point for forgeable vehicles.
//!
//! Adding a forgeable vehicle is one arm here: how its semantic part graph is derived and which
//! review-camera set to bake (its reference pack is DATA: `reference/<slug>.reference.ron`). Nothing else in the crate keys forge wiring on
//! `VehicleKind`, so a new vehicle can no longer accidentally inherit another vehicle's bespoke
//! part geometry (the latent trap when every blueprint-backed vehicle ran the T-54 part table).

use game_core::{VehicleBlueprint, VehicleKind};

use crate::{ForgePart, ReviewCameraSet};

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
    pub parts: PartStrategy,
    pub review_cameras: fn() -> ReviewCameraSet,
}

/// The forge spec for `kind`. Every playable vehicle is a benchmarked family.
pub(crate) fn forge_spec(kind: VehicleKind) -> Option<VehicleForgeSpec> {
    let spec = match kind {
        VehicleKind::T54_1951 => VehicleForgeSpec {
            parts: PartStrategy::Blueprint(crate::part_data::t54_family_parts),
            review_cameras: ReviewCameraSet::t54_benchmark_review,
        },
        // W1 PR-T1.3: the Tiger I graduates from the coarse baked-bounds graph to a bespoke
        // blueprint part table (the IS-3/Centurion tier).
        VehicleKind::TigerI => VehicleForgeSpec {
            parts: PartStrategy::Blueprint(crate::part_data::tiger_i_parts),
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        VehicleKind::TigerII => german(),
        VehicleKind::Jagdtiger => german(),
        VehicleKind::PantherII => german(),
        // Blueprint-backed bespoke part tables: every extent restates a blueprint field, so
        // the parts carry the pike/skirt/bogie identity instead of coarse baked bounds.
        VehicleKind::IS3 => VehicleForgeSpec {
            parts: PartStrategy::Blueprint(crate::part_data::is3_parts),
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        VehicleKind::Centurion => VehicleForgeSpec {
            parts: PartStrategy::Blueprint(crate::part_data::centurion_parts),
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
        // Studio-born: shares the standard cameras and a geometry-derived part graph until
        // a bespoke table earns its keep (the IS-3/Centurion pattern).
        VehicleKind::T34_85 => VehicleForgeSpec {
            parts: PartStrategy::BakedGeometry,
            review_cameras: ReviewCameraSet::standard_vehicle_review,
        },
    };
    Some(spec)
}

/// The German line shares one shape: a geometry-derived part graph and the standard review set.
fn german() -> VehicleForgeSpec {
    VehicleForgeSpec {
        parts: PartStrategy::BakedGeometry,
        review_cameras: ReviewCameraSet::standard_vehicle_review,
    }
}
