use game_core::HitboxProfile;
use glam::{Vec2, Vec3};
use terrain::StaticCoverObject;

pub(crate) const TANK_COLLISION_RADIUS_M: f32 = 1.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankFootprint {
    pub half_width_m: f32,
    pub half_length_m: f32,
}

impl TankFootprint {
    pub fn from_hitbox(hitbox: HitboxProfile) -> Self {
        Self {
            half_width_m: hitbox.half_width_m.max(0.01),
            half_length_m: hitbox.half_length_m.max(0.01),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankObstacle {
    pub center: Vec3,
    pub yaw_rad: f32,
    pub footprint: TankFootprint,
}

impl TankObstacle {
    pub fn new(center: Vec3, yaw_rad: f32, footprint: TankFootprint) -> Self {
        Self { center, yaw_rad, footprint }
    }

    pub fn from_hitbox(center: Vec3, yaw_rad: f32, hitbox: HitboxProfile) -> Self {
        Self::new(center, yaw_rad, TankFootprint::from_hitbox(hitbox))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TankWorldObstacles<'a> {
    pub cover: &'a [StaticCoverObject],
    pub tank_footprint: TankFootprint,
    pub tanks: &'a [TankObstacle],
    /// The map's standing water, if any: wading drag and the riverbed traction cut read the
    /// depth of water over the terrain contact (see [`crate::water`]). `None` keeps the dry
    /// path bit-identical (replay-locked).
    pub water: Option<terrain::WaterBody>,
    /// Collapsed buildings, as GROUND (see [`terrain::RubbleMound`]). Empty on a battlefield
    /// nothing has knocked down yet, which keeps the untouched path bit-identical.
    pub rubble: &'a [terrain::RubbleMound],
    /// The map's ground rule (see [`terrain::GroundClassifier`]) — what the surface under the
    /// tracks IS. `None` is bare grass everywhere, which is exactly the model before material
    /// existed, so terrain-free modes and old fixtures stay bit-identical.
    pub ground: Option<&'a terrain::GroundClassifier>,
}

impl<'a> TankWorldObstacles<'a> {
    pub fn new(
        cover: &'a [StaticCoverObject],
        tank_footprint: TankFootprint,
        tanks: &'a [TankObstacle],
    ) -> Self {
        Self { cover, tank_footprint, tanks, water: None, rubble: &[], ground: None }
    }

    pub fn with_water(mut self, water: Option<terrain::WaterBody>) -> Self {
        self.water = water;
        self
    }

    pub fn with_rubble(mut self, rubble: &'a [terrain::RubbleMound]) -> Self {
        self.rubble = rubble;
        self
    }

    pub fn with_ground(mut self, ground: Option<&'a terrain::GroundClassifier>) -> Self {
        self.ground = ground;
        self
    }
}

pub fn default_tank_footprint() -> TankFootprint {
    TankFootprint { half_width_m: TANK_COLLISION_RADIUS_M, half_length_m: TANK_COLLISION_RADIUS_M }
}

/// Zero the world-axis velocity components that the resolver had to drop back to `previous`,
/// leaving the sliding axis intact. The axis-separated resolver only ever holds whole world axes,
/// so killing exactly those axes keeps the surviving velocity tangent to the obstacle (the hull
/// slides along a wall instead of either sticking or keeping a phantom into-wall component).
pub(crate) fn trim_velocity(
    previous: Vec3,
    attempted: Vec3,
    resolved: Vec3,
    velocity: Vec3,
) -> Vec3 {
    let mut trimmed = velocity;
    if axis_blocked(previous.x, attempted.x, resolved.x) {
        trimmed.x = 0.0;
    }
    if axis_blocked(previous.z, attempted.z, resolved.z) {
        trimmed.z = 0.0;
    }
    trimmed
}

/// True when a move along one world axis was attempted but the resolver pinned it back to the
/// previous value (i.e. that axis is blocked by an obstacle).
fn axis_blocked(previous: f32, attempted: f32, resolved: f32) -> bool {
    (attempted - previous).abs() > 1.0e-6 && (resolved - previous).abs() <= 1.0e-5
}

/// How two overlapping footprints are into each other: the axis of LEAST penetration (the
/// minimum translation vector of the same SAT), pointing from `a` toward `b`, and the depth along
/// it. Separating them means moving `a` back along `normal` and `b` forward along it.
///
/// This is what turns a collision from a veto into a physical event. A blocking test only ever
/// needed "do these touch"; an impulse needs to know along WHICH direction the contact acts,
/// and a positional correction needs to know by how much.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootprintContact {
    /// Unit contact normal in the XZ plane, from `a` toward `b`.
    pub normal: Vec2,
    /// Penetration depth along `normal`, always positive.
    pub depth_m: f32,
    /// WHICH pair of features is touching — the identity a cached impulse is filed under.
    pub feature: ContactFeature,
}

/// Which features of the two hulls are in contact, as a value that stays put while the same
/// corners stay pressed together and changes when they stop.
///
/// A contact impulse is only worth carrying from one tick to the next if next tick's contact is
/// the SAME contact. Warm starting leans entirely on that: the accumulated impulse is reused when
/// the pair, the face and the corner all match, and thrown away when they do not. Without an
/// identity the cache would cheerfully carry a flank impulse onto a nose-on contact and shove a
/// hull sideways for a reason that stopped existing a tick ago.
///
/// Everything here is expressed in the hulls' OWN frames, not the world's, so a pair grinding
/// slowly past each other keeps one identity through the whole slide instead of minting a new one
/// every time the pair rotates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ContactFeature {
    /// Which hull owns the face the contact resolves against: `false` = `a`, `true` = `b`.
    pub reference_is_b: bool,
    /// Which of that hull's two footprint axes the face lies on: `false` = its sides (the `right`
    /// axis), `true` = its ends (the `forward` axis).
    pub reference_is_end: bool,
    /// Which of the two faces on that axis: `true` for the one on the +axis side of the hull.
    pub reference_positive: bool,
    /// Which corner of the OTHER hull is pressed deepest into that face, indexed in that hull's
    /// own frame (bit 0 set = +right side, bit 1 set = +forward end).
    pub incident_corner: u8,
}

/// Overlap depth threshold: shallower than this and the two footprints are touching, not
/// interpenetrating. Unchanged from the original test, so no blocking verdict anywhere moves.
const OVERLAP_EPSILON_M: f32 = 1.0e-5;

/// XZ-plane separating-axis test between two oriented hull footprints, reporting the contact when
/// they overlap. [`obstacles_overlap`] is this test's yes/no answer and stays bit-identical to it:
/// the separation threshold is the same, so no blocking verdict anywhere moves.
pub(crate) fn obstacles_contact(a: &TankObstacle, b: &TankObstacle) -> Option<FootprintContact> {
    footprint_contact_within(a, b, 0.0, None).filter(|contact| contact.depth_m > OVERLAP_EPSILON_M)
}

/// The same separating-axis test, but reporting a contact for hulls that are merely CLOSE as well
/// as for hulls that overlap — `depth_m` is signed, positive into each other and negative apart.
///
/// This is what a speculative contact needs. A constraint that only exists once two hulls are
/// inside each other is a constraint that arrives too late: the solver's only remaining move is to
/// stop them dead wherever they happen to be, which is how a standoff gap gets built (the pair
/// parks a detection margin apart and the metal never touches). Told how far apart they still are,
/// the same solver can allow exactly the closing that shuts the gap this tick and no more — the
/// hulls end up touching, which is where they belong.
///
/// The axis picked is the same one either way, and that falls out of the arithmetic rather than
/// needing a second rule: overlapping, the minimum depth IS the minimum translation out; apart,
/// the minimum (most negative) depth IS the axis that separates them best.
pub(crate) fn footprint_contact_within(
    a: &TankObstacle,
    b: &TankObstacle,
    margin_m: f32,
    incumbent: Option<ContactFeature>,
) -> Option<FootprintContact> {
    let center_a = Vec2::new(a.center.x, a.center.z);
    let center_b = Vec2::new(b.center.x, b.center.z);
    let [right_a, forward_a] = footprint_axes(a.yaw_rad);
    let [right_b, forward_b] = footprint_axes(b.yaw_rad);
    let delta = center_b - center_a;
    let axes = [[right_a, forward_a], [right_b, forward_b]];
    let mut shallowest: Option<(usize, Vec2, f32)> = None;
    for (index, axis) in [right_a, forward_a, right_b, forward_b].into_iter().enumerate() {
        let radius_a = a.footprint.half_width_m * axis.dot(right_a).abs()
            + a.footprint.half_length_m * axis.dot(forward_a).abs();
        let radius_b = b.footprint.half_width_m * axis.dot(right_b).abs()
            + b.footprint.half_length_m * axis.dot(forward_b).abs();
        let separation = delta.dot(axis);
        let depth_m = radius_a + radius_b - separation.abs();
        // Further apart along this axis than anyone cares about: no contact, and no point
        // projecting the rest.
        if depth_m < -margin_m {
            return None;
        }
        if shallowest.is_none_or(|(_, _, shallowest)| depth_m < shallowest) {
            // Orient the axis from `a` toward `b`, so the normal always says which way `b` lies.
            let normal = if separation < 0.0 { -axis } else { axis };
            shallowest = Some((index, normal, depth_m));
        }
    }
    let (index, normal, depth_m) = shallowest?;
    // Hold the axis that was already winning while the two ways out are within a hair of each
    // other. Two hulls at a small relative yaw have two nearly equal ways out — across one flank
    // or across the other — and which is shallower CROSSES OVER as they slide past each other
    // (measured in P1.1: up to twice across a 4 m grind). Letting the crossover through costs a
    // cache miss in the middle of exactly the sustained contact a cache exists to smooth, and buys
    // nothing: the two axes differ by the relative yaw, so the answer is the same contact either
    // way. Preferring the incumbent inside the band keeps one answer.
    let (index, normal, depth_m) = match incumbent
        .map(axis_index_of)
        .filter(|&preferred| preferred != index)
    {
        Some(preferred) if depth_along(a, b, &axes, preferred).0 <= depth_m + AXIS_HYSTERESIS_M => {
            let (depth, normal) = depth_along(a, b, &axes, preferred);
            (preferred, normal, depth)
        }
        _ => (index, normal, depth_m),
    };
    Some(FootprintContact { normal, depth_m, feature: feature_at(a, b, &axes, index, normal) })
}

/// How far apart the pair is along one named axis, with the normal oriented `a` -> `b`.
fn depth_along(
    a: &TankObstacle,
    b: &TankObstacle,
    axes: &[[Vec2; 2]; 2],
    index: usize,
) -> (f32, Vec2) {
    let axis = axes[index / 2][index % 2];
    let [right_a, forward_a] = axes[0];
    let [right_b, forward_b] = axes[1];
    let radius_a = a.footprint.half_width_m * axis.dot(right_a).abs()
        + a.footprint.half_length_m * axis.dot(forward_a).abs();
    let radius_b = b.footprint.half_width_m * axis.dot(right_b).abs()
        + b.footprint.half_length_m * axis.dot(forward_b).abs();
    let delta = Vec2::new(b.center.x - a.center.x, b.center.z - a.center.z);
    let separation = delta.dot(axis);
    let depth = radius_a + radius_b - separation.abs();
    (depth, if separation < 0.0 { -axis } else { axis })
}

/// Which of the four projected axes a remembered feature was resolved against.
fn axis_index_of(feature: ContactFeature) -> usize {
    usize::from(feature.reference_is_b) * 2 + usize::from(feature.reference_is_end)
}

/// How much shallower a rival axis has to be before the contact changes hands. Two hulls at a
/// small relative yaw sit inside this for the whole of a grind, which is the point.
const AXIS_HYSTERESIS_M: f32 = 0.05;

/// Name the features the winning separating axis picked out: the face it belongs to, and the
/// corner of the other hull pressed into it.
fn feature_at(
    a: &TankObstacle,
    b: &TankObstacle,
    axes: &[[Vec2; 2]; 2],
    axis_index: usize,
    normal: Vec2,
) -> ContactFeature {
    let reference_is_b = axis_index >= 2;
    let reference_is_end = axis_index % 2 == 1;
    let axis = axes[axis_index / 2][axis_index % 2];
    // Outward from the REFERENCE hull toward the other one. `normal` runs a -> b, so it already
    // points outward for a face on `a` and has to be flipped for one on `b`.
    let outward = if reference_is_b { -normal } else { normal };
    let (incident, incident_axes) = if reference_is_b { (a, axes[0]) } else { (b, axes[1]) };
    ContactFeature {
        reference_is_b,
        reference_is_end,
        reference_positive: outward.dot(axis) > 0.0,
        incident_corner: deepest_corner(incident, incident_axes, outward),
    }
}

/// Two corners level within this are the same corner as far as identity is concerned.
///
/// A tenth of a millimetre is far below any difference the geometry means and far above the noise
/// f32 puts into a rotated dot product. Without it a genuinely FLAT contact — two hulls parked
/// side by side, their flanks parallel — would have its two corners tied to within rounding, and
/// the identity would flicker between them tick after tick. A flickering identity is worse than no
/// identity: the cache would miss every tick AND pay to look.
const CORNER_TIE_M: f32 = 1.0e-4;

/// The corner of `hull` reaching furthest against `outward` — the support point pressed into the
/// reference face. Ties keep the LOWEST index, so a flat contact names one corner and goes on
/// naming it.
fn deepest_corner(hull: &TankObstacle, [right, forward]: [Vec2; 2], outward: Vec2) -> u8 {
    let mut best = 0;
    let mut best_depth = f32::INFINITY;
    for index in 0..4u8 {
        let sign = |bit: u8| if index & bit == bit { 1.0 } else { -1.0 };
        let offset = right * (sign(1) * hull.footprint.half_width_m)
            + forward * (sign(2) * hull.footprint.half_length_m);
        let depth = offset.dot(outward);
        if depth < best_depth - CORNER_TIE_M {
            best_depth = depth;
            best = index;
        }
    }
    best
}

/// XZ-plane separating-axis overlap between two oriented hull footprints.
pub(crate) fn obstacles_overlap(a: &TankObstacle, b: &TankObstacle) -> bool {
    obstacles_contact(a, b).is_some()
}

fn footprint_axes(yaw_rad: f32) -> [Vec2; 2] {
    let forward = Vec2::new(yaw_rad.sin(), yaw_rad.cos());
    let right = Vec2::new(forward.y, -forward.x);
    [right, forward]
}

#[cfg(test)]
mod contact_tests {
    use super::*;

    fn hull(x: f32, z: f32, yaw_rad: f32) -> TankObstacle {
        TankObstacle::new(
            Vec3::new(x, 0.0, z),
            yaw_rad,
            TankFootprint { half_width_m: 1.75, half_length_m: 3.2 },
        )
    }

    /// The contact must be the MINIMUM translation: pushing the pair apart by exactly `depth_m`
    /// along `normal` has to separate them, and no less would have. This is the property the
    /// positional correction leans on, so it is the property worth locking rather than any
    /// particular number.
    #[test]
    fn the_reported_contact_is_the_shallowest_way_out() {
        let mut state = 0x0c0f_feeeu32;
        let xorshift = |state: &mut u32| {
            *state ^= *state << 13;
            *state ^= *state >> 17;
            *state ^= *state << 5;
            (*state % 10_000) as f32 / 10_000.0
        };
        let mut seen = 0;
        for _ in 0..600 {
            let a = hull(0.0, 0.0, xorshift(&mut state) * std::f32::consts::TAU);
            let b = hull(
                (xorshift(&mut state) - 0.5) * 12.0,
                (xorshift(&mut state) - 0.5) * 12.0,
                xorshift(&mut state) * std::f32::consts::TAU,
            );
            let Some(contact) = obstacles_contact(&a, &b) else {
                assert!(!obstacles_overlap(&a, &b), "no contact must mean no overlap");
                continue;
            };
            seen += 1;
            assert!(contact.depth_m > 0.0);
            assert!((contact.normal.length() - 1.0).abs() < 1.0e-4, "the normal must be unit");

            // Separating by the full depth (plus a hair for the 1e-5 threshold) must clear it.
            let push = contact.normal * (contact.depth_m + 1.0e-3);
            let moved = TankObstacle { center: b.center + Vec3::new(push.x, 0.0, push.y), ..b };
            assert!(!obstacles_overlap(&a, &moved), "the MTV must actually separate the pair");

            // ...and separating by appreciably LESS must not: it is the minimum, not merely a way.
            let short = contact.normal * (contact.depth_m * 0.5);
            let barely = TankObstacle { center: b.center + Vec3::new(short.x, 0.0, short.y), ..b };
            assert!(obstacles_overlap(&a, &barely), "half the depth cannot already be clear");
        }
        assert!(seen > 30, "the sampling must actually produce contacts, saw {seen}");
    }

    /// The identity names a FACE and a CORNER, and both are read in the hulls' own frames.
    ///
    /// Geometry chosen so nothing is tied and the answer can be reasoned out by hand: `b` sits to
    /// `a`'s right, canted 0.2 rad. The shallowest way out is across `a`'s flank (1.10 m against
    /// 1.16 m for `b`'s flank and six metres for either pair of ends), so the reference face is
    /// `a`'s +right side; and the corner of `b` reaching furthest back into it is its rear-left,
    /// which is corner 0 in `b`'s own frame.
    #[test]
    fn the_contact_names_the_face_and_the_corner_that_meet() {
        let a = hull(0.0, 0.0, 0.0);
        let b = hull(3.0, 0.0, 0.2);
        let contact = obstacles_contact(&a, &b).expect("these overlap");

        assert!(!contact.feature.reference_is_b, "the reference face belongs to `a`");
        assert!(!contact.feature.reference_is_end, "...and it is a flank, not an end");
        assert!(contact.feature.reference_positive, "...the one `b` is standing on");
        assert_eq!(contact.feature.incident_corner, 0, "b's rear-left corner is deepest in");
    }

    /// How often the identity turns over during a grind — and the answer is a finding, not a
    /// formality.
    ///
    /// Two hulls at a small relative yaw have two nearly equal ways out: across `a`'s flank and
    /// across `b`'s. Which one is shallower CROSSES OVER as they slide past each other, so the
    /// reference face changes hands mid-pass even though the same two pieces of steel are touching
    /// the whole time. The normal only swings by the relative yaw when it happens (11° here, not
    /// the 90° an axis flip could cost), so this is not the chatter the separation pass was
    /// measured for — but it IS a cache miss, and it lands in the middle of exactly the sustained
    /// contact warm starting exists to smooth.
    ///
    /// Recorded here so the number is on the table before the solver leans on it: the fix is
    /// temporal, a bias toward the incumbent axis, and it needs the previous tick — which
    /// `obstacles_contact` has no business knowing about. It belongs to the pass that owns the
    /// cache.
    #[test]
    fn the_identity_turns_over_only_where_the_two_ways_out_cross() {
        let a = hull(0.0, 0.0, 0.0);
        let mut changes = 0;
        let mut previous: Option<ContactFeature> = None;
        for step in 0..=80 {
            let z = -2.0 + step as f32 * 0.05;
            let contact = obstacles_contact(&a, &hull(3.0, z, 0.2)).expect("overlapping");
            if previous.is_some_and(|was| was != contact.feature) {
                changes += 1;
            }
            previous = Some(contact.feature);
        }
        assert!(
            changes <= 2,
            "the identity turned over {changes} times across an 4 m slide — that is per-step \
             churn, not a crossover, and no cache can survive it"
        );
    }

    /// ...and it must NOT survive a change of face. A hull that was leaning on a flank and is now
    /// nose-on is a different contact, and reusing the old impulse would shove it sideways for a
    /// reason that stopped existing.
    #[test]
    fn the_feature_changes_when_the_contact_moves_to_another_face() {
        let a = hull(0.0, 0.0, 0.0);
        let on_the_flank = obstacles_contact(&a, &hull(3.0, 0.0, 0.2)).expect("overlapping");
        let nose_on = obstacles_contact(&a, &hull(0.0, 6.0, 0.2)).expect("overlapping");

        assert!(!on_the_flank.feature.reference_is_end);
        assert!(nose_on.feature.reference_is_end, "meeting end-on must name an end face");
        assert_ne!(on_the_flank.feature, nose_on.feature);
    }

    /// A FLAT contact — two hulls parked side by side, flanks parallel — has both corners of the
    /// touching face at the same depth, so "the deepest one" is a coin toss that f32 noise can
    /// call differently every tick. `CORNER_TIE_M` settles it: inside the tie width the lower
    /// index always wins, so the identity is a fact about the geometry rather than about the last
    /// rounding error.
    ///
    /// What this does NOT claim is hysteresis. Cant the pair by a milliradian and the corners are
    /// genuinely 6 mm apart; naming the deeper one is then correct, not a flicker. The real answer
    /// for a face pressed flat against a face is TWO contact points rather than a choice between
    /// two corners, and that manifold belongs with the solver that would use it.
    #[test]
    fn a_flat_contact_settles_its_tie_the_same_way_every_time() {
        let a = hull(0.0, 0.0, 0.0);
        let parallel = obstacles_contact(&a, &hull(3.0, 0.0, 0.0)).expect("overlapping").feature;
        // ±1e-5 rad moves the corners by ~64 µm, inside the 100 µm tie.
        for step in -10..=10 {
            let yaw = step as f32 * 1.0e-6;
            let contact = obstacles_contact(&a, &hull(3.0, 0.0, yaw)).expect("overlapping");
            assert_eq!(
                contact.feature, parallel,
                "the tie broke differently at a yaw of {yaw} rad, inside the tie width"
            );
        }
    }

    /// The solver is order-independent by construction, so the identity has to be too: swapping
    /// the pair must name the SAME physical face and the SAME physical corner, with only the
    /// which-hull-owns-it bit flipped.
    #[test]
    fn swapping_the_pair_names_the_same_physical_features() {
        let a = hull(0.0, 0.0, 0.0);
        let b = hull(3.0, 0.0, 0.2);
        let forward = obstacles_contact(&a, &b).expect("overlapping").feature;
        let reversed = obstacles_contact(&b, &a).expect("overlapping").feature;

        assert_ne!(forward.reference_is_b, reversed.reference_is_b, "ownership flips");
        assert_eq!(forward.reference_is_end, reversed.reference_is_end, "the same face");
        assert_eq!(forward.reference_positive, reversed.reference_positive);
        assert_eq!(forward.incident_corner, reversed.incident_corner, "the same corner");
    }

    /// The normal says which way `b` lies from `a`, which is what lets the solver push the pair
    /// apart rather than through each other.
    #[test]
    fn the_normal_points_from_a_toward_b() {
        let a = hull(0.0, 0.0, 0.0);
        let b = hull(2.0, 0.0, 0.0); // overlapping to +x (half_width 1.75 each)
        let contact = obstacles_contact(&a, &b).expect("these overlap");
        assert!(contact.normal.x > 0.9, "b lies to +x, got {:?}", contact.normal);

        let behind = hull(-2.0, 0.0, 0.0);
        let contact = obstacles_contact(&a, &behind).expect("these overlap");
        assert!(contact.normal.x < -0.9, "b lies to -x, got {:?}", contact.normal);
    }
}
