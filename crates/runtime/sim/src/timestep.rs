use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedTimestep {
    hz: u32,
}

impl FixedTimestep {
    pub fn from_hz(hz: u32) -> Self {
        assert!(hz > 0, "simulation tick rate must be greater than zero");
        Self { hz }
    }

    pub fn hz(self) -> u32 {
        self.hz
    }

    pub fn dt_seconds(self) -> f32 {
        // Hz is the source of truth; derive dt on demand so a Hz round-trip cannot
        // drift and the stored value is exact (integer) rather than a lossy f32.
        1.0 / self.hz as f32
    }
}
