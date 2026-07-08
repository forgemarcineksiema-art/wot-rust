//! Destructible static cover: per-object health and phase, the damage rules that drive the
//! transitions, and — crucially — the "live cover" resolution every consumer sees.
//!
//! The trick that keeps this from touching 40 call sites: the shell trace, movement collision and
//! spotting LOS all take `&[StaticCoverObject]` and only care about blocking geometry. So instead
//! of threading a parallel state slice everywhere, the sim resolves ONE live cover slice from the
//! static cover + the current states ([`live_cover_for_blocking`]) and passes that — a destroyed
//! object is simply absent, a rubble mound is a lowered box. Damage maps a hit back to its object
//! with [`cover_index_at`].

use serde::{Deserialize, Serialize};
use terrain::StaticCoverObject;

/// How a cover object presents right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CoverPhase {
    /// Whole and blocking at full height.
    #[default]
    Intact,
    /// A collapsed building: a low mound that still stops a hull but lets a turret-height shot
    /// (and a sight line over it) pass.
    Rubble,
    /// Gone — flattened foliage or cleared ground. Blocks nothing.
    Gone,
}

impl CoverPhase {
    /// The compact wire encoding (protocol v21). Kept explicit so the byte never drifts with the
    /// enum's declaration order.
    pub fn to_wire(self) -> u8 {
        match self {
            CoverPhase::Intact => 0,
            CoverPhase::Rubble => 1,
            CoverPhase::Gone => 2,
        }
    }

    pub fn from_wire(byte: u8) -> Self {
        match byte {
            1 => CoverPhase::Rubble,
            2 => CoverPhase::Gone,
            _ => CoverPhase::Intact,
        }
    }
}

/// Live structural state of one cover object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverState {
    /// Remaining structural health; `u32::MAX` for indestructible objects.
    pub health: u32,
    pub phase: CoverPhase,
}

impl CoverState {
    /// A fresh, whole object, healthed from its kind (indestructible kinds get `u32::MAX`).
    pub fn fresh(object: &StaticCoverObject) -> Self {
        Self { health: object.kind.max_health().unwrap_or(u32::MAX), phase: CoverPhase::Intact }
    }
}

/// One fresh state per cover object, index-aligned with `cover`.
pub fn cover_states_for(cover: &[StaticCoverObject]) -> Vec<CoverState> {
    cover.iter().map(CoverState::fresh).collect()
}

/// The cover the world actually collides against this tick: intact objects as-authored, rubble as
/// a lowered box, and destroyed objects omitted entirely. Every blocking consumer (shell trace,
/// movement, spotting LOS) takes this in place of the raw static cover, so cover destruction
/// changes what blocks/hides without any of them knowing about phases.
pub fn live_cover_for_blocking(
    cover: &[StaticCoverObject],
    states: &[CoverState],
) -> Vec<StaticCoverObject> {
    let mut live = Vec::with_capacity(cover.len());
    for (index, object) in cover.iter().enumerate() {
        match states.get(index).map(|state| state.phase).unwrap_or_default() {
            CoverPhase::Intact => live.push(object.clone()),
            CoverPhase::Gone => {}
            CoverPhase::Rubble => {
                let frac = object.kind.rubble_height_frac();
                let full_half = object.half_extents_m[1];
                let rubble_half = full_half * frac;
                let mut mound = object.clone();
                // Keep the mound sitting on the ground: lower the centre by the height it lost.
                mound.center[1] -= full_half - rubble_half;
                mound.half_extents_m[1] = rubble_half;
                live.push(mound);
            }
        }
    }
    live
}

/// The index of the still-standing (non-Gone) cover object whose box contains `point`, if any —
/// how a shell absorbed by cover finds which object to damage. Tests against the phase-adjusted
/// box (rubble is lower) with a small skin so a surface hit still lands inside.
pub fn cover_index_at(
    point: [f32; 3],
    cover: &[StaticCoverObject],
    states: &[CoverState],
) -> Option<usize> {
    const SKIN_M: f32 = 0.15;
    for (index, object) in cover.iter().enumerate() {
        let phase = states.get(index).map(|state| state.phase).unwrap_or_default();
        if phase == CoverPhase::Gone {
            continue;
        }
        let (mut center, mut half) = (object.center, object.half_extents_m);
        if phase == CoverPhase::Rubble {
            let rubble_half = half[1] * object.kind.rubble_height_frac();
            center[1] -= half[1] - rubble_half;
            half[1] = rubble_half;
        }
        if (0..3).all(|axis| (point[axis] - center[axis]).abs() <= half[axis] + SKIN_M) {
            return Some(index);
        }
    }
    None
}

/// Apply `hp` of damage to cover object `index`. Indestructible or already-destroyed objects are
/// untouched. On reaching zero health the object collapses: a building to rubble, foliage to gone.
/// Deterministic; no RNG.
pub fn damage_cover(states: &mut [CoverState], cover: &[StaticCoverObject], index: usize, hp: u32) {
    let (Some(state), Some(object)) = (states.get_mut(index), cover.get(index)) else {
        return;
    };
    if object.kind.max_health().is_none() || state.phase != CoverPhase::Intact {
        return;
    }
    state.health = state.health.saturating_sub(hp);
    if state.health == 0 {
        state.phase =
            if object.kind.leaves_rubble() { CoverPhase::Rubble } else { CoverPhase::Gone };
    }
}

/// Flatten a crushable cover object under a hull that drove into it: it goes straight to Gone
/// (a hedgerow does not become rubble). Returns `true` if it crushed something this call.
pub fn crush_cover(states: &mut [CoverState], object: &StaticCoverObject, index: usize) -> bool {
    if !object.kind.is_crushable() {
        return false;
    }
    let Some(state) = states.get_mut(index) else {
        return false;
    };
    if state.phase == CoverPhase::Gone {
        return false;
    }
    state.health = 0;
    state.phase = CoverPhase::Gone;
    true
}

/// Coarse XZ test: is a hull at `hull_center` (with plan reach `hull_reach_m`, roughly its
/// half-length) overlapping the cover box? Deliberately coarse — flattening flimsy foliage does
/// not need the movement SAT; the hull is essentially on top of the hedge.
pub fn hull_overlaps_cover_xz(
    hull_center: [f32; 3],
    hull_reach_m: f32,
    object: &StaticCoverObject,
) -> bool {
    let dx = (hull_center[0] - object.center[0]).abs();
    let dz = (hull_center[2] - object.center[2]).abs();
    dx <= object.half_extents_m[0] + hull_reach_m && dz <= object.half_extents_m[2] + hull_reach_m
}

#[cfg(test)]
mod tests {
    use terrain::StaticCoverKind;

    use super::*;

    fn object(
        id: &str,
        kind: StaticCoverKind,
        center: [f32; 3],
        half: [f32; 3],
    ) -> StaticCoverObject {
        StaticCoverObject {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            center,
            half_extents_m: half,
        }
    }

    #[test]
    fn a_building_collapses_to_a_lower_rubble_box_that_still_blocks_in_plan() {
        let cover =
            vec![object("barn", StaticCoverKind::FarmBuilding, [0.0, 3.0, 0.0], [5.0, 3.0, 4.0])];
        let mut states = cover_states_for(&cover);
        damage_cover(&mut states, &cover, 0, 10_000);
        assert_eq!(states[0].phase, CoverPhase::Rubble);

        let live = live_cover_for_blocking(&cover, &states);
        assert_eq!(live.len(), 1, "a rubble mound still blocks");
        assert!(live[0].half_extents_m[1] < 3.0, "the mound is lower than the building");
        assert_eq!(live[0].half_extents_m[0], 5.0, "its footprint (plan) is unchanged");
        // The mound sits on the ground, not floating at the old centre height.
        assert!(live[0].center[1] < 3.0);
    }

    #[test]
    fn foliage_goes_fully_gone_and_stops_blocking() {
        let cover =
            vec![object("hedge", StaticCoverKind::TreeLine, [0.0, 2.0, 0.0], [10.0, 2.0, 1.0])];
        let mut states = cover_states_for(&cover);
        damage_cover(&mut states, &cover, 0, 10_000);
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert!(live_cover_for_blocking(&cover, &states).is_empty(), "gone foliage blocks nothing");
    }

    #[test]
    fn rail_and_wreck_cover_are_indestructible() {
        let cover = vec![
            object("rail", StaticCoverKind::RailCover, [0.0, 1.0, 0.0], [3.0, 1.0, 1.0]),
            object("hulk", StaticCoverKind::Wreck, [9.0, 1.0, 0.0], [2.0, 1.0, 3.0]),
        ];
        let mut states = cover_states_for(&cover);
        damage_cover(&mut states, &cover, 0, u32::MAX);
        damage_cover(&mut states, &cover, 1, u32::MAX);
        assert_eq!(states[0].phase, CoverPhase::Intact);
        assert_eq!(states[1].phase, CoverPhase::Intact);
        assert_eq!(live_cover_for_blocking(&cover, &states).len(), 2);
    }

    #[test]
    fn a_hull_crushes_a_hedgerow_it_drives_into_but_not_a_building() {
        let cover = vec![
            object("hedge", StaticCoverKind::TreeLine, [0.0, 1.0, 0.0], [8.0, 1.0, 0.5]),
            object("barn", StaticCoverKind::FarmBuilding, [40.0, 2.0, 0.0], [5.0, 2.0, 4.0]),
        ];
        let mut states = cover_states_for(&cover);
        assert!(hull_overlaps_cover_xz([0.0, 0.0, 0.3], 3.0, &cover[0]));
        assert!(crush_cover(&mut states, &cover[0], 0), "the hedge is crushed");
        assert!(!crush_cover(&mut states, &cover[1], 1), "the barn is not crushable");
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert_eq!(states[1].phase, CoverPhase::Intact);
    }

    #[test]
    fn a_shell_hit_maps_to_the_cover_it_struck() {
        let cover = vec![
            object("a", StaticCoverKind::FarmBuilding, [0.0, 2.0, 0.0], [4.0, 2.0, 4.0]),
            object("b", StaticCoverKind::FarmBuilding, [30.0, 2.0, 0.0], [4.0, 2.0, 4.0]),
        ];
        let states = cover_states_for(&cover);
        assert_eq!(cover_index_at([30.5, 2.0, 1.0], &cover, &states), Some(1));
        assert_eq!(
            cover_index_at([100.0, 2.0, 0.0], &cover, &states),
            None,
            "open air hits nothing"
        );
    }
}
