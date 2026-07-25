use terrain::StaticCoverObject;

use crate::CameraObstacle;

/// One phase-consistent client view of blocking cover. Replacing this value updates movement,
/// sight, and camera geometry together while authored cover keeps stable render/scar indices.
pub(super) struct LiveCoverCache {
    phase_bytes: Vec<u8>,
    blocking: Vec<StaticCoverObject>,
    camera_obstacles: Vec<CameraObstacle>,
    replicated: bool,
}

impl LiveCoverCache {
    pub(super) fn from_born_phases(authored: &[StaticCoverObject]) -> Self {
        let phases = terrain::initial_cover_phase_bytes(authored);
        Self::build(authored, phases, false)
    }

    /// Reject incomplete arrays so startup keeps the authored born phases until a complete
    /// snapshot arrives. A late join with a complete snapshot starts directly from its live world.
    pub(super) fn from_replicated(
        authored: &[StaticCoverObject],
        phase_bytes: &[u8],
    ) -> Option<Self> {
        (phase_bytes.len() == authored.len())
            .then(|| Self::build(authored, phase_bytes.to_vec(), true))
    }

    fn build(authored: &[StaticCoverObject], phase_bytes: Vec<u8>, replicated: bool) -> Self {
        let blocking = sim::live_cover_for_phase_bytes(authored, &phase_bytes);
        let camera_obstacles = blocking.iter().map(CameraObstacle::from_static_cover).collect();
        Self { phase_bytes, blocking, camera_obstacles, replicated }
    }

    pub(super) fn phase_bytes(&self) -> &[u8] {
        &self.phase_bytes
    }

    pub(super) fn blocking(&self) -> &[StaticCoverObject] {
        &self.blocking
    }

    pub(super) fn camera_obstacles(&self) -> &[CameraObstacle] {
        &self.camera_obstacles
    }

    pub(super) fn is_replicated(&self) -> bool {
        self.replicated
    }
}
