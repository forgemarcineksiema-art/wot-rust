use game_core::VehicleKind;
use serde::{Deserialize, Serialize};
use vehicle_geometry::BakedVehicle;

use crate::RatioReport;

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

/// A measurable absolute dimension of the baked vehicle, in metres. Ratios alone pass at the
/// wrong scale — these are the anchors that pin the model to the real tank's tape measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimensionKind {
    /// Z-extent of the visual hull including the running gear (hull length over tracks).
    HullLength,
    /// X-extent of the visual hull including the running gear (width over tracks).
    HullWidth,
    /// Highest exterior point of hull or turret above the ground plane (height to turret roof;
    /// cupolas and fittings count — they are part of the silhouette).
    HeightToTurretRoof,
    /// Muzzle to hull rear (overall length, gun forward).
    OverallLengthWithGun,
    /// Road-wheel diameter, straight from the running-gear kinematics.
    RoadWheelDiameter,
}

impl DimensionKind {
    pub fn label(self) -> &'static str {
        match self {
            DimensionKind::HullLength => "hull length (over tracks)",
            DimensionKind::HullWidth => "width (over tracks)",
            DimensionKind::HeightToTurretRoof => "height to turret roof",
            DimensionKind::OverallLengthWithGun => "overall length (gun forward)",
            DimensionKind::RoadWheelDiameter => "road wheel diameter",
        }
    }
}

/// One real-world dimension the baked model must honour, with the source that documents it.
/// The tolerance is absolute (metres), never a percentage — a tape measure, not a vibe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionTarget {
    kind: DimensionKind,
    target_m: f32,
    tolerance_m: f32,
    source: ReferenceSource,
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
        Self { kind, target_m, tolerance_m, source }
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
}

impl ReferencePack {
    #[allow(clippy::too_many_arguments)]
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
        }
    }

    /// Attach absolute dimension anchors (the dossier's tape-measure numbers).
    pub fn with_dimensions(mut self, dimensions: Vec<DimensionTarget>) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// The reference pack for `kind`, resolved through the central forge registry so the pack, the
    /// part-graph strategy, and the review cameras all stay registered in one place.
    pub fn for_vehicle(kind: VehicleKind) -> Option<Self> {
        Some((crate::registry::forge_spec(kind)?.reference_pack)())
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

    pub fn measure_baked_vehicle(&self, vehicle: &BakedVehicle) -> Option<RatioReport> {
        crate::reference_measure::measure_baked_vehicle(self, vehicle)
    }

    /// Measure this pack's absolute dimension anchors against the baked vehicle. `None` when the
    /// vehicle is foreign to the pack; an empty report when the pack has no anchors yet.
    pub fn measure_dimensions(&self, vehicle: &BakedVehicle) -> Option<crate::DimensionReport> {
        crate::reference_measure::measure_dimensions(self, vehicle)
    }
}
