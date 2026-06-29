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
        }
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

    pub fn measure_baked_vehicle(&self, vehicle: &BakedVehicle) -> Option<RatioReport> {
        crate::reference_measure::measure_baked_vehicle(self, vehicle)
    }
}
