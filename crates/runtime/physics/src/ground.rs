//! The ground under a track, in layers (the one program's X4): the terrain, raised wherever
//! collapsed masonry stands on it — and raised wherever a LOW SOLID stands within the running
//! gear's step of the hull's current support.
//!
//! Before this, nothing was climbable but terrain and rubble: a knee-high parapet blocked in
//! plan like a tenement (P2.2 of `docs/contact-and-tracks-program.md` measured out because no
//! content under 0.8 m existed). Now a solid whose top is no higher than the support plus the
//! step enters the support envelope exactly as a mound does — the rigid beam climbs it, the
//! hull tilts and pays the slope — and, by the SAME predicate, the separating-axis test lets the
//! hull's plan onto it ([`crate::TankObstacle::climbing`]). One rule, two readers.
//!
//! The step is the vehicle's ([`game_core::HullPlan::step_m`]: the belt's top run less a
//! clearance, 0.68–0.81 m across the fleet, the T-54 at the dossier's ~0.8 m vertical
//! obstacle), and it is measured from the CURRENT support: a hull already carried up a mound
//! steps onto a wall it could not have climbed from the street.

use glam::Vec3;
use terrain::{CoverBox, HeightMap, RubbleMound, StaticCoverObject};

use crate::collision::TankFootprint;

/// A standing solid the hull may climb this tick: its plan and the height of its top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepSolid {
    pub plan: CoverBox,
    pub top_m: f32,
}

impl StepSolid {
    const EMPTY: Self =
        Self { plan: CoverBox { center: [0.0; 3], half: [0.0; 3], yaw_rad: 0.0 }, top_m: 0.0 };
}

/// How many low solids one hull can be over at once. A parapet, the kerb beside it and a
/// field wall is three; eight is a ceiling nothing authored reaches, and it keeps the list on
/// the stack so the per-tick gather allocates nothing.
pub const MAX_STEP_SOLIDS: usize = 8;

/// The low solids near one hull this tick — the output of [`step_solids_near`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepSolids {
    items: [StepSolid; MAX_STEP_SOLIDS],
    count: usize,
}

impl StepSolids {
    pub const NONE: Self = Self { items: [StepSolid::EMPTY; MAX_STEP_SOLIDS], count: 0 };

    pub fn as_slice(&self) -> &[StepSolid] {
        &self.items[..self.count]
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn len(&self) -> usize {
        self.count
    }
}

/// Everything under a track that is not the heightmap: the debris of collapsed buildings and the
/// low solids within the hull's step. Both empty on a battlefield nothing has been knocked down
/// on and away from every low wall, which is the path that stays bit-identical.
#[derive(Debug, Clone, Copy)]
pub struct GroundLayers<'a> {
    pub rubble: &'a [RubbleMound],
    pub steps: &'a [StepSolid],
}

impl<'a> GroundLayers<'a> {
    pub const NONE: GroundLayers<'static> = GroundLayers { rubble: &[], steps: &[] };

    /// Rubble alone — every caller that has no hull to measure a step for.
    pub fn rubble(rubble: &'a [RubbleMound]) -> Self {
        Self { rubble, steps: &[] }
    }

    pub fn with_steps(self, steps: &'a [StepSolid]) -> Self {
        Self { steps, ..self }
    }
}

/// THE rule (X4): a solid is a step for a hull when its top is no higher than the hull's
/// support plus the hull's step. Read by the support envelope (the solid raises the ground) and
/// by the separating-axis test (the solid does not block) — the same predicate, so a hull is
/// never let onto a solid that will not carry it, nor carried by one it cannot reach.
pub fn is_step_for(top_m: f32, support_y: f32, step_m: f32) -> bool {
    top_m <= support_y + step_m
}

/// The low solids within `reach_m` of `position` that are steps for a hull of `footprint`
/// standing at `position.y`, in cover order (deterministic), at most [`MAX_STEP_SOLIDS`].
pub fn step_solids_near(
    cover: &[StaticCoverObject],
    position: Vec3,
    footprint: TankFootprint,
    reach_m: f32,
) -> StepSolids {
    let mut out = StepSolids::NONE;
    for object in cover {
        if out.count == MAX_STEP_SOLIDS {
            break;
        }
        let top_m = object.center[1] + object.half_extents_m[1];
        if !is_step_for(top_m, position.y, footprint.step_m) {
            continue;
        }
        let plan = CoverBox::of(object);
        let [x0, z0, x1, z1] = plan.bounds_xz();
        if position.x < x0 - reach_m
            || position.x > x1 + reach_m
            || position.z < z0 - reach_m
            || position.z > z1 + reach_m
        {
            continue;
        }
        out.items[out.count] = StepSolid { plan, top_m };
        out.count += 1;
    }
    out
}

/// The one surface every station and every probe reads: the terrain, raised wherever collapsed
/// masonry or a low solid stands on it. With nothing in either layer this IS the heightmap
/// sample, expression for expression.
pub(crate) fn surface_at(
    heightmap: &HeightMap,
    point: Vec3,
    layers: GroundLayers<'_>,
) -> Option<f32> {
    let terrain = heightmap.sample_height(point.x, point.z);
    if layers.rubble.is_empty() && layers.steps.is_empty() {
        return terrain;
    }
    let mut raised = terrain::rubble_height_at(layers.rubble, point.x, point.z);
    for step in layers.steps {
        if step.plan.contains_xz(point.x, point.z, 0.0) {
            raised = Some(raised.map_or(step.top_m, |current: f32| current.max(step.top_m)));
        }
    }
    terrain::ground_with_rubble(terrain, raised)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain::StaticCoverKind;

    fn wall(top_m: f32) -> StaticCoverObject {
        StaticCoverObject {
            id: "wall".into(),
            name: "wall".into(),
            kind: StaticCoverKind::LowWall,
            center: [50.0, top_m * 0.5, 50.0],
            half_extents_m: [6.0, top_m * 0.5, 0.5],
            yaw_rad: 0.0,
        }
    }

    fn hull() -> TankFootprint {
        TankFootprint { half_width_m: 1.6, half_length_m: 3.1, height_m: 2.5, step_m: 0.8 }
    }

    /// The gather applies the rule from the CURRENT support: the same wall is a step for a hull
    /// standing high enough and a wall for one that is not.
    #[test]
    fn a_solid_is_a_step_only_from_a_support_within_its_reach() {
        let walls = [wall(1.2)];
        let low = step_solids_near(&walls, Vec3::new(50.0, 0.0, 47.0), hull(), 4.0);
        assert!(low.is_empty(), "1.2 m over a 0.8 m step is a wall from the street");
        let high = step_solids_near(&walls, Vec3::new(50.0, 0.5, 47.0), hull(), 4.0);
        assert_eq!(high.len(), 1, "...and a step from half a metre up");
        let far = step_solids_near(&walls, Vec3::new(50.0, 0.5, 30.0), hull(), 4.0);
        assert!(far.is_empty(), "out of reach it is not gathered");
    }

    /// The surface over a step is its top inside its plan and the terrain outside; with no
    /// layer at all it is the heightmap sample itself.
    #[test]
    fn the_surface_over_a_step_is_its_top_and_the_terrain_beside_it() {
        let map = HeightMap::flat(64, 64, 2.0, 0.0).expect("flat");
        let walls = [wall(0.6)];
        let steps = step_solids_near(&walls, Vec3::new(50.0, 0.0, 49.0), hull(), 4.0);
        let layers = GroundLayers::NONE.with_steps(steps.as_slice());
        assert_eq!(surface_at(&map, Vec3::new(50.0, 0.0, 50.0), layers), Some(0.6));
        assert_eq!(surface_at(&map, Vec3::new(50.0, 0.0, 52.0), layers), Some(0.0));
        assert_eq!(surface_at(&map, Vec3::new(50.0, 0.0, 50.0), GroundLayers::NONE), Some(0.0));
    }
}
