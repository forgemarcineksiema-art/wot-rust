//! Destructible static cover: per-object health and phase, the damage rules that drive the
//! transitions, and — crucially — the "live cover" resolution every consumer sees.
//!
//! The trick that keeps this from touching 40 call sites: the shell trace, movement collision and
//! spotting LOS all take `&[StaticCoverObject]` and only care about blocking geometry. So instead
//! of threading a parallel state slice everywhere, the sim resolves the live cover from the static
//! cover + the current states and passes that — a destroyed object is simply absent, a rubble
//! mound is a lowered box. Damage maps a hit back to its object with [`cover_index_at`].
//!
//! There are TWO resolutions, not one, because rubble means different things to different
//! consumers: see [`CoverPurpose`], [`live_cover_for_sight_and_shells`] and
//! [`live_cover_for_movement`].

use serde::{Deserialize, Serialize};
use terrain::{RubbleMound, StaticCoverObject};

/// How a cover object presents right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CoverPhase {
    /// Whole and blocking at full height.
    #[default]
    Intact,
    /// A collapsed building: debris. It still stops a shell and hides what is behind it below its
    /// crest, but for MOVEMENT it is no longer an obstacle at all — it is ground, and a hull
    /// climbs it (see [`rubble_mounds`] and `terrain::RubbleMound`).
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

/// The states a battle STARTS from (urban-map program PR-07): fresh everywhere, except that
/// born-ruins (`terrain::born_cover_phase_byte`) begin already collapsed at zero health. The
/// server's lazy init and the client's pre-snapshot bake both read the same birth rule, so a
/// battle opens on the same ruined skyline everywhere — and snapshots re-send whole states,
/// so convergence stays free.
pub fn initial_cover_states(cover: &[StaticCoverObject]) -> Vec<CoverState> {
    cover
        .iter()
        .map(|object| match terrain::born_cover_phase_byte(object) {
            0 => CoverState::fresh(object),
            byte => CoverState { health: 0, phase: CoverPhase::from_wire(byte) },
        })
        .collect()
}

/// What a consumer is asking the live cover FOR.
///
/// One resolved slice used to answer for everybody, which worked exactly as long as every
/// consumer wanted the same geometry. Rubble is where that stops being true: a mound still hides
/// what is behind it and still stops a shell below its crest, but it is a pile of broken masonry,
/// not a wall — a hull climbs it. The two questions therefore get two answers, and every call
/// site has to say which one it is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoverPurpose {
    /// Shell traces, spotting LOS, camera solids: a mound is a lowered box that still blocks.
    SightAndShells,
    /// Horizontal movement collision.
    Movement,
}

/// The cover the world actually collides against this tick: intact objects as-authored, rubble as
/// a lowered box, and destroyed objects omitted entirely.
///
/// A battlefield with nothing broken on it yet BORROWS the authored slice. That is the common
/// case for most of a battle, and it is worth a `Cow`: a [`StaticCoverObject`] carries two
/// `String`s, so rebuilding this every tick cost a fresh heap allocation per name per object —
/// on a city map (90–160 boxes) some 300 allocations a tick, 18 000 a second, for a slice that
/// is byte-for-byte the input. The server already hand-rolled exactly this `Cow` for the bots'
/// copy; the rule belongs here, once, where every caller gets it.
fn live_cover_for<'a>(
    cover: &'a [StaticCoverObject],
    states: &[CoverState],
    purpose: CoverPurpose,
) -> std::borrow::Cow<'a, [StaticCoverObject]> {
    if states.iter().all(|state| state.phase == CoverPhase::Intact) {
        return std::borrow::Cow::Borrowed(cover);
    }
    let mut live = Vec::with_capacity(cover.len());
    for (index, object) in cover.iter().enumerate() {
        match states.get(index).map(|state| state.phase).unwrap_or_default() {
            CoverPhase::Intact => live.push(object.clone()),
            CoverPhase::Gone => {}
            // Debris is not an obstacle to a hull — it is the ground the hull stands on, and it
            // reaches the drive through the support envelope instead (`rubble_mounds`). Leaving
            // it in the movement slice was the whole reason a flattened block still walled a
            // tank exactly as the standing block had.
            CoverPhase::Rubble if purpose == CoverPurpose::Movement => {}
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
    std::borrow::Cow::Owned(live)
}

/// The cover a shell trace, a spotting sight line or the camera meets: intact objects
/// as-authored, a collapsed building as the low mound it slumped into, cleared ground absent.
pub fn live_cover_for_sight_and_shells<'a>(
    cover: &'a [StaticCoverObject],
    states: &[CoverState],
) -> std::borrow::Cow<'a, [StaticCoverObject]> {
    live_cover_for(cover, states, CoverPurpose::SightAndShells)
}

/// The cover a hull's movement collides against.
pub fn live_cover_for_movement<'a>(
    cover: &'a [StaticCoverObject],
    states: &[CoverState],
) -> std::borrow::Cow<'a, [StaticCoverObject]> {
    live_cover_for(cover, states, CoverPurpose::Movement)
}

/// The collapsed buildings on this battlefield, as the GROUND a hull drives on (see
/// [`terrain::RubbleMound`]). Built from the AUTHORED boxes, so the pile a hull climbs and the
/// mound a shell meets are the same debris. Empty until something comes down, which is what keeps
/// an untouched battlefield bit-identical.
pub fn rubble_mounds(cover: &[StaticCoverObject], states: &[CoverState]) -> Vec<RubbleMound> {
    cover
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            states.get(*index).map(|state| state.phase) == Some(CoverPhase::Rubble)
        })
        .map(|(_, object)| RubbleMound::from_cover(object))
        .collect()
}

/// The same, from replicated phase bytes — what the client predictor drives on.
pub fn rubble_mounds_for_phase_bytes(
    cover: &[StaticCoverObject],
    phase_bytes: &[u8],
) -> Vec<RubbleMound> {
    rubble_mounds(cover, &states_from_phase_bytes(phase_bytes))
}

/// Both live-cover resolutions for one tick, sharing the work whenever they are equal.
///
/// They differ ONLY over rubble, and rubble is rare: a battle that has swept a fence away has two
/// identical answers, and resolving them separately would pay the whole rebuild twice — on a city
/// map that is 150 objects with two `String`s each, which is exactly the allocation this module
/// went to some trouble to avoid. So the movement slice is materialised only when a mound actually
/// exists; otherwise both views borrow the one that was built.
pub struct LiveCover<'a> {
    sight: std::borrow::Cow<'a, [StaticCoverObject]>,
    movement: Option<std::borrow::Cow<'a, [StaticCoverObject]>>,
}

impl<'a> LiveCover<'a> {
    pub fn resolve(cover: &'a [StaticCoverObject], states: &[CoverState]) -> Self {
        let sight = live_cover_for_sight_and_shells(cover, states);
        let movement = states
            .iter()
            .any(|state| state.phase == CoverPhase::Rubble)
            .then(|| live_cover_for_movement(cover, states));
        Self { sight, movement }
    }

    /// What stops a shell and hides a hull.
    pub fn sight(&self) -> &[StaticCoverObject] {
        &self.sight
    }

    /// What stops a HULL.
    pub fn movement(&self) -> &[StaticCoverObject] {
        self.movement.as_deref().unwrap_or(&self.sight)
    }
}

fn states_from_phase_bytes(phase_bytes: &[u8]) -> Vec<CoverState> {
    phase_bytes
        .iter()
        .map(|&byte| CoverState { health: 0, phase: CoverPhase::from_wire(byte) })
        .collect()
}

/// Resolve replicated phase bytes into the exact sight/shell geometry used by the authoritative
/// simulation. Health stays server-only; `Intact`, `Rubble`, and `Gone` geometry does not.
pub fn sight_cover_for_phase_bytes(
    cover: &[StaticCoverObject],
    phase_bytes: &[u8],
) -> Vec<StaticCoverObject> {
    live_cover_for_sight_and_shells(cover, &states_from_phase_bytes(phase_bytes)).into_owned()
}

/// ...and into the movement geometry, which is what the client predictor must drive against if
/// it is to stop where the server stops it.
pub fn movement_cover_for_phase_bytes(
    cover: &[StaticCoverObject],
    phase_bytes: &[u8],
) -> Vec<StaticCoverObject> {
    live_cover_for_movement(cover, &states_from_phase_bytes(phase_bytes)).into_owned()
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

/// Record the wound one absorbed shell leaves on a cover face (protocol v32): which face took
/// the hit, where across it, how wide, and whether it is a kinetic inset or an HE bite. Purely
/// visual state — blocking never reads it — but replicated, so every client and a late joiner
/// dress the same wall with the same scars. Per-cover cap: the oldest wound weathers away.
pub fn record_cover_scar(
    ledger: &mut Vec<terrain::CoverScar>,
    cover_index: usize,
    object: &StaticCoverObject,
    impact: &game_core::ShellImpact,
) {
    let local = [
        impact.position.x - object.center[0],
        impact.position.y - object.center[1],
        impact.position.z - object.center[2],
    ];
    let normalized: Vec<f32> =
        (0..3).map(|axis| local[axis] / object.half_extents_m[axis].max(0.05)).collect();
    // The struck face is the axis the hit sits furthest along; roof hits map to +Y.
    let (face, u_axis, v_axis) = {
        let ax = normalized[0].abs();
        let ay = normalized[1].abs();
        let az = normalized[2].abs();
        if ay >= ax && ay >= az {
            (4u8, 0usize, 2usize)
        } else if ax >= az {
            (if normalized[0] >= 0.0 { 0u8 } else { 1u8 }, 2usize, 1usize)
        } else {
            (if normalized[2] >= 0.0 { 2u8 } else { 3u8 }, 0usize, 1usize)
        }
    };
    let quantize = |n: f32| (((n + 1.0) * 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
    let radius_m = if impact.shell_type == game_core::ShellType::HighExplosive {
        (impact.caliber_mm * 0.008).clamp(0.3, 1.5)
    } else {
        (impact.caliber_mm * 0.0006).clamp(0.04, 0.15)
    };
    let scar = terrain::CoverScar {
        cover: cover_index as u16,
        face,
        u_q: quantize(normalized[u_axis]),
        v_q: quantize(normalized[v_axis]),
        radius_q: ((radius_m / terrain::COVER_SCAR_RADIUS_STEP_M).round() as u8).max(1),
        kind: if impact.shell_type == game_core::ShellType::HighExplosive {
            terrain::COVER_SCAR_KIND_HIGH_EXPLOSIVE
        } else {
            terrain::COVER_SCAR_KIND_KINETIC
        },
    };
    let on_this_cover = ledger.iter().filter(|s| s.cover == scar.cover).count();
    if on_this_cover >= terrain::MAX_COVER_SCARS_PER_COVER
        && let Some(oldest) = ledger.iter().position(|s| s.cover == scar.cover)
    {
        ledger.remove(oldest);
    }
    ledger.push(scar);
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

        let live = live_cover_for_sight_and_shells(&cover, &states);
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
        assert!(
            live_cover_for_sight_and_shells(&cover, &states).is_empty(),
            "gone foliage blocks nothing"
        );
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
        assert_eq!(live_cover_for_sight_and_shells(&cover, &states).len(), 2);
    }

    #[test]
    fn a_hull_crushes_a_hedgerow_it_drives_into_but_not_a_building() {
        let cover = vec![
            object("hedge", StaticCoverKind::TreeLine, [0.0, 1.0, 0.0], [8.0, 1.0, 0.5]),
            object("barn", StaticCoverKind::FarmBuilding, [40.0, 2.0, 0.0], [5.0, 2.0, 4.0]),
        ];
        let mut states = cover_states_for(&cover);
        assert!(crush_cover(&mut states, &cover[0], 0), "the hedge is crushed");
        assert!(!crush_cover(&mut states, &cover[1], 1), "the barn is not crushable");
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert_eq!(states[1].phase, CoverPhase::Intact);
    }

    /// Urban-map doctrine decision 2, as tests: a CityBuilding soaks 1500 HP and collapses
    /// to the standard rubble mound (hull blocked, turret-height shot clears); a StoneWall
    /// opens at 150 HP and goes fully GONE — a breached wall is a door, never a mound.
    #[test]
    fn a_city_building_is_masonry_and_a_stone_wall_breaches_clean() {
        let cover = vec![
            object("tenement_a", StaticCoverKind::CityBuilding, [0.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
            object("yard_wall", StaticCoverKind::StoneWall, [30.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
        ];
        let mut states = cover_states_for(&cover);

        damage_cover(&mut states, &cover, 0, 1499);
        assert_eq!(states[0].phase, CoverPhase::Intact, "1499 HP does not fell masonry");
        damage_cover(&mut states, &cover, 0, 1);
        assert_eq!(states[0].phase, CoverPhase::Rubble, "the block collapses at 1500");

        damage_cover(&mut states, &cover, 1, 150);
        assert_eq!(states[1].phase, CoverPhase::Gone, "a breached wall leaves no mound");

        let live = live_cover_for_sight_and_shells(&cover, &states);
        assert_eq!(live.len(), 1, "the wall is a clear door; the rubble still stands");
        assert!(
            live[0].half_extents_m[1] < 5.5 * 0.5,
            "the mound is low enough for a turret-height shot"
        );
    }

    /// A 30 t hull breaches a brick garden wall by driving through it; a city building
    /// stops the hull like any building.
    #[test]
    fn a_hull_crushes_a_stone_wall_but_not_a_city_building() {
        let cover = vec![
            object("yard_wall", StaticCoverKind::StoneWall, [0.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
            object("tenement", StaticCoverKind::CityBuilding, [30.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
        ];
        let mut states = cover_states_for(&cover);
        assert!(crush_cover(&mut states, &cover[0], 0), "the wall crushes under the hull");
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert!(!crush_cover(&mut states, &cover[1], 1), "masonry blocks do not crush");
    }

    /// The recorded no-protocol-bump proof (urban-map doctrine decision 1): cover phases ride
    /// the wire as kind-AGNOSTIC bytes, so states on the new urban kinds round-trip through
    /// the same encoding untouched — appending kinds cannot shift a single wire byte.
    #[test]
    fn new_urban_kinds_ride_the_same_phase_bytes() {
        let cover = vec![
            object("tenement", StaticCoverKind::CityBuilding, [0.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
            object("yard_wall", StaticCoverKind::StoneWall, [30.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
        ];
        let mut states = cover_states_for(&cover);
        damage_cover(&mut states, &cover, 0, u32::MAX);
        damage_cover(&mut states, &cover, 1, u32::MAX);
        let bytes: Vec<u8> = states.iter().map(|state| state.phase.to_wire()).collect();
        assert_eq!(bytes, vec![1, 2], "Rubble/Gone use the same bytes every kind uses");
        let decoded: Vec<CoverPhase> =
            bytes.iter().map(|&byte| CoverPhase::from_wire(byte)).collect();
        assert_eq!(decoded, vec![CoverPhase::Rubble, CoverPhase::Gone]);
    }

    #[test]
    fn replicated_phase_bytes_resolve_through_the_authoritative_live_cover_rule() {
        let cover = vec![
            object("whole", StaticCoverKind::CityBuilding, [0.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
            object("rubble", StaticCoverKind::CityBuilding, [30.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
            object("gone", StaticCoverKind::StoneWall, [60.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
        ];
        let states = [
            CoverState { health: 1500, phase: CoverPhase::Intact },
            CoverState { health: 0, phase: CoverPhase::Rubble },
            CoverState { health: 0, phase: CoverPhase::Gone },
        ];

        let from_bytes = sight_cover_for_phase_bytes(&cover, &[0, 1, 2]);
        assert_eq!(
            from_bytes,
            live_cover_for_sight_and_shells(&cover, &states).into_owned(),
            "client phase bytes must use the sim's exact Intact/Rubble/Gone geometry"
        );
        assert_eq!(
            from_bytes.iter().map(|object| object.id.as_str()).collect::<Vec<_>>(),
            ["whole", "rubble"]
        );
        assert!(from_bytes[1].half_extents_m[1] < cover[1].half_extents_m[1]);
    }

    /// Born-ruins (urban-map PR-07): a "ruin" id starts at zero health in its collapsed
    /// phase, the sim's lazy state init picks that up, and the live-blocking slice serves
    /// the mound (or the clear door) from tick zero — no shell ever fired.
    #[test]
    fn born_ruins_start_collapsed_in_the_sim_and_on_the_wire() {
        let cover = vec![
            object("tenement_a", StaticCoverKind::CityBuilding, [0.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
            object(
                "tenement_ruin",
                StaticCoverKind::CityBuilding,
                [30.0, 5.5, 0.0],
                [9.0, 5.5, 5.0],
            ),
            object("wall_ruin", StaticCoverKind::StoneWall, [60.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
        ];
        let states = initial_cover_states(&cover);
        assert_eq!(states[0].phase, CoverPhase::Intact);
        assert_eq!((states[1].phase, states[1].health), (CoverPhase::Rubble, 0));
        assert_eq!((states[2].phase, states[2].health), (CoverPhase::Gone, 0));

        // The sim's lazy init reads the same birth rule.
        let mut state = crate::SimulationState::new();
        state.refresh_spotting(None, &cover);
        assert_eq!(state.cover_states()[1].phase, CoverPhase::Rubble);

        // And the wire bytes carry the ruin to every client and late joiner.
        let bytes: Vec<u8> = states.iter().map(|s| s.phase.to_wire()).collect();
        assert_eq!(bytes, vec![0, 1, 2]);

        let live = live_cover_for_sight_and_shells(&cover, &states);
        assert_eq!(live.len(), 2, "the ruined wall is a clear door from tick zero");
        assert!(live[1].half_extents_m[1] < 5.5 * 0.5, "the born mound is already low");
    }

    /// The split is inert everywhere except rubble. Intact boxes and cleared ground mean exactly
    /// one thing to everybody, so the two resolutions must agree on them — this is what says the
    /// refactor changed nothing on a battlefield that has not been knocked down yet.
    #[test]
    fn movement_and_sight_agree_on_everything_that_is_not_rubble() {
        let cover = vec![
            object("whole", StaticCoverKind::CityBuilding, [0.0, 5.5, 0.0], [9.0, 5.5, 5.0]),
            object("swept", StaticCoverKind::StoneWall, [30.0, 1.1, 0.0], [0.4, 1.1, 7.0]),
        ];
        let mut states = cover_states_for(&cover);
        assert_eq!(
            live_cover_for_movement(&cover, &states),
            live_cover_for_sight_and_shells(&cover, &states),
            "an untouched battlefield resolves once"
        );
        // A StoneWall breaches clean to Gone: absent for both, still no disagreement.
        damage_cover(&mut states, &cover, 1, u32::MAX);
        assert_eq!(
            live_cover_for_movement(&cover, &states),
            live_cover_for_sight_and_shells(&cover, &states),
            "cleared ground blocks nothing, for anybody"
        );
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
