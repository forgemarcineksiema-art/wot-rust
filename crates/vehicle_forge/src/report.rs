use game_core::VehicleKind;
use serde::{Deserialize, Serialize};

use crate::{RatioKind, RatioTarget, ReferencePack};

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
            "# T-54/T-55 Forge reference report\n\nVehicle: {:?}\nReference: {}\n\n",
            self.vehicle,
            self.reference.family_slug()
        );
        out.push_str("| Ratio | Measured | Target | Tolerance | Result |\n");
        out.push_str("| --- | ---: | ---: | ---: | --- |\n");
        for measurement in &self.measurements {
            let target = measurement.target();
            let result = if measurement.passed() { "pass" } else { "fail" };
            out.push_str(&format!(
                "| {:?} | {:.3} | {:.3} | +/-{:.3} | {} |\n",
                measurement.kind(),
                measurement.measured(),
                target.target(),
                target.tolerance(),
                result
            ));
        }
        out
    }
}
