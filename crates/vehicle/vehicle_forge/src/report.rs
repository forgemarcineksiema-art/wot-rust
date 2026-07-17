use game_core::VehicleKind;
use serde::{Deserialize, Serialize};

use crate::{DimensionKind, DimensionTarget, RatioKind, RatioTarget, ReferencePack};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredRatio {
    target: RatioTarget,
    measured: f32,
    passed: bool,
}

impl MeasuredRatio {
    pub fn new(target: RatioTarget, measured: f32) -> Self {
        let passed = target.passes(measured);
        Self { target, measured, passed }
    }

    pub fn kind(&self) -> RatioKind {
        self.target.kind()
    }

    pub fn target(&self) -> &RatioTarget {
        &self.target
    }

    pub fn measured(&self) -> f32 {
        self.measured
    }

    pub fn passed(&self) -> bool {
        self.passed
    }

    /// Signed percentage difference of the measurement from its target. This is the "how wrong",
    /// not just "pass/fail", that the Milestone 1 acceptance asks for.
    pub fn percent_difference(&self) -> f32 {
        let target = self.target.target();
        if target.abs() < f32::EPSILON {
            return 0.0;
        }
        (self.measured - target) / target * 100.0
    }
}

/// One absolute anchor measured against the baked mesh, in metres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredDimension {
    target: DimensionTarget,
    measured_m: f32,
    passed: bool,
}

impl MeasuredDimension {
    pub fn new(target: DimensionTarget, measured_m: f32) -> Self {
        let passed = target.passes(measured_m);
        Self { target, measured_m, passed }
    }

    pub fn kind(&self) -> DimensionKind {
        self.target.kind()
    }

    pub fn target(&self) -> &DimensionTarget {
        &self.target
    }

    pub fn measured_m(&self) -> f32 {
        self.measured_m
    }

    pub fn passed(&self) -> bool {
        self.passed
    }

    /// Signed error in metres — the tape-measure miss, before any percentage flattering.
    pub fn delta_m(&self) -> f32 {
        self.measured_m - self.target.target_m()
    }

    pub fn percent_difference(&self) -> f32 {
        let target = self.target.target_m();
        if target.abs() < f32::EPSILON {
            return 0.0;
        }
        (self.measured_m - target) / target * 100.0
    }
}

/// The absolute-dimension side of a reference report: metres against the dossier's anchors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionReport {
    vehicle: VehicleKind,
    measurements: Vec<MeasuredDimension>,
}

impl DimensionReport {
    pub fn new(vehicle: VehicleKind, measurements: Vec<MeasuredDimension>) -> Self {
        Self { vehicle, measurements }
    }

    pub fn vehicle(&self) -> VehicleKind {
        self.vehicle
    }

    pub fn measurements(&self) -> &[MeasuredDimension] {
        &self.measurements
    }

    pub fn measurement(&self, kind: DimensionKind) -> Option<&MeasuredDimension> {
        self.measurements.iter().find(|measurement| measurement.kind() == kind)
    }

    pub fn is_empty(&self) -> bool {
        self.measurements.is_empty()
    }

    pub fn all_pass(&self) -> bool {
        self.measurements.iter().all(MeasuredDimension::passed)
    }

    pub fn markdown_summary(&self) -> String {
        let mut out = String::from(
            "| Dimension | Measured m | Target m | Δ m | Δ% | Tolerance | Result | Source |\n",
        );
        out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |\n");
        for measurement in &self.measurements {
            let target = measurement.target();
            let result = if measurement.passed() { "pass" } else { "FAIL" };
            out.push_str(&format!(
                "| {} | {:.3} | {:.3} | {:+.3} | {:+.1}% | +/-{:.3} | {} | {} |\n",
                measurement.kind().label(),
                measurement.measured_m(),
                target.target_m(),
                measurement.delta_m(),
                measurement.percent_difference(),
                target.tolerance_m(),
                result,
                target.source().label(),
            ));
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioReport {
    vehicle: VehicleKind,
    reference: ReferencePack,
    measurements: Vec<MeasuredRatio>,
}

impl RatioReport {
    pub fn new(
        vehicle: VehicleKind,
        reference: ReferencePack,
        measurements: Vec<MeasuredRatio>,
    ) -> Self {
        Self { vehicle, reference, measurements }
    }

    pub fn vehicle(&self) -> VehicleKind {
        self.vehicle
    }

    pub fn reference(&self) -> &ReferencePack {
        &self.reference
    }

    pub fn measurements(&self) -> &[MeasuredRatio] {
        &self.measurements
    }

    pub fn measurement(&self, kind: RatioKind) -> Option<&MeasuredRatio> {
        self.measurements.iter().find(|measurement| measurement.kind() == kind)
    }

    pub fn passes(&self, kind: RatioKind) -> Option<bool> {
        self.measurement(kind).map(MeasuredRatio::passed)
    }

    pub fn all_pass(&self) -> bool {
        self.measurements.iter().all(MeasuredRatio::passed)
    }

    pub fn markdown_summary(&self) -> String {
        let mut out = format!(
            "# {} Forge reference report\n\nVehicle: {:?}\nReference: {}\n\n",
            self.reference.display_name(),
            self.vehicle,
            self.reference.family_slug()
        );
        out.push_str("| Ratio | Measured | Target | Tolerance | Δ% | Result |\n");
        out.push_str("| --- | ---: | ---: | ---: | ---: | --- |\n");
        for measurement in &self.measurements {
            let target = measurement.target();
            let result = if measurement.passed() { "pass" } else { "fail" };
            out.push_str(&format!(
                "| {:?} | {:.3} | {:.3} | +/-{:.3} | {:+.1}% | {} |\n",
                measurement.kind(),
                measurement.measured(),
                target.target(),
                target.tolerance(),
                measurement.percent_difference(),
                result
            ));
        }
        out
    }
}
