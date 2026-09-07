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
    /// Which way it went down (Z8): `terrain::fall_heading_byte` of the crusher's heading or
    /// the shell's flight, written the tick the object leaves `Intact`. Only a felled TREE
    /// reads it (the trunk lies along it); masonry drops where it stands. `serde(default)`
    /// keeps older fixtures loading with every trunk at +X.
    #[serde(default)]
    pub fall: u8,
    /// Z9: the wall segments' states, two bits each (`terrain::segment_state`), replicated —
    /// all whole on a fresh building, all rubble once the box has come down.
    #[serde(default)]
    pub segments: terrain::SegmentStates,
    /// Z9: each segment's remaining budget (`WallMaterial::segment_health`), server-only.
    #[serde(default)]
    pub segment_health: [u16; terrain::SEGMENT_SLOTS],
}

impl CoverState {
    /// A fresh, whole object, healthed from its kind (indestructible kinds get `u32::MAX`).
    pub fn fresh(object: &StaticCoverObject) -> Self {
        let segment = terrain::wall_material(object).map_or(0, |m| m.segment_health() as u16);
        Self {
            health: object.kind.max_health().unwrap_or(u32::MAX),
            phase: CoverPhase::Intact,
            fall: 0,
            segments: [0; terrain::SEGMENT_BYTES],
            segment_health: [segment; terrain::SEGMENT_SLOTS],
        }
    }

    /// A state that has come down, whatever its phase byte: no health, every segment rubble.
    fn fallen(phase: CoverPhase) -> Self {
        Self {
            health: 0,
            phase,
            fall: 0,
            segments: terrain::SEGMENTS_ALL_RUBBLE,
            segment_health: [0; terrain::SEGMENT_SLOTS],
        }
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
            byte => CoverState::fallen(CoverPhase::from_wire(byte)),
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
    let opened = purpose == CoverPurpose::SightAndShells
        && states.iter().any(|state| terrain::segments_opened(&state.segments));
    if !opened && states.iter().all(|state| state.phase == CoverPhase::Intact) {
        return std::borrow::Cow::Borrowed(cover);
    }
    let mut live = Vec::with_capacity(cover.len());
    for (index, object) in cover.iter().enumerate() {
        match states.get(index).map(|state| state.phase).unwrap_or_default() {
            // Z9: a standing building with an OPENING is a hollow of slabs to the eye and the
            // shell — the wall that is there blocks, the segment that is open does not. The
            // hull never enters (§13.4), so movement keeps the whole box.
            CoverPhase::Intact
                if purpose == CoverPurpose::SightAndShells
                    && states.get(index).is_some_and(|s| terrain::segments_opened(&s.segments)) =>
            {
                live.extend(terrain::opened_building_boxes(object, &states[index].segments));
            }
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

/// A per-battle memo of the live-cover resolution, so the authoritative tick stops re-cloning the
/// whole cover slice — two `String`s per object, ~150 objects on a city map — every single frame
/// once anything is broken. It rebuilds ONLY when a cover phase actually changes (rare) and borrows
/// its materialised slices on every tick in between. This is the server-side twin of the client's
/// `LiveCoverCache`; the honest common case (nothing damaged yet) still borrows the authored slice
/// inside [`live_cover_for`], so an untouched battle allocates nothing here either.
#[derive(Debug, Default, Clone)]
pub struct CoverCache {
    phases: Vec<CoverPhase>,
    segments: Vec<terrain::SegmentStates>,
    /// Z13: the landed turrets the sight list carries as low solids.
    turrets: Vec<[f32; 3]>,
    sight: Vec<StaticCoverObject>,
    movement: Option<Vec<StaticCoverObject>>,
    rubble: Vec<RubbleMound>,
}

/// The cache is DERIVED state, not authoritative: two [`crate::SimulationState`]s with equal
/// `cover_states` are equal whether or not either has materialised its cache. So it must not sway
/// `SimulationState`'s derived `PartialEq`, which the replay/determinism tests lean on — always-equal
/// keeps the memo invisible to equality while `cover_states`, the real state, is compared as before.
impl PartialEq for CoverCache {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl CoverCache {
    /// Rebuild the memo IFF the cover phases (or, Z9, the wall segments) changed since it was
    /// last built; otherwise leave the materialised slices untouched. The guard is a compare
    /// of a few bytes per object, so the steady state (no cover changed this tick) is a walk
    /// over ~1 KB instead of ~150 heap clones. `movement` is materialised only when a mound
    /// exists, exactly like [`LiveCover`].
    /// With the landed turrets (Z13): each rest is a low solid in the SIGHT
    /// list — the shell stops in it, the eye stops at it — and never in the movement list
    /// (a hull crosses it; X4's model tilts it). Rebuilds when a rest appears or moves.
    pub fn refresh_with_turrets(
        &mut self,
        cover: &[StaticCoverObject],
        states: &[CoverState],
        turret_rests: &[[f32; 3]],
    ) {
        let unchanged = self.phases.len() == states.len()
            && self.phases.iter().zip(states).all(|(phase, state)| *phase == state.phase)
            && self.segments.iter().zip(states).all(|(seg, state)| *seg == state.segments)
            && self.turrets == turret_rests;
        if unchanged {
            return;
        }
        self.sight = sight_cover_with_turrets(cover, states, turret_rests);
        self.movement = (states.iter().any(|state| state.phase == CoverPhase::Rubble)
            || !turret_rests.is_empty())
        .then(|| live_cover_for_movement(cover, states).into_owned());
        self.rubble = rubble_mounds(cover, states);
        self.phases = states.iter().map(|state| state.phase).collect();
        self.segments = states.iter().map(|state| state.segments).collect();
        self.turrets = turret_rests.to_vec();
    }

    /// What stops a shell and hides a hull.
    pub fn sight(&self) -> &[StaticCoverObject] {
        &self.sight
    }

    /// What stops a HULL — rubble is climbable, so it parts from `sight` once a building falls.
    pub fn movement(&self) -> &[StaticCoverObject] {
        self.movement.as_deref().unwrap_or(&self.sight)
    }

    /// What a hull STANDS ON that the heightmap does not know: collapsed buildings, as debris.
    pub fn rubble(&self) -> &[RubbleMound] {
        &self.rubble
    }
}

fn states_from_phase_bytes(phase_bytes: &[u8]) -> Vec<CoverState> {
    states_from_wire(phase_bytes, &[])
}

/// The states the wire's bytes describe: a phase byte per object and (Z9) `SEGMENT_BYTES` of
/// packed segment states per object — or none at all, which reads as every segment whole.
fn states_from_wire(phase_bytes: &[u8], segment_bytes: &[u8]) -> Vec<CoverState> {
    let complete = segment_bytes.len() == phase_bytes.len() * terrain::SEGMENT_BYTES;
    phase_bytes
        .iter()
        .enumerate()
        .map(|(index, &byte)| {
            let mut state = CoverState::fallen(CoverPhase::from_wire(byte));
            state.segments = if complete {
                let at = index * terrain::SEGMENT_BYTES;
                segment_bytes[at..at + terrain::SEGMENT_BYTES].try_into().expect("sized above")
            } else {
                [0; terrain::SEGMENT_BYTES]
            };
            state
        })
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

/// [`sight_cover_for_phase_bytes`] with the wall segments (Z9) and the landed turrets (Z13):
/// a standing building with an opening resolves to its hollow of slabs and every landed
/// turret to its low solid, exactly as the authority resolves them.
pub fn sight_cover_for_wire(
    cover: &[StaticCoverObject],
    phase_bytes: &[u8],
    segment_bytes: &[u8],
    turret_rests: &[[f32; 3]],
) -> Vec<StaticCoverObject> {
    sight_cover_with_turrets(cover, &states_from_wire(phase_bytes, segment_bytes), turret_rests)
}

/// The sight list with the landed turrets appended (Z13).
fn sight_cover_with_turrets(
    cover: &[StaticCoverObject],
    states: &[CoverState],
    turret_rests: &[[f32; 3]],
) -> Vec<StaticCoverObject> {
    let mut sight = live_cover_for_sight_and_shells(cover, states).into_owned();
    sight
        .extend(turret_rests.iter().enumerate().map(|(index, &rest)| turret_rest_box(index, rest)));
    sight
}

/// Z13: a landed turret as the low solid the shell and the eye meet — steel on the ground,
/// `game_core::TURRET_REST_HALF_M` about its rest, never damaged (`cover_index_at` walks the
/// authored cover and never finds it).
pub fn turret_rest_box(index: usize, rest: [f32; 3]) -> StaticCoverObject {
    StaticCoverObject {
        id: format!("turret#{index}"),
        name: "a landed turret".to_string(),
        kind: terrain::StaticCoverKind::Wreck,
        center: rest,
        half_extents_m: game_core::TURRET_REST_HALF_M,
    }
}

/// Where every blown-off turret on the field has come to rest (Z13).
pub fn turret_rests_of(tanks: &[crate::TankState]) -> Vec<[f32; 3]> {
    tanks.iter().filter_map(|tank| tank.turret_rest).collect()
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
/// untouched. On reaching zero health the object collapses: a building to rubble, foliage to gone
/// — along `heading_rad`, the shell's flight, which is the way a felled tree lies (Z8).
/// Deterministic; no RNG.
pub fn damage_cover(
    states: &mut [CoverState],
    cover: &[StaticCoverObject],
    index: usize,
    hp: u32,
    heading_rad: f32,
) {
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
        state.fall = terrain::fall_heading_byte(heading_rad);
        // Z9: the box came down — every segment with it.
        state.segments = terrain::SEGMENTS_ALL_RUBBLE;
        state.segment_health = [0; terrain::SEGMENT_SLOTS];
    }
}

/// A high-explosive charge this large is what stone yields to (§13.4 „kamień tylko od
/// bezpośredniego dużego HE"): the 122 mm and up, not a 76 mm.
pub const LARGE_HE_FILLER_KG: f32 = 1.5;

/// What one absorbed shell takes off a wall segment of `material` (Z9, §13.4 „drewno pada od
/// taranu, cegła od kilku HE, kamień tylko od dużego HE"): timber takes every round in full;
/// brick takes HE in full and a kinetic round as a chip; stone yields only to a LARGE
/// high-explosive charge and to nothing else. `cover_hp` is the shell's cover damage
/// (`cover_damage_hp`).
pub fn segment_damage(
    material: terrain::WallMaterial,
    shell_type: game_core::ShellType,
    filler_kg: f32,
    cover_hp: u32,
) -> u32 {
    let high_explosive = shell_type == game_core::ShellType::HighExplosive;
    match material {
        terrain::WallMaterial::Timber => cover_hp,
        terrain::WallMaterial::Brick => {
            if high_explosive {
                cover_hp
            } else {
                cover_hp / 4
            }
        }
        terrain::WallMaterial::Stone => {
            if high_explosive && filler_kg >= LARGE_HE_FILLER_KG {
                cover_hp
            } else {
                0
            }
        }
    }
}

/// Which face of a box a point on its surface sits on (as `CoverScar.face` counts: 0 +X, 1 -X,
/// 2 +Z, 3 -Z, 4 the roof), and the point's normalized `u` across that face's run.
pub fn struck_face(object: &StaticCoverObject, position: [f32; 3]) -> (u8, f32) {
    let normalized: Vec<f32> = (0..3)
        .map(|axis| (position[axis] - object.center[axis]) / object.half_extents_m[axis].max(0.05))
        .collect();
    let ax = normalized[0].abs();
    let ay = normalized[1].abs();
    let az = normalized[2].abs();
    if ay >= ax && ay >= az {
        (4, normalized[0])
    } else if ax >= az {
        (if normalized[0] >= 0.0 { 0 } else { 1 }, normalized[2])
    } else {
        (if normalized[2] >= 0.0 { 2 } else { 3 }, normalized[0])
    }
}

/// Z9: one absorbed shell strikes the wall SEGMENT under it. The segment's budget is the
/// material's; what the round takes off it is `segment_damage`; its state steps whole →
/// damaged → ruin (an opening) → rubble (a lip) at thirds of the budget and never back. Returns
/// the (facade, segment) struck, `None` for the roof, a felled box or a kind without walls.
pub fn strike_segment(
    states: &mut [CoverState],
    cover: &[StaticCoverObject],
    index: usize,
    position: [f32; 3],
    shell_type: game_core::ShellType,
    filler_kg: f32,
    cover_hp: u32,
) -> Option<(usize, usize)> {
    let (Some(state), Some(object)) = (states.get_mut(index), cover.get(index)) else {
        return None;
    };
    if state.phase != CoverPhase::Intact {
        return None;
    }
    let material = terrain::wall_material(object)?;
    let (face, u) = struck_face(object, position);
    if face > 3 {
        return None;
    }
    let facade = usize::from(face);
    let segment = terrain::segment_at(terrain::facade_run_m(object, facade), u);
    let damage = segment_damage(material, shell_type, filler_kg, cover_hp);
    if damage == 0 {
        return None;
    }
    let slot = facade * terrain::SEGMENTS_PER_FACADE + segment;
    let left = state.segment_health[slot].saturating_sub(damage.min(u32::from(u16::MAX)) as u16);
    state.segment_health[slot] = left;
    let full = material.segment_health() as f32;
    let next = if left == 0 {
        terrain::SEGMENT_RUBBLE
    } else if f32::from(left) <= full / 3.0 {
        terrain::SEGMENT_RUIN
    } else if f32::from(left) <= full * 2.0 / 3.0 {
        terrain::SEGMENT_DAMAGED
    } else {
        terrain::SEGMENT_WHOLE
    };
    if next > terrain::segment_state(&state.segments, facade, segment) {
        terrain::set_segment_state(&mut state.segments, facade, segment, next);
    }
    Some((facade, segment))
}

/// Flatten a crushable cover object under a hull that drove into it: it goes straight to Gone
/// (a hedgerow does not become rubble). Returns `true` if it crushed something this call.
/// `heading_rad` is the hull's heading — the way the flattened tree goes down (Z8).
pub fn crush_cover(
    states: &mut [CoverState],
    object: &StaticCoverObject,
    index: usize,
    heading_rad: f32,
) -> bool {
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
    state.fall = terrain::fall_heading_byte(heading_rad);
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
    let (face, _) = struck_face(object, impact.position.to_array());
    let (u_axis, v_axis) = match face {
        4 => (0usize, 2usize),
        0 | 1 => (2usize, 1usize),
        _ => (0usize, 1usize),
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
    use glam::Vec3;
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
        damage_cover(&mut states, &cover, 0, 10_000, 0.0);
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
        damage_cover(&mut states, &cover, 0, 10_000, 0.0);
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert!(
            live_cover_for_sight_and_shells(&cover, &states).is_empty(),
            "gone foliage blocks nothing"
        );
    }

    /// Rail embankments and crags never change; a wreck is steel (Inny Poziom Z3): shellfire
    /// brings the hulk down to a hull-line mound that still blocks a shell below its crest.
    #[test]
    fn rail_and_crag_cover_are_indestructible_and_a_wreck_is_steel() {
        let cover = vec![
            object("rail", StaticCoverKind::RailCover, [0.0, 1.0, 0.0], [3.0, 1.0, 1.0]),
            object("hulk", StaticCoverKind::Wreck, [9.0, 1.35, 0.0], [2.0, 1.35, 3.0]),
            object("crag", StaticCoverKind::Crag, [20.0, 2.0, 0.0], [2.0, 2.0, 2.0]),
        ];
        let mut states = cover_states_for(&cover);
        damage_cover(&mut states, &cover, 0, u32::MAX, 0.0);
        damage_cover(&mut states, &cover, 2, u32::MAX, 0.0);
        assert_eq!(states[0].phase, CoverPhase::Intact, "a rail embankment is earth");
        assert_eq!(states[2].phase, CoverPhase::Intact, "a crag is the hill");

        let full = StaticCoverKind::Wreck.max_health().expect("a wreck has hit points");
        damage_cover(&mut states, &cover, 1, full / 2, 0.0);
        assert_eq!(states[1].phase, CoverPhase::Intact, "half its health: still a hulk");
        damage_cover(&mut states, &cover, 1, full / 2, 0.0);
        assert_eq!(states[1].phase, CoverPhase::Rubble, "shelled down to its hull line");
        let live = live_cover_for_sight_and_shells(&cover, &states);
        let mound = live.iter().find(|object| object.id == "hulk").expect("the mound still blocks");
        assert!(
            mound.half_extents_m[1] < 1.35 * 0.5 && mound.half_extents_m[1] > 0.3,
            "a hull-line mound, not the full hulk and not nothing: {}",
            mound.half_extents_m[1]
        );
    }

    #[test]
    fn a_hull_crushes_a_hedgerow_it_drives_into_but_not_a_building() {
        let cover = vec![
            object("hedge", StaticCoverKind::TreeLine, [0.0, 1.0, 0.0], [8.0, 1.0, 0.5]),
            object("barn", StaticCoverKind::FarmBuilding, [40.0, 2.0, 0.0], [5.0, 2.0, 4.0]),
        ];
        let mut states = cover_states_for(&cover);
        assert!(crush_cover(&mut states, &cover[0], 0, 0.0), "the hedge is crushed");
        assert!(!crush_cover(&mut states, &cover[1], 1, 0.0), "the barn is not crushable");
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert_eq!(states[1].phase, CoverPhase::Intact);
    }

    /// Z8: the fall has a direction. A hull crushing a hedge lays it down along its own
    /// heading; a shell felling a bole lays it down along its flight; the byte survives the
    /// wire's 256 steps to within a degree and a half. Masonry keeps the byte at rest — it
    /// drops where it stands and nothing reads it.
    #[test]
    fn a_felled_tree_remembers_which_way_it_fell() {
        let cover = vec![
            object("hedge", StaticCoverKind::TreeLine, [0.0, 1.0, 0.0], [8.0, 1.0, 0.5]),
            object("oak", StaticCoverKind::TreeTrunk, [20.0, 0.75, 0.0], [0.5, 0.75, 0.5]),
            object("barn", StaticCoverKind::FarmBuilding, [40.0, 2.0, 0.0], [5.0, 2.0, 4.0]),
        ];
        let mut states = cover_states_for(&cover);
        let north_east = 0.75f32;
        assert!(crush_cover(&mut states, &cover[0], 0, north_east), "the hedge is crushed");
        damage_cover(&mut states, &cover, 1, u32::MAX, -2.0);
        damage_cover(&mut states, &cover, 2, u32::MAX, 1.0);
        let degree = std::f32::consts::PI / 180.0;
        let hedge = terrain::fall_heading_rad(states[0].fall);
        assert!((hedge - north_east).abs() < 1.5 * degree, "the hedge fell {hedge}");
        let oak = terrain::fall_heading_rad(states[1].fall);
        let shot = (-2.0f32).rem_euclid(std::f32::consts::TAU);
        assert!((oak - shot).abs() < 1.5 * degree, "the oak fell {oak}, the shot flew {shot}");
        assert_eq!(states[2].phase, CoverPhase::Rubble);
        assert_eq!(states[2].fall, terrain::fall_heading_byte(1.0), "written once, read by no one");
        assert_eq!(cover_states_for(&cover)[0].fall, 0, "a standing tree has no fall");
    }

    /// Z9 (§13.4 „drewno pada od taranu, cegła od kilku HE, kamień tylko od dużego HE"): the
    /// wall's material decides. The same barn segment falls to a 57 mm HE in more rounds than
    /// to a 152 mm HE; a stone church segment ignores the 57 mm and every kinetic round and
    /// yields only to the large charge; the segment struck is the one under the shell and its
    /// neighbour stands whole; the box itself still stands (the segments are not its health).
    #[test]
    fn a_57_and_a_152_fell_the_same_barn_segment_in_different_counts_and_stone_only_to_a_large_he()
    {
        let cover = vec![
            object("barn", StaticCoverKind::FarmBuilding, [0.0, 2.0, 0.0], [6.0, 2.0, 4.0]),
            object(
                "ostrogorsk_church",
                StaticCoverKind::CityBuilding,
                [50.0, 8.0, 0.0],
                [8.0, 8.0, 6.0],
            ),
        ];
        // ZiS-2's O-271 and the ML-20's OF-540, by their charges; BR-271 the 57 mm shot.
        let he_57 = game_core::ShellSpec::high_explosive(57.0, 700.0, 30.0, 90, 1.5)
            .with_projectile(3.75, 0.22);
        let he_152 = game_core::ShellSpec::high_explosive(152.0, 655.0, 40.0, 910, 5.0)
            .with_projectile(43.6, 6.0);
        let ap_57 = game_core::ShellSpec::armor_piercing(57.0, 990.0, 112.0, 85)
            .with_projectile(3.15, 0.02);
        let rounds_to_fell = |shell: &game_core::ShellSpec, target: usize, face_x: f32| {
            let mut states = cover_states_for(&cover);
            let hp = crate::cover_damage_hp(shell);
            let hit = [face_x, cover[target].center[1] * 0.5, cover[target].center[2] - 3.0];
            for round in 1..=40 {
                strike_segment(
                    &mut states,
                    &cover,
                    target,
                    hit,
                    shell.shell_type,
                    shell.filler_kg,
                    hp,
                );
                assert_eq!(states[target].phase, CoverPhase::Intact, "the box stands");
                assert_eq!(
                    terrain::segment_state(&states[target].segments, 0, 1),
                    terrain::SEGMENT_WHOLE,
                    "the neighbouring segment stands whole"
                );
                if terrain::segment_state(&states[target].segments, 0, 0) == terrain::SEGMENT_RUBBLE
                {
                    return round;
                }
            }
            usize::MAX
        };
        let barn_57 = rounds_to_fell(&he_57, 0, 6.0);
        let barn_152 = rounds_to_fell(&he_152, 0, 6.0);
        assert!(barn_152 < barn_57, "the big charge fells it sooner: {barn_152} vs {barn_57}");
        assert!(barn_57 <= 4, "a barn segment is timber: {barn_57} rounds of 57 mm HE");
        assert_eq!(rounds_to_fell(&he_57, 1, 58.0), usize::MAX, "stone shrugs off a 57 mm HE");
        assert_eq!(rounds_to_fell(&ap_57, 1, 58.0), usize::MAX, "and every kinetic round");
        assert!(rounds_to_fell(&he_152, 1, 58.0) <= 3, "the large charge opens the church");
        // A wall's states step in thirds and only forward; the whole box's collapse fells them all.
        let mut states = cover_states_for(&cover);
        let hp = crate::cover_damage_hp(&ap_57);
        let hit = [6.0, 1.0, -3.0];
        let first = strike_segment(&mut states, &cover, 0, hit, ap_57.shell_type, 0.02, hp);
        assert_eq!(first, Some((0, 0)));
        assert!(terrain::segment_state(&states[0].segments, 0, 0) <= terrain::SEGMENT_DAMAGED);
        damage_cover(&mut states, &cover, 0, u32::MAX, 0.0);
        assert_eq!(states[0].segments, terrain::SEGMENTS_ALL_RUBBLE, "the box came down");
        assert_eq!(strike_segment(&mut states, &cover, 0, hit, ap_57.shell_type, 0.02, hp), None);
    }

    /// Z13: a landed turret is a low solid. The eye (and so the shell, which reads the same
    /// list) stops in it below its top and clears it above; the hull's movement list never
    /// carries it (a hull crosses it — X4's model tilts it); it comes and goes with the rests.
    #[test]
    fn the_eye_and_the_shell_stop_in_a_landed_turret_and_a_hull_does_not() {
        use crate::spotting::line_of_sight;
        let cover =
            vec![object("barn", StaticCoverKind::FarmBuilding, [0.0, 2.0, 40.0], [6.0, 2.0, 4.0])];
        let states = cover_states_for(&cover);
        let rest = [10.0, 0.45, 0.0];
        let mut cache = CoverCache::default();
        cache.refresh_with_turrets(&cover, &states, &[rest]);
        assert_eq!(cache.sight().len(), 2, "the barn and the landed turret");
        assert_eq!(cache.movement().len(), 1, "the hull's list carries only the barn");
        let low = |x: f32| Vec3::new(x, 0.6, 0.0);
        assert!(!line_of_sight(None, cache.sight(), low(20.0), low(0.0)), "the eye stops in it");
        assert!(line_of_sight(
            None,
            cache.sight(),
            Vec3::new(20.0, 1.5, 0.0),
            Vec3::new(0.0, 1.5, 0.0)
        ));
        assert!(
            line_of_sight(None, cache.movement(), low(20.0), low(0.0)),
            "movement never blocks on it"
        );
        cache.refresh_with_turrets(&cover, &states, &[]);
        assert_eq!(cache.sight().len(), 1, "no rest, no solid");
        assert_eq!(
            sight_cover_for_wire(&cover, &[0], &[], &[rest]).len(),
            2,
            "the client resolves the same solid from the replicated rest"
        );
    }

    /// Z9: an opening is honest. A brick house with one ruined street segment lets the eye
    /// (and so the shell) through that segment into the hollow, and the far wall still stops
    /// it; at the sill's height the eye is blocked; ruin the facing segment of the far wall
    /// too and the line sees clean through. Movement keeps the whole box: the hull never
    /// enters (§13.4). A damaged segment alone opens nothing.
    #[test]
    fn a_ruined_segment_opens_the_wall_and_the_far_wall_still_stops_the_eye() {
        use crate::spotting::line_of_sight;
        let cover =
            vec![object("house", StaticCoverKind::CityBuilding, [0.0, 4.0, 0.0], [5.0, 4.0, 4.0])];
        let mut states = cover_states_for(&cover);
        let eye = Vec3::new(20.0, 1.8, -2.0);
        let beyond = Vec3::new(-20.0, 1.8, -2.0);
        let inside = Vec3::new(0.0, 1.8, -2.0);
        let sees = |states: &[CoverState], from: Vec3, to: Vec3| {
            line_of_sight(None, &live_cover_for_sight_and_shells(&cover, states), from, to)
        };
        assert!(!sees(&states, eye, beyond));
        assert!(!sees(&states, eye, inside));
        terrain::set_segment_state(&mut states[0].segments, 0, 0, terrain::SEGMENT_DAMAGED);
        assert_eq!(
            live_cover_for_sight_and_shells(&cover, &states).len(),
            1,
            "damage opens nothing"
        );
        terrain::set_segment_state(&mut states[0].segments, 0, 0, terrain::SEGMENT_RUIN);
        assert!(live_cover_for_sight_and_shells(&cover, &states).len() > 1, "a hollow of slabs");
        assert!(sees(&states, eye, inside), "the eye reaches into the hollow through the opening");
        assert!(!sees(&states, eye, beyond), "the far wall still stops it");
        assert!(!sees(&states, Vec3::new(20.0, 0.3, -2.0), Vec3::new(0.0, 0.3, -2.0)), "the sill");
        assert!(
            !sees(&states, Vec3::new(20.0, 1.8, 2.0), Vec3::new(0.0, 1.8, 2.0)),
            "the whole one"
        );
        terrain::set_segment_state(&mut states[0].segments, 1, 0, terrain::SEGMENT_RUIN);
        assert!(sees(&states, eye, beyond), "through two openings the line sees clean through");
        assert_eq!(
            live_cover_for_movement(&cover, &states).as_ref(),
            &cover[..],
            "the hull never enters"
        );
        let mut cache = CoverCache::default();
        cache.refresh_with_turrets(&cover, &states, &[]);
        assert!(cache.sight().len() > 1, "the memo resolves the segments too");
        terrain::set_segment_state(&mut states[0].segments, 1, 0, terrain::SEGMENT_RUBBLE);
        cache.refresh_with_turrets(&cover, &states, &[]);
        assert_eq!(cache.sight(), live_cover_for_sight_and_shells(&cover, &states).as_ref());
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

        damage_cover(&mut states, &cover, 0, 1499, 0.0);
        assert_eq!(states[0].phase, CoverPhase::Intact, "1499 HP does not fell masonry");
        damage_cover(&mut states, &cover, 0, 1, 0.0);
        assert_eq!(states[0].phase, CoverPhase::Rubble, "the block collapses at 1500");

        damage_cover(&mut states, &cover, 1, 150, 0.0);
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
        assert!(crush_cover(&mut states, &cover[0], 0, 0.0), "the wall crushes under the hull");
        assert_eq!(states[0].phase, CoverPhase::Gone);
        assert!(!crush_cover(&mut states, &cover[1], 1, 0.0), "masonry blocks do not crush");
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
        damage_cover(&mut states, &cover, 0, u32::MAX, 0.0);
        damage_cover(&mut states, &cover, 1, u32::MAX, 0.0);
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
            CoverState::fresh(&cover[0]),
            CoverState::fallen(CoverPhase::Rubble),
            CoverState::fallen(CoverPhase::Gone),
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
        damage_cover(&mut states, &cover, 1, u32::MAX, 0.0);
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

    #[test]
    fn the_cover_cache_matches_a_fresh_resolution_and_invalidates_on_phase_change() {
        // One of each phase outcome: a building that rubbles, foliage that vanishes, an intact wall.
        let cover = vec![
            object("barn", StaticCoverKind::FarmBuilding, [0.0, 3.0, 0.0], [5.0, 3.0, 4.0]),
            object("hedge", StaticCoverKind::TreeLine, [20.0, 2.0, 0.0], [10.0, 2.0, 1.0]),
            object("wall", StaticCoverKind::StoneWall, [40.0, 1.0, 0.0], [4.0, 1.0, 0.5]),
        ];
        let states = cover_states_for(&cover);
        let mut cache = CoverCache::default();

        // Intact: the memo equals a fresh resolution, and with no rubble yet movement borrows sight.
        cache.refresh_with_turrets(&cover, &states, &[]);
        assert_eq!(cache.sight(), live_cover_for_sight_and_shells(&cover, &states).as_ref());
        assert_eq!(cache.movement(), live_cover_for_movement(&cover, &states).as_ref());
        assert_eq!(cache.rubble(), rubble_mounds(&cover, &states).as_slice());
        assert!(cache.rubble().is_empty(), "nothing is rubble yet");

        // Bring the barn down (Rubble) and clear the hedge (Gone), then refresh: the memo must
        // follow the new phases — a stale cache would keep blocking with cover the battle has lost.
        let mut changed = states.clone();
        damage_cover(&mut changed, &cover, 0, 10_000, 0.0);
        damage_cover(&mut changed, &cover, 1, 10_000, 0.0);
        assert_eq!(changed[0].phase, CoverPhase::Rubble);
        assert_eq!(changed[1].phase, CoverPhase::Gone);

        cache.refresh_with_turrets(&cover, &changed, &[]);
        assert_eq!(cache.sight(), live_cover_for_sight_and_shells(&cover, &changed).as_ref());
        assert_eq!(cache.movement(), live_cover_for_movement(&cover, &changed).as_ref());
        assert_eq!(cache.rubble(), rubble_mounds(&cover, &changed).as_slice());
        assert_eq!(cache.rubble().len(), 1, "the barn is now a mound");
        assert_ne!(cache.movement(), cache.sight(), "movement and sight part ways over rubble");

        // Refreshing again with the SAME phases keeps the same answer (the steady-state borrow path).
        cache.refresh_with_turrets(&cover, &changed, &[]);
        assert_eq!(cache.sight(), live_cover_for_sight_and_shells(&cover, &changed).as_ref());
    }
}
