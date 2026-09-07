use terrain::StaticCoverObject;

use crate::CameraObstacle;

/// One phase-consistent client view of live cover. Replacing this value updates movement, sight,
/// and camera geometry together while authored cover keeps stable render/scar indices.
///
/// Movement and sight are resolved SEPARATELY because they stop agreeing once rubble is involved:
/// a mound still hides and still stops shells, but it is masonry a hull can climb, not a wall.
pub(super) struct LiveCoverCache {
    phase_bytes: Vec<u8>,
    /// Z9: the packed wall segments, `terrain::SEGMENT_BYTES` per object (empty: all whole).
    segment_bytes: Vec<u8>,
    /// Z13: where the blown-off turrets rest — low solids in `blocking`, never in `movement`.
    turret_rests: Vec<[f32; 3]>,
    blocking: Vec<StaticCoverObject>,
    movement: Vec<StaticCoverObject>,
    rubble: Vec<terrain::RubbleMound>,
    camera_obstacles: Vec<CameraObstacle>,
    replicated: bool,
}

impl LiveCoverCache {
    pub(super) fn from_born_phases(authored: &[StaticCoverObject]) -> Self {
        let phases = terrain::initial_cover_phase_bytes(authored);
        Self::build(authored, phases, Vec::new(), Vec::new(), false)
    }

    /// Reject incomplete arrays so startup keeps the authored born phases until a complete
    /// snapshot arrives. A late join with a complete snapshot starts directly from its live world.
    /// The segments (Z9) may be absent — an older host — and then read as every wall whole; the
    /// turret rests (Z13) are however many the host has landed.
    pub(super) fn from_replicated(
        authored: &[StaticCoverObject],
        phase_bytes: &[u8],
        segment_bytes: &[u8],
        turret_rests: &[[f32; 3]],
    ) -> Option<Self> {
        let complete = segment_bytes.len() == authored.len() * terrain::SEGMENT_BYTES;
        let segments = if complete { segment_bytes.to_vec() } else { Vec::new() };
        (phase_bytes.len() == authored.len()).then(|| {
            Self::build(authored, phase_bytes.to_vec(), segments, turret_rests.to_vec(), true)
        })
    }

    fn build(
        authored: &[StaticCoverObject],
        phase_bytes: Vec<u8>,
        segment_bytes: Vec<u8>,
        turret_rests: Vec<[f32; 3]>,
        replicated: bool,
    ) -> Self {
        let blocking =
            sim::sight_cover_for_wire(authored, &phase_bytes, &segment_bytes, &turret_rests);
        let movement = sim::movement_cover_for_phase_bytes(authored, &phase_bytes);
        let rubble = sim::rubble_mounds_for_phase_bytes(authored, &phase_bytes);
        // The boom's solids keep a mound as the lowered box it always was for the camera (X11:
        // the sight slice carries the pyramid in `rubble` instead); presentation, not honesty.
        let camera_obstacles =
            sim::camera_cover_for_wire(authored, &phase_bytes, &segment_bytes, &turret_rests)
                .iter()
                .map(CameraObstacle::from_static_cover)
                .collect();
        Self {
            phase_bytes,
            segment_bytes,
            turret_rests,
            blocking,
            movement,
            rubble,
            camera_obstacles,
            replicated,
        }
    }

    pub(super) fn phase_bytes(&self) -> &[u8] {
        &self.phase_bytes
    }

    /// The packed wall segments (Z9), `terrain::SEGMENT_BYTES` per object; empty when the
    /// host never sent them.
    pub(super) fn segment_bytes(&self) -> &[u8] {
        &self.segment_bytes
    }

    /// Where the blown-off turrets rest (Z13).
    pub(super) fn turret_rests(&self) -> &[[f32; 3]] {
        &self.turret_rests
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

/// The rests a snapshot carries (Z13), as the resolver takes them.
pub(super) fn rests_from_wire(rests: &[net::TurretRest]) -> Vec<[f32; 3]> {
    rests.iter().map(|rest| rest.position).collect()
}
