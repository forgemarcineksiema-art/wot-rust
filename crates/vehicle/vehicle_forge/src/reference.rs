use game_core::VehicleKind;
use serde::{Deserialize, Serialize};
use vehicle_geometry::BakedVehicle;

use crate::RatioReport;
use crate::outline::{
    OutlineMeasurement, OutlineSet, OutlineSpec, composed_triangles_for, measure,
};

/// The reference pack of `kind` as AUTHORED: one RON file per vehicle beside the crate
/// (`reference/<slug>.reference.ron`), embedded with `include_str!`. Since 2026-09-05 (Forge 2.0
/// acceleration, step 2b) this is the only place a vehicle's anchors, ratios and sources live —
/// a data PR edits the file and runs the vehicle gate; nothing is restated in Rust.
fn embedded_reference_ron(kind: VehicleKind) -> Option<&'static str> {
    Some(match kind {
        VehicleKind::T54_1951 => include_str!("../reference/t54_1951.reference.ron"),
        VehicleKind::TigerI => include_str!("../reference/tiger_i_ausf_e.reference.ron"),
        VehicleKind::TigerII => include_str!("../reference/tiger_ii_ausf_b.reference.ron"),
        VehicleKind::Jagdtiger => include_str!("../reference/jagdtiger.reference.ron"),
        VehicleKind::PantherII => include_str!("../reference/panther_ii.reference.ron"),
        VehicleKind::IS3 => include_str!("../reference/is3.reference.ron"),
        VehicleKind::Centurion => include_str!("../reference/centurion_mk3.reference.ron"),
        VehicleKind::T34_85 => include_str!("../reference/t34_85.reference.ron"),
    })
}

/// The traced outlines of `kind` (`outlines/<slug>.outline.ron`), for the vehicles whose
/// drawing has been traced (K0); the others carry no outline gate yet.
fn embedded_outline_ron(kind: VehicleKind) -> Option<&'static str> {
    match kind {
        VehicleKind::T54_1951 => Some(include_str!("../outlines/t54_1951.outline.ron")),
        VehicleKind::TigerI => Some(include_str!("../outlines/tiger_i_ausf_e.outline.ron")),
        _ => None,
    }
}

/// The authored outline set of `kind`, or `None` when its drawing is not traced yet.
pub fn outline_set(kind: VehicleKind) -> Option<OutlineSet> {
    let text = embedded_outline_ron(kind)?;
    Some(OutlineSet::parse(text).unwrap_or_else(|error| {
        panic!("outlines/{}.outline.ron does not parse: {error}", kind.slug())
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RatioKind {
    HullLengthToWidth,
    HullHeightToLength,
    TurretWidthToHullWidth,
    TurretHeightToHullHeight,
    GunProtrusionToHullLength,
    /// Turret (or casemate) plan proportion: Z-extent over X-extent of the turret submesh.
    TurretLengthToWidth,
    /// Where the turret ring sits along the hull: `(ring_z - hull.min.z) / hull length`,
    /// 0 = rear, 1 = bow. Catches a turret drifting fore/aft while every extent stays right.
    TurretRingPositionOnHull,
    /// Road-wheel diameter over hull length — the running gear's visual weight on the side view.
    RoadWheelDiameterToHullLength,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceSource {
    label: String,
    url: String,
    note: String,
}

impl ReferenceSource {
    pub fn new(label: impl Into<String>, url: impl Into<String>, note: impl Into<String>) -> Self {
        Self { label: label.into(), url: url.into(), note: note.into() }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn note(&self) -> &str {
        &self.note
    }
}

/// A measurable absolute dimension of the baked vehicle, in metres (counts are unit-less and
/// say so in their doc). Ratios alone pass at the wrong scale — these are the anchors that pin
/// the model to the real tank's tape measure. Append-only: reports serialize this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimensionKind {
    /// Z-extent of the visual hull including the running gear (hull length over tracks).
    HullLength,
    /// X-extent of the visual hull including the running gear (width over tracks).
    HullWidth,
    /// Highest exterior point of hull or turret above the ground plane (silhouette apex:
    /// cupolas and fittings count — the Tiger I anchors 3.00 m at the drum cupola).
    HeightToTurretRoof,
    /// Muzzle to hull rear (overall length, gun forward).
    OverallLengthWithGun,
    /// Road-wheel diameter, measured off the road-wheel unit MESH (not the generator field).
    RoadWheelDiameter,
    /// Turret-ring race diameter in the clear. The race is interior — invisible to an exterior
    /// bake — so this anchor pins the BLUEPRINT's ring against the dossier (basis: Blueprint).
    TurretRingDiameter,
    /// Height of the gun's trunnion axis above the ground plane (basis: Mounts — the same
    /// frame the sim fires from, so the dossier pins gameplay, not just the picture).
    FireLineHeight,
    /// Commander-cupola external diameter, measured as a horizontal slice of the turret mesh
    /// inside the blueprint's cupola disc, above the bare roof.
    CupolaDiameter,
    /// Width of one track belt, measured off the track-link unit mesh.
    TrackWidth,
    /// Track gauge (kolej): twice the mean |x| of the link instances — centre distance between
    /// the two belts as actually placed.
    TrackGauge,
    /// Belly floor above the ground plane, measured over the central strip of the hull mesh
    /// (fenders, sponsons and gear brackets excluded by the strip).
    GroundClearance,
    /// Unit-less: link instances per side, counted from the rest-pose placements.
    TrackLinkCountPerSide,
    /// Unit-less: road wheels per side, counted from the rest-pose placements.
    RoadWheelCount,
    /// Height to the structural turret roof EXCLUDING the cupola (the number Soviet documents
    /// quote as "по крышу башни"); flush hatch lids count as roof plane by design.
    HeightToTurretRoofBare,
    /// Height of the fender shelf's SHEET TOP above the ground plane, measured in a thin strip
    /// at the fender's outer edge (mid-hull, clear of the mudguard arches and the stowage line).
    /// This is the band the 2026-08-12 review found mis-read: the sheet was authored onto the
    /// track crest because a three-view scan measured the crest line and called it the shelf.
    FenderShelfHeight,
}

impl DimensionKind {
    pub fn label(self) -> &'static str {
        match self {
            DimensionKind::HullLength => "hull length (over tracks)",
            DimensionKind::HullWidth => "width (over tracks)",
            DimensionKind::HeightToTurretRoof => "height to silhouette apex",
            DimensionKind::OverallLengthWithGun => "overall length (gun forward)",
            DimensionKind::RoadWheelDiameter => "road wheel diameter",
            DimensionKind::TurretRingDiameter => "turret ring diameter (in the clear)",
            DimensionKind::FireLineHeight => "fire line height (trunnion axis)",
            DimensionKind::CupolaDiameter => "commander cupola diameter",
            DimensionKind::TrackWidth => "track width",
            DimensionKind::TrackGauge => "track gauge (belt centres)",
            DimensionKind::GroundClearance => "ground clearance",
            DimensionKind::TrackLinkCountPerSide => "track links per side (count)",
            DimensionKind::RoadWheelCount => "road wheels per side (count)",
            DimensionKind::HeightToTurretRoofBare => "height to turret roof (bare, no cupola)",
            DimensionKind::FenderShelfHeight => "fender shelf height (sheet top)",
        }
    }

    /// Counts are unit-less integers riding the same anchor machinery; formatting cares.
    pub fn is_count(self) -> bool {
        matches!(self, DimensionKind::TrackLinkCountPerSide | DimensionKind::RoadWheelCount)
    }
}

/// Where a measurement came from — honesty about the instrument. `Mesh` is the tape measure on
/// the bake; `Mounts` is the sim's own frame; `Instances` counts what actually renders;
/// `Blueprint` is a declared value used only where the feature is invisible to an exterior bake
/// (e.g. the turret-ring race) and says so in the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MeasurementBasis {
    #[default]
    Mesh,
    Mounts,
    Instances,
    Blueprint,
}

impl MeasurementBasis {
    pub fn label(self) -> &'static str {
        match self {
            MeasurementBasis::Mesh => "mesh",
            MeasurementBasis::Mounts => "mounts",
            MeasurementBasis::Instances => "instances",
            MeasurementBasis::Blueprint => "blueprint",
        }
    }
}

/// Whether an anchor is enforced or a declared, visible debt. `Locked` fails the dimension gate
/// on any miss; `Target` is the dossier's documented value the model has NOT reached yet — it
/// reports as debt (never silently passes) and flips to `Locked` in the PR that closes it.
/// This is the FLOOR/TARGET mechanism of the art-direction program, applied to centimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AnchorStatus {
    #[default]
    Locked,
    Target,
}

/// One real-world dimension the baked model must honour, with the source that documents it.
/// The tolerance is absolute (metres), never a percentage — a tape measure, not a vibe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionTarget {
    kind: DimensionKind,
    target_m: f32,
    tolerance_m: f32,
    source: ReferenceSource,
    /// `Locked` gates; `Target` reports as debt until its fixing PR flips it. Serde-defaulted
    /// so previously serialized reports decode as the stricter `Locked`.
    #[serde(default)]
    status: AnchorStatus,
}

impl DimensionTarget {
    pub fn new(
        kind: DimensionKind,
        target_m: f32,
        tolerance_m: f32,
        source: ReferenceSource,
    ) -> Self {
        assert!(target_m.is_finite() && target_m > 0.0);
        assert!(tolerance_m.is_finite() && tolerance_m >= 0.0);
        Self { kind, target_m, tolerance_m, source, status: AnchorStatus::Locked }
    }

    /// A documented value the model has not reached yet: reported as debt, not asserted.
    /// The dossier number lands first, the geometry PR flips it to `Locked` — data first,
    /// model second.
    pub fn target_pending(
        kind: DimensionKind,
        target_m: f32,
        tolerance_m: f32,
        source: ReferenceSource,
    ) -> Self {
        let mut anchor = Self::new(kind, target_m, tolerance_m, source);
        anchor.status = AnchorStatus::Target;
        anchor
    }

    pub fn kind(&self) -> DimensionKind {
        self.kind
    }

    pub fn target_m(&self) -> f32 {
        self.target_m
    }

    pub fn tolerance_m(&self) -> f32 {
        self.tolerance_m
    }

    pub fn source(&self) -> &ReferenceSource {
        &self.source
    }

    pub fn status(&self) -> AnchorStatus {
        self.status
    }

    pub(crate) fn passes(&self, measured_m: f32) -> bool {
        measured_m.is_finite() && (measured_m - self.target_m).abs() <= self.tolerance_m
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioTarget {
    kind: RatioKind,
    target: f32,
    tolerance: f32,
    note: String,
}

impl RatioTarget {
    pub fn new(kind: RatioKind, target: f32, tolerance: f32, note: impl Into<String>) -> Self {
        assert!(target.is_finite() && target > 0.0);
        assert!(tolerance.is_finite() && tolerance >= 0.0);
        Self { kind, target, tolerance, note: note.into() }
    }

    pub fn kind(&self) -> RatioKind {
        self.kind
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn tolerance(&self) -> f32 {
        self.tolerance
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    pub(crate) fn passes(&self, measured: f32) -> bool {
        measured.is_finite() && (measured - self.target).abs() <= self.tolerance
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferencePack {
    family_slug: String,
    display_name: String,
    vehicles: Vec<VehicleKind>,
    summary: String,
    road_wheel_count_per_side: usize,
    sources: Vec<ReferenceSource>,
    ratios: Vec<RatioTarget>,
    /// Absolute anchors (metres). Optional per pack: the gate only fires for vehicles whose
    /// dossier has been converted into targets, so the bar rises vehicle by vehicle.
    #[serde(default)]
    dimensions: Vec<DimensionTarget>,
    /// Reference outlines per view (the K0 overlay gate). Optional per pack, like the anchors:
    /// the gate only fires for vehicles whose drawing has been traced into loops.
    #[serde(default)]
    outlines: Vec<OutlineSpec>,
}

impl ReferencePack {
    pub fn new(
        family_slug: impl Into<String>,
        display_name: impl Into<String>,
        vehicles: Vec<VehicleKind>,
        summary: impl Into<String>,
        road_wheel_count_per_side: usize,
        sources: Vec<ReferenceSource>,
        ratios: Vec<RatioTarget>,
    ) -> Self {
        assert!(!vehicles.is_empty());
        assert!(road_wheel_count_per_side > 0);
        Self {
            family_slug: family_slug.into(),
            display_name: display_name.into(),
            vehicles,
            summary: summary.into(),
            road_wheel_count_per_side,
            sources,
            ratios,
            dimensions: Vec::new(),
            outlines: Vec::new(),
        }
    }

    /// Attach the reference outlines (closed loops per view, from the dossier's drawing).
    pub fn with_outlines(mut self, outlines: Vec<OutlineSpec>) -> Self {
        self.outlines = outlines;
        self
    }

    /// The pack without its outlines — what `reference/<slug>.reference.ron` holds; the loops
    /// stay in `outlines/<slug>.outline.ron` and are attached at load.
    pub fn without_outlines(mut self) -> Self {
        self.outlines.clear();
        self
    }

    /// Attach absolute dimension anchors (the dossier's tape-measure numbers).
    pub fn with_dimensions(mut self, dimensions: Vec<DimensionTarget>) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// The reference pack for `kind`: registered in the central forge registry (with the
    /// part-graph strategy and the review cameras), authored in `reference/<slug>.reference.ron`,
    /// with the traced outlines of `outlines/<slug>.outline.ron` attached when the vehicle has
    /// them.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        crate::registry::forge_spec(kind)?;
        Self::embedded(kind)
    }

    /// The pack as authored in its RON file, outlines attached.
    pub fn embedded(kind: VehicleKind) -> Option<Self> {
        let text = embedded_reference_ron(kind)?;
        let pack: Self = ron::from_str(text).unwrap_or_else(|error| {
            panic!("reference/{}.reference.ron does not parse: {error}", kind.slug())
        });
        assert!(
            pack.vehicles.contains(&kind),
            "reference/{}.reference.ron does not list {kind:?} among its vehicles",
            kind.slug()
        );
        Some(match outline_set(kind) {
            Some(set) => pack.with_outlines(set.into_views()),
            None => pack,
        })
    }

    pub fn family_slug(&self) -> &str {
        &self.family_slug
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn vehicles(&self) -> &[VehicleKind] {
        &self.vehicles
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn road_wheel_count_per_side(&self) -> usize {
        self.road_wheel_count_per_side
    }

    pub fn sources(&self) -> &[ReferenceSource] {
        &self.sources
    }

    pub fn ratio(&self, kind: RatioKind) -> Option<&RatioTarget> {
        self.ratios.iter().find(|target| target.kind == kind)
    }

    pub fn ratios(&self) -> &[RatioTarget] {
        &self.ratios
    }

    pub fn dimensions(&self) -> &[DimensionTarget] {
        &self.dimensions
    }

    pub fn outlines(&self) -> &[OutlineSpec] {
        &self.outlines
    }

    /// Every outline of this pack against the composed silhouette of `vehicle` (the bake plus
    /// its rest-pose running gear). Empty when the pack has no outlines yet.
    pub fn measure_outlines(&self, vehicle: &BakedVehicle) -> Vec<OutlineMeasurement> {
        if self.outlines.is_empty() {
            return Vec::new();
        }
        let tris = composed_triangles_for(vehicle);
        self.outlines.iter().map(|spec| measure(&tris, spec)).collect()
    }

    pub fn measure_baked_vehicle(&self, vehicle: &BakedVehicle) -> Option<RatioReport> {
        crate::reference_measure::measure_baked_vehicle(self, vehicle, None)
    }

    /// Ratios against a LIVE blueprint (a Studio `--blueprint-file` override): the running gear
    /// is rebuilt from the edited track instead of the embedded one.
    pub fn measure_baked_vehicle_live(
        &self,
        vehicle: &BakedVehicle,
        blueprint: &game_core::VehicleBlueprint,
    ) -> Option<RatioReport> {
        crate::reference_measure::measure_baked_vehicle(self, vehicle, Some(blueprint))
    }

    /// Measure this pack's absolute dimension anchors against the baked vehicle. `None` when the
    /// vehicle is foreign to the pack; an empty report when the pack has no anchors yet.
    pub fn measure_dimensions(&self, vehicle: &BakedVehicle) -> Option<crate::DimensionReport> {
        crate::reference_measure::measure_dimensions(self, vehicle, None)
    }

    /// Anchors against a LIVE blueprint: gear-derived anchors (track width, gauge, counts,
    /// wheel diameter) and blueprint-basis anchors (ring) follow the edited file, so the fast
    /// loop's numbers describe what the author just typed.
    pub fn measure_dimensions_live(
        &self,
        vehicle: &BakedVehicle,
        blueprint: &game_core::VehicleBlueprint,
    ) -> Option<crate::DimensionReport> {
        crate::reference_measure::measure_dimensions(self, vehicle, Some(blueprint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> ReferenceSource {
        ReferenceSource::new("test", "n/a", "synthetic")
    }

    #[test]
    fn a_plain_anchor_is_locked_and_a_pending_one_is_target() {
        let locked = DimensionTarget::new(DimensionKind::HullLength, 6.0, 0.1, source());
        assert_eq!(locked.status(), AnchorStatus::Locked);
        let pending =
            DimensionTarget::target_pending(DimensionKind::HullLength, 6.0, 0.1, source());
        assert_eq!(pending.status(), AnchorStatus::Target);
        // Target anchors still measure and still judge — they only change who fails the gate.
        assert!(pending.passes(6.05));
        assert!(!pending.passes(6.2));
    }

    #[test]
    fn serialized_anchors_without_a_status_decode_as_locked() {
        // Reports recorded before the status field existed must come back as the stricter tier.
        let json = r#"{
            "kind": "HullLength",
            "target_m": 6.0,
            "tolerance_m": 0.1,
            "source": {"label": "t", "url": "n/a", "note": ""}
        }"#;
        let decoded: DimensionTarget = serde_json::from_str(json).expect("decode");
        assert_eq!(decoded.status(), AnchorStatus::Locked);
    }

    #[test]
    fn count_kinds_know_they_are_counts() {
        assert!(DimensionKind::TrackLinkCountPerSide.is_count());
        assert!(DimensionKind::RoadWheelCount.is_count());
        assert!(!DimensionKind::HullLength.is_count());
    }
}
