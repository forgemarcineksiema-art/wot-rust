//! The RON blueprint source: one `blueprints/<slug>.blueprint.ron` file per vehicle, embedded
//! with `include_str!` (a missing file is a COMPILE error, and a new [`VehicleKind`] variant
//! fails the exhaustive match below), parsed once per kind behind a `OnceLock`, linted with
//! teaching errors before it is trusted. This is the file an author — human or AI — edits;
//! everything downstream (hitbox, mounts, armour volumes, mesh recipes, contact footprint)
//! derives from it.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::t54_hybrid::t54_hybrid;
use super::{ArmorShape, GunShape, HullShape, TrackShape, TurretShape, VehicleBlueprint};
use crate::VehicleKind;

/// The on-disk schema: the blueprint's plain-data shapes plus the kind tag the registry
/// cross-checks (a file pasted under the wrong name is a teaching error, not a silent adoption).
/// The T-54's `VisualDetail` extras stay in Rust and are attached by the loader — they are a
/// deep tree of loft data for one vehicle, not part of the shared schema.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BlueprintFile {
    pub kind: VehicleKind,
    pub hull: HullShape,
    pub track: TrackShape,
    pub turret: TurretShape,
    pub gun: GunShape,
    pub armor: ArmorShape,
}

impl BlueprintFile {
    /// Strip a full blueprint down to the on-disk schema (the exporter's direction).
    pub fn from_blueprint(blueprint: &VehicleBlueprint) -> Self {
        Self {
            kind: blueprint.kind,
            hull: blueprint.hull,
            track: blueprint.track,
            turret: blueprint.turret,
            gun: blueprint.gun,
            armor: blueprint.armor,
        }
    }

    /// Assemble the runtime blueprint: the parsed shapes plus any Rust-side extras
    /// (the T-54's hybrid visual tree).
    fn into_blueprint(self) -> VehicleBlueprint {
        VehicleBlueprint {
            kind: self.kind,
            hull: self.hull,
            track: self.track,
            turret: self.turret,
            gun: self.gun,
            armor: self.armor,
            visual_detail: (self.kind == VehicleKind::T54_1951).then(|| t54_hybrid(&self)),
        }
    }
}

/// The embedded RON source for `kind`, or `None` for kinds that have no blueprint (the
/// test-only prototype). EXHAUSTIVE on purpose: adding a `VehicleKind` variant fails here
/// until its blueprint file exists — the data-file equivalent of the old missing-match-arm
/// compile error.
pub(super) fn blueprint_ron(kind: VehicleKind) -> Option<&'static str> {
    match kind {
        VehicleKind::PrototypeMedium => None,
        VehicleKind::T54_1951 => Some(include_str!("../../blueprints/t54_1951.blueprint.ron")),
        VehicleKind::TigerI => Some(include_str!("../../blueprints/tiger_i_ausf_e.blueprint.ron")),
        VehicleKind::TigerII => {
            Some(include_str!("../../blueprints/tiger_ii_ausf_b.blueprint.ron"))
        }
        VehicleKind::Jagdtiger => Some(include_str!("../../blueprints/jagdtiger.blueprint.ron")),
        VehicleKind::PantherII => Some(include_str!("../../blueprints/panther_ii.blueprint.ron")),
        VehicleKind::IS3 => Some(include_str!("../../blueprints/is3.blueprint.ron")),
        VehicleKind::Centurion => {
            Some(include_str!("../../blueprints/centurion_mk3.blueprint.ron"))
        }
        VehicleKind::T34_85 => Some(include_str!("../../blueprints/t34_85.blueprint.ron")),
    }
}

/// Parse and validate a blueprint RON string for `kind`. Public for the loader, the studio
/// live-override path, and the tests; panics are reserved for the embedded files (a broken
/// embedded blueprint is a build defect, caught by `all_blueprints_pass_lint`).
pub fn parse_blueprint(kind: VehicleKind, ron_text: &str) -> Result<VehicleBlueprint, String> {
    // Windows editors love to prepend a UTF-8 BOM; a live-override author should never
    // lose a loop iteration to an invisible byte.
    let ron_text = ron_text.trim_start_matches('\u{feff}');
    let file: BlueprintFile = ron::from_str(ron_text)
        .map_err(|error| format!("{}: blueprint RON does not parse: {error}", kind.slug()))?;
    if file.kind != kind {
        return Err(format!(
            "{}: blueprint file declares kind {:?} but is registered for {:?} — a pasted \
             template must have its `kind` retagged before it is trusted",
            kind.slug(),
            file.kind,
            kind
        ));
    }
    let blueprint = file.into_blueprint();
    let issues = super::lint::validate_blueprint(&blueprint);
    let errors: Vec<_> =
        issues.iter().filter(|i| i.severity == super::lint::Severity::Error).collect();
    if !errors.is_empty() {
        let listing =
            errors.iter().map(|i| format!("  - {}: {}", i.field, i.message)).collect::<Vec<_>>();
        return Err(format!(
            "{}: blueprint fails validation:\n{}",
            kind.slug(),
            listing.join("\n")
        ));
    }
    Ok(blueprint)
}

/// The on-disk schema for a vehicle's [`VisualDetail`](super::VisualDetail) tree (W4 F2c): the
/// full loft/gun/fittings data under the same kind tag the blueprint files carry, so a file
/// pasted under the wrong name is the same teaching error, not a silent adoption.
///
/// Today the tree is still GENERATED (`t54_hybrid` derives it from the blueprint, so raising
/// the roof still raises the cupola with it); this schema is what the fleet slot (F3) reads
/// and what the exporter writes — the round-trip lock in `blueprint_source.rs` proves every
/// field of the tree survives RON with digit fidelity before any vehicle is asked to live there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisualDetailFile {
    pub kind: VehicleKind,
    pub detail: super::VisualDetail,
}

impl VisualDetailFile {
    /// Wrap a vehicle's visual tree for export (the exporter's direction).
    pub fn from_parts(kind: VehicleKind, detail: &super::VisualDetail) -> Self {
        Self { kind, detail: *detail }
    }
}

/// Parse a visual-detail RON string for `kind`. Public for the exporter's round-trip lock and
/// the future fleet-slot loader; the same BOM tolerance and kind cross-check as
/// [`parse_blueprint`], because the failure modes of a hand-edited file are the same.
pub fn parse_visual_detail(
    kind: VehicleKind,
    ron_text: &str,
) -> Result<super::VisualDetail, String> {
    let ron_text = ron_text.trim_start_matches('\u{feff}');
    let file: VisualDetailFile = ron::from_str(ron_text)
        .map_err(|error| format!("{}: visual-detail RON does not parse: {error}", kind.slug()))?;
    if file.kind != kind {
        return Err(format!(
            "{}: visual-detail file declares kind {:?} but is registered for {:?} — a pasted \
             template must have its `kind` retagged before it is trusted",
            kind.slug(),
            file.kind,
            kind
        ));
    }
    Ok(file.detail)
}

/// The parsed, validated blueprint for `kind` — parsed once per process.
pub(super) fn load_blueprint(kind: VehicleKind) -> Option<VehicleBlueprint> {
    static CACHE: OnceLock<[Option<VehicleBlueprint>; VehicleKind::ALL.len()]> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        VehicleKind::ALL.map(|kind| {
            blueprint_ron(kind).map(|text| match parse_blueprint(kind, text) {
                Ok(blueprint) => blueprint,
                Err(error) => panic!("embedded blueprint invalid — {error}"),
            })
        })
    });
    let index = VehicleKind::ALL.iter().position(|&k| k == kind).expect("kind is in ALL");
    cache[index]
}
