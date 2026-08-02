#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatePrecision {
    F32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LargeWorldStrategy {
    OriginRebaseAboveMeters(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldCoordinatePolicy {
    simulation_precision: CoordinatePrecision,
    renderer_precision: CoordinatePrecision,
    large_world_strategy: LargeWorldStrategy,
}

impl WorldCoordinatePolicy {
    pub fn wot_like_maps() -> Self {
        Self {
            simulation_precision: CoordinatePrecision::F32,
            renderer_precision: CoordinatePrecision::F32,
            large_world_strategy: LargeWorldStrategy::OriginRebaseAboveMeters(4096.0),
        }
    }

    pub fn simulation_precision(self) -> CoordinatePrecision {
        self.simulation_precision
    }

    pub fn renderer_precision(self) -> CoordinatePrecision {
        self.renderer_precision
    }

    pub fn large_world_strategy(self) -> LargeWorldStrategy {
        self.large_world_strategy
    }

    pub fn fits_without_rebase(self, max_extent_m: f32) -> bool {
        match self.large_world_strategy {
            LargeWorldStrategy::OriginRebaseAboveMeters(threshold_m) => max_extent_m <= threshold_m,
        }
    }
}
