use terrain::StaticCoverObject;

use crate::CameraObstacle;

/// One phase-consistent client view of live cover. Replacing this value updates movement, sight,
/// and camera geometry together while authored cover keeps stable render/scar indices.
///
/// Movement and sight are resolved SEPARATELY because they stop agreeing once rubble is involved:
/// a mound still hides and still stops shells, but it is masonry a hull can climb, not a wall.
pub(super) struct LiveCoverCache {
    phase_bytes: Vec<u8>,
    blocking: Vec<StaticCoverObject>,
    movement: Vec<StaticCoverObject>,
    rubble: Vec<terrain::RubbleMound>,
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
        let blocking = sim::sight_cover_for_phase_bytes(authored, &phase_bytes);
        let movement = sim::movement_cover_for_phase_bytes(authored, &phase_bytes);
        let rubble = sim::rubble_mounds_for_phase_bytes(authored, &phase_bytes);
        let camera_obstacles = blocking.iter().map(CameraObstacle::from_static_cover).collect();
        Self { phase_bytes, blocking, movement, rubble, camera_obstacles, replicated }
    }

    pub(super) fn phase_bytes(&self) -> &[u8] {
        &self.phase_bytes
    }

    /// What stops a shell and hides a hull: a collapsed building is still the mound it slumped
    /// into. Sight, shell traces and the camera read this.
    pub(super) fn blocking(&self) -> &[StaticCoverObject] {
        &self.blocking
    }

    /// What stops a HULL. The predictor drives against this, so it must be the same rule the
    /// server's movement uses — see `sim::live_cover_for_movement`.
    pub(super) fn movement(&self) -> &[StaticCoverObject] {
        &self.movement
    }

    /// What a hull STANDS ON that the heightmap does not know about: collapsed buildings, as
    /// debris. The predictor must read the same piles the authority does or the local hull will
    /// climb ground the server has not raised.
    pub(super) fn rubble(&self) -> &[terrain::RubbleMound] {
        &self.rubble
    }

    pub(super) fn camera_obstacles(&self) -> &[CameraObstacle] {
        &self.camera_obstacles
    }

    pub(super) fn is_replicated(&self) -> bool {
        self.replicated
    }
}
