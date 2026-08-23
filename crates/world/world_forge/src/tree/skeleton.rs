//! The tree grown ONCE, as pure data (Drzewa 3.0 PR3): a bounded Weber–Penn-style parametric
//! recursion — a species is a per-level parameter table plus a crown envelope, never a grammar.
//! The skeleton owns every decision a tree makes (where a limb leaves the trunk, how a twig
//! curves, where a leaf cluster may sit); the mesh rungs and the impostor only FILTER it, so
//! LOD identity is structural, not a discipline of burned RNG draws.
//!
//! Determinism: every branch draws from its own hashed seed (`seed ^ branch path`), so adding
//! or pruning one branch never reshuffles its siblings — the same stability idea the detail
//! scatter kernel uses for per-placement seeds.

use glam::Vec3;

use crate::shape::Rng;

/// The phyllotactic advance between successive children around their parent's axis.
pub const GOLDEN_ANGLE_RAD: f32 = 2.399_963;

/// The trunk — level 0, the one branch with no parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrunkParams {
    pub height_m: f32,
    /// Radius at chest height; station 0 widens by `flare`, the top narrows to `taper`.
    pub radius_m: f32,
    /// Top radius as a fraction of `radius_m`.
    pub taper: f32,
    /// Station-0 radius multiplier — the root flare where the bole meets the soil.
    pub flare: f32,
    /// Centerline resolution. More stations buy curve, not girth.
    pub stations: u32,
    /// Maximum unit-slope lean, drawn deterministically per individual.
    pub lean: f32,
}

/// One recursion level of the branch table (level 1 = limbs off the trunk).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BranchLevelParams {
    /// Children per parent branch.
    pub count: u32,
    /// ± on `count`, drawn per parent.
    pub count_variance: u32,
    /// Where along the parent children may attach, as fractions of its length.
    pub along_range: (f32, f32),
    /// Child length as a fraction of its parent's length (before envelope and variance).
    pub length_ratio: f32,
    /// ± relative variance on the length.
    pub length_variance: f32,
    /// Child base radius as a fraction of the parent's radius at the attachment point.
    /// Da Vinci's rule holds when `count · radius_ratio² ≲ 1` — the locks check it.
    pub radius_ratio: f32,
    /// Tip radius as a fraction of the child's base radius.
    pub taper: f32,
    /// Angle away from the parent's axis at the attachment.
    pub down_angle_rad: f32,
    /// ± on the down angle.
    pub down_angle_variance_rad: f32,
    /// Total bend accumulated across the branch's stations.
    pub curve_rad: f32,
    /// ± on the curve.
    pub curve_variance_rad: f32,
    /// Vertical pull per station: positive grows toward the sky (poplar shoots), negative
    /// weeps (willow curtains). Zero keeps the launched direction.
    pub tropism: f32,
    /// Centerline resolution of each child.
    pub stations: u32,
}

/// The crown hull: scales child length by attachment height so the family silhouette is a
/// CONSTRUCTION, not scatter luck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeEnvelope {
    /// Broadleaf mass, widest through the middle (oak, fruit).
    Dome,
    /// Narrow and parallel (Lombardy poplar).
    Column,
    /// Widest at the crown base, tapering to the leader (pine).
    Cone,
    /// Wide low skirt (willow).
    Weeping,
}

impl ShapeEnvelope {
    /// Length multiplier for a child attached `height01` of the way up the crown.
    pub fn length_scale(self, height01: f32) -> f32 {
        let h = height01.clamp(0.0, 1.0);
        match self {
            ShapeEnvelope::Dome => 0.55 + 0.45 * (std::f32::consts::PI * (0.15 + 0.7 * h)).sin(),
            ShapeEnvelope::Column => 0.4,
            ShapeEnvelope::Cone => 1.0 - 0.75 * h,
            ShapeEnvelope::Weeping => 1.0 - 0.35 * h,
        }
    }
}

/// A whole species' growth program.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeArchitecture {
    pub trunk: TrunkParams,
    /// The bare-trunk fraction: children of the trunk attach above this.
    pub crown_begin_frac: f32,
    /// Level 1..=N tables. Two levels read at battle range; three is the ceiling.
    pub levels: Vec<BranchLevelParams>,
    pub envelope: ShapeEnvelope,
}

/// One centerline sample of a branch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Station {
    pub position: Vec3,
    pub radius_m: f32,
}

/// One grown branch. `parent` indexes [`TreeSkeleton::branches`]; the trunk has none.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub parent: Option<u32>,
    pub level: u32,
    pub stations: Vec<Station>,
}

impl Branch {
    pub fn base(&self) -> Station {
        self.stations[0]
    }

    pub fn tip(&self) -> Station {
        *self.stations.last().expect("a branch has stations")
    }

    pub fn length_m(&self) -> f32 {
        self.stations.windows(2).map(|pair| pair[0].position.distance(pair[1].position)).sum()
    }
}

/// Where a leaf cluster may sit: on the outer part of a last-level branch, tangent along the
/// growth. The card baker consumes these; the skeleton only offers them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeafAnchor {
    pub position: Vec3,
    pub tangent: Vec3,
    /// Index into [`TreeSkeleton::branches`].
    pub branch: u32,
    /// Fraction of the way along that branch.
    pub t: f32,
}

/// The grown tree: branches in parent-before-child order, plus the leaf anchors.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSkeleton {
    pub branches: Vec<Branch>,
    pub anchors: Vec<LeafAnchor>,
}

impl TreeSkeleton {
    /// The highest centerline point — the tip every LOD rung must agree on.
    pub fn tip_height_m(&self) -> f32 {
        self.branches
            .iter()
            .flat_map(|branch| branch.stations.iter())
            .map(|station| station.position.y)
            .fold(0.0, f32::max)
    }

    pub fn branches_of_level(&self, level: u32) -> impl Iterator<Item = &Branch> {
        self.branches.iter().filter(move |branch| branch.level == level)
    }
}

/// Deterministic per-branch seed: mixing, not sequence — a sibling's entropy never depends on
/// how much its neighbours drew.
fn child_seed(parent_seed: u64, ordinal: u32) -> u64 {
    let mut z = parent_seed ^ (0x9E37_79B9_7F4A_7C15_u64.wrapping_mul(ordinal as u64 + 1));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^ (z >> 31)
}

/// Grow the whole skeleton. Same architecture + same seed = the same tree, station for
/// station; the seed varies the individual inside the species family.
pub fn grow(architecture: &TreeArchitecture, seed: u64) -> TreeSkeleton {
    let mut branches = Vec::new();
    grow_trunk(architecture, seed, &mut branches);
    for level in 1..=architecture.levels.len() as u32 {
        let table = architecture.levels[(level - 1) as usize];
        let parents: Vec<u32> = (0..branches.len() as u32)
            .filter(|&index| branches[index as usize].level == level - 1)
            .collect();
        for parent_index in parents {
            grow_children(architecture, seed, level, table, parent_index, &mut branches);
        }
    }
    let anchors = offer_anchors(architecture, &branches);
    TreeSkeleton { branches, anchors }
}

fn grow_trunk(architecture: &TreeArchitecture, seed: u64, branches: &mut Vec<Branch>) {
    let trunk = architecture.trunk;
    let mut rng = Rng(child_seed(seed ^ 0x7EE5_3000, 0));
    let lean = Vec3::new(rng.signed() * trunk.lean, 0.0, rng.signed() * trunk.lean);
    let stations = trunk.stations.max(2);
    let mut positions = Vec::with_capacity(stations as usize);
    for station in 0..stations {
        let t = station as f32 / (stations - 1) as f32;
        // The lean grows with height (a bole bends, it does not tilt as a rigid pole), and a
        // faint deterministic S keeps a close-up bole from reading extruded.
        let sway = (t * std::f32::consts::PI * 1.7).sin() * trunk.radius_m * 0.18;
        let position = Vec3::new(
            lean.x * trunk.height_m * t * t + sway * lean.x.signum(),
            trunk.height_m * t,
            lean.z * trunk.height_m * t * t - sway * lean.z.signum(),
        );
        positions.push(position);
    }
    let radius_at = |t: f32| -> f32 {
        // Root flare decays fast; above it the linear taper owns the profile.
        let flare = 1.0 + (trunk.flare - 1.0) * (1.0 - (t / 0.12).min(1.0)).powi(2);
        trunk.radius_m * flare * (1.0 - (1.0 - trunk.taper) * t)
    };
    let stations = positions
        .iter()
        .enumerate()
        .map(|(index, &position)| Station {
            position,
            radius_m: radius_at(index as f32 / (positions.len() - 1) as f32),
        })
        .collect();
    branches.push(Branch { parent: None, level: 0, stations });
}

fn grow_children(
    architecture: &TreeArchitecture,
    seed: u64,
    level: u32,
    table: BranchLevelParams,
    parent_index: u32,
    branches: &mut Vec<Branch>,
) {
    let parent = branches[parent_index as usize].clone();
    let parent_length = parent.length_m();
    if parent_length <= f32::EPSILON {
        return;
    }
    let parent_seed = child_seed(seed ^ ((level as u64) << 32), parent_index);
    let mut parent_rng = Rng(parent_seed);
    let count = (table.count as i64
        + (parent_rng.next() % (2 * table.count_variance as u64 + 1)) as i64
        - table.count_variance as i64)
        .max(0) as u32;
    let (along_min, along_max) = table.along_range;
    for ordinal in 0..count {
        let mut rng = Rng(child_seed(parent_seed, ordinal + 1));
        // Attachment: evenly laddered along the range, jittered inside its slot so two limbs
        // never share a height by construction.
        let slot = (ordinal as f32 + 0.3 + rng.unit() * 0.4) / count as f32;
        let along = along_min + (along_max - along_min) * slot;
        let (attach, parent_tangent, parent_radius) = sample_branch(&parent, along);
        // Heading: the phyllotactic spiral around the parent, jittered.
        let heading = ordinal as f32 * GOLDEN_ANGLE_RAD + rng.unit() * 0.9;
        let down = table.down_angle_rad + rng.signed() * table.down_angle_variance_rad;
        let mut direction = launch_direction(parent_tangent, heading, down);
        // Length: the parent's, cut by the table, shaped by the crown envelope at this height.
        let height01 = height_in_crown(architecture, attach.y);
        let envelope = architecture.envelope.length_scale(height01);
        let length = parent_length
            * table.length_ratio
            * envelope
            * (1.0 + rng.signed() * table.length_variance);
        if length < 0.05 {
            continue;
        }
        let base_radius = (parent_radius * table.radius_ratio).min(parent_radius * 0.9);
        let stations = table.stations.max(2);
        let step = length / (stations - 1) as f32;
        let curve = table.curve_rad + rng.signed() * table.curve_variance_rad;
        let curve_step = curve / (stations - 1) as f32;
        let mut position = attach;
        let mut grown = Vec::with_capacity(stations as usize);
        for station in 0..stations {
            let t = station as f32 / (stations - 1) as f32;
            grown.push(Station {
                position,
                radius_m: base_radius * (1.0 - (1.0 - table.taper) * t),
            });
            // Bend: rotate around the branch's horizontal normal (the curve), then let the
            // tropism pull the running direction vertically. Arc, not slide.
            direction = bend(direction, curve_step);
            direction = (direction + Vec3::Y * table.tropism).normalize_or_zero();
            position += direction * step;
        }
        branches.push(Branch { parent: Some(parent_index), level, stations: grown });
    }
}

/// Sample a branch centerline at fraction `t`: position, unit tangent, interpolated radius.
fn sample_branch(branch: &Branch, t: f32) -> (Vec3, Vec3, f32) {
    let stations = &branch.stations;
    let scaled = t.clamp(0.0, 1.0) * (stations.len() - 1) as f32;
    let index = (scaled as usize).min(stations.len() - 2);
    let frac = scaled - index as f32;
    let a = stations[index];
    let b = stations[index + 1];
    let position = a.position.lerp(b.position, frac);
    let tangent = (b.position - a.position).normalize_or_zero();
    let radius = a.radius_m + (b.radius_m - a.radius_m) * frac;
    (position, tangent, radius)
}

/// The child's initial direction: the parent tangent tilted `down` away from itself, headed
/// `heading` around it.
fn launch_direction(parent_tangent: Vec3, heading: f32, down: f32) -> Vec3 {
    let reference = if parent_tangent.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = parent_tangent.cross(reference).normalize_or_zero();
    let v = parent_tangent.cross(u);
    let radial = u * heading.cos() + v * heading.sin();
    (parent_tangent * down.cos() + radial * down.sin()).normalize_or_zero()
}

/// One curve increment: tilt the direction toward the horizon (a broadleaf limb relaxes
/// outward as it grows) by rotating in the vertical plane it lives in.
fn bend(direction: Vec3, angle_rad: f32) -> Vec3 {
    let flat = Vec3::new(direction.x, 0.0, direction.z);
    if flat.length_squared() < 1.0e-8 {
        return direction;
    }
    let out = flat.normalize();
    let pitch = direction.y.atan2(direction.dot(out));
    let bent = pitch - angle_rad;
    (out * bent.cos() + Vec3::Y * bent.sin()).normalize_or_zero()
}

fn height_in_crown(architecture: &TreeArchitecture, y: f32) -> f32 {
    let crown_base = architecture.trunk.height_m * architecture.crown_begin_frac;
    let crown_span = (architecture.trunk.height_m - crown_base).max(0.01);
    ((y - crown_base) / crown_span).clamp(0.0, 1.0)
}

/// Leaf anchors ride the OUTER 60% of every last-level branch — one per station in range plus
/// the tip, tangents along the growth.
fn offer_anchors(architecture: &TreeArchitecture, branches: &[Branch]) -> Vec<LeafAnchor> {
    let last_level = architecture.levels.len() as u32;
    let mut anchors = Vec::new();
    for (index, branch) in branches.iter().enumerate() {
        if branch.level != last_level {
            continue;
        }
        let stations = branch.stations.len();
        for station in 0..stations {
            let t = station as f32 / (stations - 1) as f32;
            if t < 0.4 {
                continue;
            }
            let tangent = if station + 1 < stations {
                (branch.stations[station + 1].position - branch.stations[station].position)
                    .normalize_or_zero()
            } else {
                (branch.stations[station].position - branch.stations[station - 1].position)
                    .normalize_or_zero()
            };
            anchors.push(LeafAnchor {
                position: branch.stations[station].position,
                tangent,
                branch: index as u32,
                t,
            });
        }
    }
    anchors
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oak's REAL growth program (`TreeSpecies::Oak.architecture()`) — the locks below run
    /// against the same table the bake consumes, so a species tweak re-answers to them
    /// immediately instead of to a drifting test copy.
    fn draft_oak() -> TreeArchitecture {
        super::super::TreeSpecies::Oak.architecture().expect("the oak is branched")
    }

    #[test]
    fn the_same_seed_grows_the_same_tree_station_for_station() {
        let oak = draft_oak();
        assert_eq!(grow(&oak, 42), grow(&oak, 42), "growth is deterministic");
        assert_ne!(grow(&oak, 1).branches, grow(&oak, 2).branches, "no two oaks alike");
    }

    /// Radii thin along every path: within a branch station by station, and across every
    /// attachment (a child never outgrows its parent's girth where it leaves it).
    #[test]
    fn radii_thin_along_every_path() {
        let skeleton = grow(&draft_oak(), 7);
        let mut checked = 0;
        for branch in &skeleton.branches {
            for pair in branch.stations.windows(2) {
                assert!(
                    pair[1].radius_m <= pair[0].radius_m + 1.0e-4,
                    "a branch thickens along its own run"
                );
            }
            if let Some(parent) = branch.parent {
                let parent = &skeleton.branches[parent as usize];
                assert!(
                    branch.base().radius_m < parent.base().radius_m,
                    "a child outgrew its parent"
                );
                checked += 1;
            }
        }
        assert!(checked >= 5, "the lock walked a real tree, not a stump: {checked} children");
    }

    /// Da Vinci's observation, banded: at every parent the children's cross-sections sum to no
    /// more than 1.3x the parent's own base cross-section. Wood does not appear from nowhere.
    #[test]
    fn children_obey_the_da_vinci_band() {
        let skeleton = grow(&draft_oak(), 0);
        for (index, parent) in skeleton.branches.iter().enumerate() {
            let sum: f32 = skeleton
                .branches
                .iter()
                .filter(|child| child.parent == Some(index as u32))
                .map(|child| child.base().radius_m.powi(2))
                .sum();
            assert!(
                sum <= 1.3 * parent.base().radius_m.powi(2),
                "branch {index}: children claim {sum} of a {} section",
                parent.base().radius_m.powi(2)
            );
        }
    }

    /// Every child's root sits ON its parent's centerline (inside the tube by construction) —
    /// the no-weld attachment can never open a crack under sway.
    #[test]
    fn every_child_roots_inside_its_parent() {
        let skeleton = grow(&draft_oak(), 3);
        for branch in &skeleton.branches {
            let Some(parent) = branch.parent else { continue };
            let parent = &skeleton.branches[parent as usize];
            let root = branch.base().position;
            let nearest = parent
                .stations
                .windows(2)
                .map(|pair| {
                    let segment = pair[1].position - pair[0].position;
                    let t = ((root - pair[0].position).dot(segment)
                        / segment.length_squared().max(1.0e-8))
                    .clamp(0.0, 1.0);
                    root.distance(pair[0].position + segment * t)
                })
                .fold(f32::MAX, f32::min);
            assert!(
                nearest <= branch.base().radius_m + 1.0e-3,
                "a child floats {nearest} m off its parent's centerline"
            );
        }
    }

    /// Authored counts hold as a band, per level: the table plus its variance, never a runaway
    /// recursion — authored tables, not L-systems.
    #[test]
    fn branch_counts_stay_inside_the_authored_bands() {
        let oak = draft_oak();
        for seed in 0..8 {
            let skeleton = grow(&oak, seed);
            let limbs = skeleton.branches_of_level(1).count() as u32;
            assert!(
                (oak.levels[0].count - oak.levels[0].count_variance
                    ..=oak.levels[0].count + oak.levels[0].count_variance)
                    .contains(&limbs),
                "seed {seed}: {limbs} limbs"
            );
            let twigs = skeleton.branches_of_level(2).count() as u32;
            let per_limb = oak.levels[1];
            assert!(
                twigs >= limbs * (per_limb.count - per_limb.count_variance)
                    && twigs <= limbs * (per_limb.count + per_limb.count_variance),
                "seed {seed}: {twigs} twigs on {limbs} limbs"
            );
        }
    }

    /// The family band: individuals differ, the species holds — tip heights across seeds stay
    /// within ±25% of their own mean, and every tree clears the trunk.
    #[test]
    fn the_species_family_holds_across_seeds() {
        let oak = draft_oak();
        let tips: Vec<f32> = (0..8).map(|seed| grow(&oak, seed).tip_height_m()).collect();
        let mean = tips.iter().sum::<f32>() / tips.len() as f32;
        assert!(mean > oak.trunk.height_m, "the crown rises above the bole: {mean}");
        for (seed, tip) in tips.iter().enumerate() {
            assert!(
                (tip - mean).abs() / mean < 0.25,
                "seed {seed} left the family: tip {tip} vs mean {mean}"
            );
        }
    }

    /// The crown envelope is a construction: no limb's reach exceeds what the dome allows at
    /// its height by more than 15%.
    #[test]
    fn no_branch_leaves_the_crown_envelope() {
        let oak = draft_oak();
        let skeleton = grow(&oak, 5);
        // The dome's widest allowance, in metres: the longest level-1 launch the table can
        // produce at the envelope's peak.
        let trunk_length = skeleton.branches[0].length_m();
        let widest =
            trunk_length * oak.levels[0].length_ratio * (1.0 + oak.levels[0].length_variance);
        for branch in skeleton.branches_of_level(1) {
            let height01 = ((branch.base().position.y - oak.trunk.height_m * oak.crown_begin_frac)
                / (oak.trunk.height_m * (1.0 - oak.crown_begin_frac)))
                .clamp(0.0, 1.0);
            let allowed = widest * oak.envelope.length_scale(height01);
            let reach = branch.length_m();
            assert!(
                reach <= allowed * 1.15,
                "a limb at height01 {height01:.2} reaches {reach:.2} m of an allowed {allowed:.2}"
            );
        }
    }

    /// Anchors ride the outer 60% of last-level branches only — the skeleton offers leaf
    /// seats, it does not scatter them mid-bole.
    #[test]
    fn leaf_anchors_sit_on_the_outer_twigs() {
        let skeleton = grow(&draft_oak(), 11);
        assert!(
            skeleton.anchors.len() >= 40,
            "a crown offers real seats: {}",
            skeleton.anchors.len()
        );
        for anchor in &skeleton.anchors {
            let branch = &skeleton.branches[anchor.branch as usize];
            assert_eq!(branch.level, 2, "anchors live on the last level");
            assert!(anchor.t >= 0.4, "an anchor crept inside the branch: t {}", anchor.t);
            assert!(anchor.tangent.length() > 0.9, "a unit tangent comes with every seat");
        }
    }

    /// Wave 2 (PR8): the weep is a construction — every willow curtain ENDS below where it
    /// began (the negative tropism arcs it over and down), and the crown's skirt reaches
    /// wide of the bole.
    #[test]
    fn the_willow_curtains_fall_and_its_skirt_spreads() {
        let willow = super::super::TreeSpecies::Willow.architecture().expect("wave 2");
        for seed in 0..4 {
            let skeleton = grow(&willow, seed);
            let mut curtains = 0;
            let mut falling = 0;
            for branch in skeleton.branches_of_level(2) {
                curtains += 1;
                falling += u32::from(branch.tip().position.y < branch.base().position.y);
            }
            assert!(curtains >= 12, "seed {seed}: a willow hangs real curtains: {curtains}");
            assert!(
                falling * 10 >= curtains * 8,
                "seed {seed}: the weep must fall: {falling}/{curtains} curtains descend"
            );
            let skirt = skeleton
                .branches_of_level(1)
                .map(|branch| {
                    let tip = branch.tip().position;
                    (tip.x * tip.x + tip.z * tip.z).sqrt()
                })
                .fold(0.0_f32, f32::max);
            // A mature riverside willow spreads 10–16+ m across; the band admits that and
            // still refuses a squat (under 5 m across) or a monster (over 17 m).
            assert!(
                (2.5..=8.5).contains(&skirt),
                "seed {seed}: the skirt reaches wide of the bole: {skirt} m"
            );
        }
    }

    /// The stability contract behind per-branch hashed seeds: pruning one limb's entropy draw
    /// cannot exist, because a sibling's seed never depends on how much its neighbours drew.
    #[test]
    fn sibling_seeds_are_hashed_not_sequential() {
        let a = child_seed(0xDEAD_BEEF, 1);
        let b = child_seed(0xDEAD_BEEF, 2);
        let b_again = child_seed(0xDEAD_BEEF, 2);
        assert_ne!(a, b);
        assert_eq!(b, b_again, "a sibling's seed is a pure function of parent and ordinal");
    }
}
