//! Trees 3.0 (Drzewa 3.0): the species stays a PARAMETER SET, not a model — that law survives
//! every generation of this module. What changed is what the parameters describe. Trees 2.0
//! wrote "never L-systems" and meant it about UNBOUNDED grammar recursion; that stays banned.
//! Its successor is [`skeleton`]: a bounded Weber–Penn-style parametric recursion (2–3 authored
//! levels, counts in the table, budgets analytic) grown ONCE as pure data, which every LOD rung
//! only filters — the SpeedTree ancestor, homegrown, procedural-only per map-forge policy #10.
//!
//! The lobed painterly crown below (FBM icospheres, normals bent from the crown centroid) is
//! the LIVE bake while species migrate one wave at a time; its purpose — a canopy lit as one
//! soft mass, never a triangle salad — transplants into the card shade lane and outlives the
//! lobes themselves. Trunk and canopy come back as separate meshes so the consumer colors them
//! without any material-enum churn.

mod bark;
pub mod leaf_atlas;
pub mod leaves;
pub mod skeleton;

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use crate::WorldMaterial;
use crate::shape::{Rng, icosphere, merge_meshes};

/// The authored species. Numbers live in [`TreeSpecies::params`] — one table, review-gated by
/// the goldens below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TreeSpecies {
    Oak,
    Poplar,
    Willow,
    FruitTree,
    Bush,
    Pine,
}

impl TreeSpecies {
    pub const ALL: [TreeSpecies; 6] = [
        TreeSpecies::Oak,
        TreeSpecies::Poplar,
        TreeSpecies::Willow,
        TreeSpecies::FruitTree,
        TreeSpecies::Bush,
        TreeSpecies::Pine,
    ];

    /// Trunk radius at the butt, metres — the stump a felled tree leaves is sized from this.
    pub fn trunk_radius(self) -> f32 {
        self.params().trunk_radius
    }

    /// Authored trunk height (ground to first crown mass), metres.
    pub fn trunk_height(self) -> f32 {
        self.params().trunk_height
    }

    /// The growth program for a species that has migrated to the skeleton (Drzewa 3.0).
    /// `None` = the legacy lobed bake still owns it; the scaffold dies with the last wave.
    pub fn architecture(self) -> Option<skeleton::TreeArchitecture> {
        use skeleton::{BranchLevelParams, ShapeEnvelope, TrunkParams};
        match self {
            // The first migrant: the battlefield oak, whose numbers mirror its params table —
            // trunk 9.2 m / r 0.52, a 5±1-limb dome. Twig counts are authored for the card
            // canopy (PR6); the bark meshes levels 0–1 only until then.
            TreeSpecies::Oak => Some(skeleton::TreeArchitecture {
                trunk: TrunkParams {
                    height_m: 9.2,
                    radius_m: 0.52,
                    taper: 0.55,
                    flare: 1.35,
                    stations: 7,
                    lean: 0.10,
                },
                crown_begin_frac: 0.5,
                levels: vec![
                    // Limbs reach UP as much as out (down angle 0.62 rad off the vertical
                    // bole, positive tropism): the card crown they carry must top 15 m — the
                    // mature-height floor the lobed canopy used to clear with its spheres.
                    BranchLevelParams {
                        count: 5,
                        count_variance: 1,
                        along_range: (0.55, 0.95),
                        length_ratio: 0.78,
                        length_variance: 0.18,
                        radius_ratio: 0.38,
                        taper: 0.35,
                        down_angle_rad: 0.50,
                        down_angle_variance_rad: 0.18,
                        curve_rad: 0.5,
                        curve_variance_rad: 0.2,
                        tropism: 0.14,
                        stations: 5,
                    },
                    BranchLevelParams {
                        count: 7,
                        count_variance: 2,
                        along_range: (0.3, 0.95),
                        length_ratio: 0.45,
                        length_variance: 0.25,
                        radius_ratio: 0.42,
                        taper: 0.3,
                        down_angle_rad: 0.75,
                        down_angle_variance_rad: 0.25,
                        curve_rad: 0.4,
                        curve_variance_rad: 0.25,
                        tropism: -0.02,
                        stations: 3,
                    },
                ],
                envelope: ShapeEnvelope::Dome,
            }),
            // Wave 1 (PR7): the Lombardy poplar — one bole running the FULL height (the
            // lobed stack faked 21.9 m; the skeleton's trunk honestly grows it), short
            // steeply-rising branches all the way up, hard positive tropism, the Column
            // envelope keeping every reach tight. The mature floor is 19 m.
            TreeSpecies::Poplar => Some(skeleton::TreeArchitecture {
                trunk: TrunkParams {
                    height_m: 19.6,
                    radius_m: 0.37,
                    taper: 0.30,
                    flare: 1.25,
                    stations: 8,
                    lean: 0.05,
                },
                crown_begin_frac: 0.22,
                levels: vec![
                    BranchLevelParams {
                        count: 10,
                        count_variance: 2,
                        along_range: (0.22, 0.97),
                        length_ratio: 0.26,
                        length_variance: 0.22,
                        radius_ratio: 0.30,
                        taper: 0.35,
                        down_angle_rad: 0.55,
                        down_angle_variance_rad: 0.15,
                        curve_rad: 0.25,
                        curve_variance_rad: 0.12,
                        tropism: 0.30,
                        stations: 4,
                    },
                    BranchLevelParams {
                        count: 4,
                        count_variance: 1,
                        along_range: (0.3, 0.95),
                        length_ratio: 0.5,
                        length_variance: 0.25,
                        radius_ratio: 0.45,
                        taper: 0.3,
                        down_angle_rad: 0.55,
                        down_angle_variance_rad: 0.2,
                        curve_rad: 0.25,
                        curve_variance_rad: 0.15,
                        tropism: 0.12,
                        stations: 3,
                    },
                ],
                envelope: ShapeEnvelope::Column,
            }),
            // Wave 1 (PR7): the orchard fruit tree — a short bole opening into a low, wide
            // dome; heavy down-angles and a whisper of droop, the way a laden apple grows.
            TreeSpecies::FruitTree => Some(skeleton::TreeArchitecture {
                trunk: TrunkParams {
                    height_m: 2.1,
                    radius_m: 0.18,
                    taper: 0.5,
                    flare: 1.3,
                    stations: 5,
                    lean: 0.12,
                },
                crown_begin_frac: 0.45,
                levels: vec![
                    BranchLevelParams {
                        count: 4,
                        count_variance: 1,
                        along_range: (0.5, 0.95),
                        length_ratio: 0.85,
                        length_variance: 0.2,
                        radius_ratio: 0.42,
                        taper: 0.35,
                        down_angle_rad: 0.95,
                        down_angle_variance_rad: 0.2,
                        curve_rad: 0.45,
                        curve_variance_rad: 0.2,
                        tropism: -0.02,
                        stations: 4,
                    },
                    BranchLevelParams {
                        count: 5,
                        count_variance: 2,
                        along_range: (0.3, 0.95),
                        length_ratio: 0.5,
                        length_variance: 0.25,
                        radius_ratio: 0.45,
                        taper: 0.3,
                        down_angle_rad: 0.8,
                        down_angle_variance_rad: 0.25,
                        curve_rad: 0.35,
                        curve_variance_rad: 0.2,
                        tropism: -0.04,
                        stations: 3,
                    },
                ],
                envelope: ShapeEnvelope::Dome,
            }),
            TreeSpecies::Willow | TreeSpecies::Bush | TreeSpecies::Pine => None,
        }
    }

    fn params(self) -> SpeciesParams {
        match self {
            // Broad, muscular: a tall trunk, heavy limbs, a wide 4-lobe crown. Scaled to a
            // real mature oak (~17-18 m) — the whole species k=2.0 over its first draft, trunk
            // radius by k^0.6 so a taller tree reads correctly SLENDER, not a fat stump.
            TreeSpecies::Oak => SpeciesParams {
                trunk_height: 9.2,
                trunk_radius: 0.52,
                taper: 0.55,
                limbs: 4,
                limb_length: 4.4,
                limb_pitch: 0.9,
                lobes: 4,
                lobe_radius: 4.6,
                crown_height: 11.8,
                crown_spread: 3.4,
                fbm_amplitude: 0.34,
                crown_stack_height: 0.0,
                crown_tip_frac: 1.0,
            },
            // A column: minimal limbs, stacked narrow lobes. A Lombardy poplar reaches ~22 m
            // and stays slim — k=2.4 on height, the spread barely grows.
            TreeSpecies::Poplar => SpeciesParams {
                trunk_height: 15.6,
                trunk_radius: 0.37,
                taper: 0.45,
                limbs: 2,
                limb_length: 2.2,
                limb_pitch: 1.25,
                lobes: 3,
                lobe_radius: 3.2,
                crown_height: 17.8,
                crown_spread: 1.1,
                fbm_amplitude: 0.22,
                crown_stack_height: 0.0,
                crown_tip_frac: 1.0,
            },
            // Weeping: a short trunk, long drooping limbs, a low wide crown. ~15 m riverside
            // willow — k=2.0, the wide skirt scales with it.
            TreeSpecies::Willow => SpeciesParams {
                trunk_height: 6.4,
                trunk_radius: 0.46,
                taper: 0.6,
                limbs: 5,
                limb_length: 5.2,
                limb_pitch: 0.35,
                lobes: 3,
                lobe_radius: 5.0,
                crown_height: 8.2,
                crown_spread: 3.8,
                fbm_amplitude: 0.42,
                crown_stack_height: 0.0,
                crown_tip_frac: 1.0,
            },
            // Orchard scale: short and round.
            TreeSpecies::FruitTree => SpeciesParams {
                trunk_height: 2.2,
                trunk_radius: 0.18,
                taper: 0.55,
                limbs: 3,
                limb_length: 1.2,
                limb_pitch: 0.8,
                lobes: 2,
                lobe_radius: 1.5,
                crown_height: 3.1,
                crown_spread: 0.8,
                fbm_amplitude: 0.30,
                crown_stack_height: 0.0,
                crown_tip_frac: 1.0,
            },
            // No trunk to speak of: the honest concealment bush stays a soft blob cluster.
            TreeSpecies::Bush => SpeciesParams {
                trunk_height: 0.5,
                trunk_radius: 0.10,
                taper: 0.7,
                limbs: 0,
                limb_length: 0.0,
                limb_pitch: 0.0,
                lobes: 3,
                lobe_radius: 1.1,
                crown_height: 1.1,
                crown_spread: 0.9,
                fbm_amplitude: 0.38,
                crown_stack_height: 0.0,
                crown_tip_frac: 1.0,
            },
            // The conifer: a bare lower trunk, then lobes STACKED up the axis with radii
            // tapering toward a tip — a cone by construction, not by scatter luck. A mature
            // pine is ~20-22 m; k=2.7, and the cone stays narrow (spread barely moves).
            TreeSpecies::Pine => SpeciesParams {
                trunk_height: 19.4,
                trunk_radius: 0.47,
                taper: 0.4,
                limbs: 0,
                limb_length: 0.0,
                limb_pitch: 0.0,
                lobes: 6,
                lobe_radius: 4.0,
                crown_height: 6.5,
                crown_spread: 0.3,
                fbm_amplitude: 0.22,
                crown_stack_height: 12.4,
                crown_tip_frac: 0.32,
            },
        }
    }
}

struct SpeciesParams {
    trunk_height: f32,
    trunk_radius: f32,
    /// Top radius as a fraction of the base radius.
    taper: f32,
    limbs: u32,
    limb_length: f32,
    /// Radians above horizontal the limbs reach.
    limb_pitch: f32,
    lobes: u32,
    lobe_radius: f32,
    crown_height: f32,
    /// How far lobe centers scatter from the crown axis.
    crown_spread: f32,
    fbm_amplitude: f32,
    /// When positive, the crown becomes a STACK: lobes climb this span up the axis from
    /// `crown_height` instead of scattering around it — the conifer silhouette.
    crown_stack_height: f32,
    /// Stacked crowns only: the top lobe's radius as a fraction of `lobe_radius`.
    crown_tip_frac: f32,
}

/// A baked tree: trunk (bark) and canopy meshes, plus the card canopy (Drzewa 3.0) — a
/// migrated species carries its crown as [`leaves::LeafCard`] data and an EMPTY canopy mesh;
/// a legacy species carries lobes and an empty card list. The consumer colors each.
#[derive(Debug, Clone)]
pub struct BakedTree {
    pub species: TreeSpecies,
    pub trunk: GeometryMesh,
    pub canopy: GeometryMesh,
    pub leaves: Vec<leaves::LeafCard>,
}

impl BakedTree {
    pub fn triangle_count(&self) -> usize {
        self.trunk.triangle_count() + self.canopy.triangle_count()
    }

    /// The rendered tip, metres: the highest point of ANY representation — bark, lobes or the
    /// card deck. This is the number the TreeLine honesty locks and the LOD tip invariant
    /// measure; a crown of cards that outgrew its mesh bounds must still be contained.
    pub fn tip(&self) -> f32 {
        let mesh_tip = [&self.trunk, &self.canopy]
            .into_iter()
            .filter_map(|mesh| mesh.bounds().map(|bounds| bounds.max.y))
            .fold(0.0_f32, f32::max);
        let card_tip = self
            .leaves
            .iter()
            .map(|card| card.center.y + card.half_right.y.abs() + card.half_up.y.abs())
            .fold(0.0_f32, f32::max);
        mesh_tip.max(card_tip)
    }

    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for mesh in [&self.trunk, &self.canopy] {
            for vertex in mesh.vertices() {
                for value in vertex.position.to_array().into_iter().chain(vertex.normal.to_array())
                {
                    super::fnv(&mut hash, u64::from(value.to_bits()));
                }
            }
            for index in mesh.indices() {
                super::fnv(&mut hash, u64::from(*index));
            }
        }
        // The card deck is silhouette too: a card that moves is a tree that changed.
        for card in &self.leaves {
            for value in card
                .center
                .to_array()
                .into_iter()
                .chain(card.half_right.to_array())
                .chain(card.half_up.to_array())
                .chain(card.normal.to_array())
                .chain([card.shade])
            {
                super::fnv(&mut hash, u64::from(value.to_bits()));
            }
            super::fnv(&mut hash, u64::from(card.slot));
        }
        hash
    }
}

/// LOD0/LOD1 budgets: the full close tree and the mid-distance one. LOD2 stays the existing
/// painted frustum stack in `foliage.rs` — at 300 m it reads perfectly and costs nothing.
///
/// The ceiling was 1,200 through the lobed era. Widened DELIBERATELY for the migration
/// (Drzewa 3.0 PR4): a bole under the roundness law spends ~370 tris where the hand-typed
/// 7-sided prism spent 84, and the measured cost of the whole scatter is the probe's
/// (baseline 2026-08-22: worst view 10.036 ms of a 16.667 budget, ~26 oaks). PR12 narrows
/// this back around the final card-canopy numbers.
pub const TREE_LOD0_TRIS: std::ops::RangeInclusive<usize> = 180..=2_600;
pub const TREE_LOD1_MAX_TRIS: usize = 260;

/// The review gate for the whole species table at seed 0 (goldens; bless deliberately).
pub const TREE_GOLDEN_HASHES: [(TreeSpecies, u64); 6] = [
    // Blessed 2026-08-03 with the trees-to-scale pass (Oak 17.6 m, Poplar 21.9, Willow 14.5,
    // Pine 20.4 — realistic mature heights; FruitTree/Bush unchanged).
    // Oak re-blessed 2026-08-22 (Drzewa 3.0 PR6): the lobes are DEAD for the oak — the crown
    // is the card deck (~200 cluster cards on the twig anchors, shade lane carrying the
    // one-mass law), the bark meshes the whole skeleton including twigs, and the limbs grew
    // upright so the card crown clears the 15 m mature floor the spheres used to clear.
    (TreeSpecies::Oak, 0xf7cd_1555_8cf2_d40a),
    // Poplar re-blessed 2026-08-22 (Drzewa 3.0 PR7): skeleton + cards — one bole honestly
    // grown to 19.6 m, the Column envelope, hard up-tropism.
    (TreeSpecies::Poplar, 0xf7ff_cf98_86f7_e353),
    (TreeSpecies::Willow, 0xe919_48a3_f6cf_487d),
    // FruitTree re-blessed 2026-08-22 (Drzewa 3.0 PR7): skeleton + cards — a short bole
    // opening into a low orchard dome with heavy down-angles.
    (TreeSpecies::FruitTree, 0x514a_0361_ea03_cf4a),
    (TreeSpecies::Bush, 0x9706_2456_e825_0149),
    (TreeSpecies::Pine, 0x3b7d_1202_48cc_dc4d),
];

/// Bake one tree. `seed` varies the individual (limb headings, lobe scatter, FBM phases) —
/// same species, same silhouette family, never the same tree twice in a shelterbelt.
pub fn bake_tree(species: TreeSpecies, seed: u64) -> BakedTree {
    bake_tree_lod(species, seed, TreeLod::Close)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeLod {
    Close,
    Mid,
}

pub fn bake_tree_lod(species: TreeSpecies, seed: u64, lod: TreeLod) -> BakedTree {
    // The migrated path (Drzewa 3.0): the skeleton grows ONCE, the rung meshes its bark and
    // deals its card deck — no lobes, no entropy burn (every branch and card draws from
    // hashed seeds, so Close/Mid share identity by construction). The lobed body below is
    // the legacy path, scaffolding that dies with the last species wave.
    if let Some(architecture) = species.architecture() {
        let skeleton = skeleton::grow(&architecture, seed);
        return BakedTree {
            species,
            trunk: bark::mesh_bark(&skeleton, lod),
            canopy: GeometryMesh::new(Vec::new(), Vec::new()),
            leaves: leaves::grow_cards(&skeleton, species, seed, lod),
        };
    }

    let params = species.params();
    let mut rng = Rng(seed ^ 0x7EE5_0000 ^ species as u64);
    let (trunk_sides, lobe_subdiv) = match lod {
        TreeLod::Close => (7, 1),
        TreeLod::Mid => (5, 0),
    };

    let trunk = {
        // Legacy lobed-era bark: a tapered tube with a slight deterministic lean.
        let lean = Vec3::new(rng.signed() * 0.10, 0.0, rng.signed() * 0.10);
        let mut trunk = tapered_tube(
            Vec3::ZERO,
            Vec3::new(
                lean.x * params.trunk_height,
                params.trunk_height,
                lean.z * params.trunk_height,
            ),
            params.trunk_radius,
            params.trunk_radius * params.taper,
            trunk_sides,
        );
        // Limbs: level-one branches reaching from the upper trunk toward the crown.
        // Mid burns the same RNG draws Close spends on limbs so the crown identity — and the
        // canopy tip — stay shared across LOD rungs. Without the burn, Mid's first lobe would
        // consume a limb's entropy and the tree would silently shrink as the camera backed off.
        for limb in 0..params.limbs {
            let heading = rng.unit() * std::f32::consts::TAU
                + limb as f32 / params.limbs.max(1) as f32 * std::f32::consts::TAU;
            let pitch = params.limb_pitch + rng.signed() * 0.15;
            let start = Vec3::new(0.0, params.trunk_height * (0.55 + rng.unit() * 0.25), 0.0);
            let direction =
                Vec3::new(heading.cos() * pitch.cos(), pitch.sin(), heading.sin() * pitch.cos());
            let end = start + direction * (params.limb_length * (0.8 + rng.unit() * 0.4));
            if lod == TreeLod::Close {
                let limb_mesh = tapered_tube(
                    start,
                    end,
                    params.trunk_radius * 0.42,
                    params.trunk_radius * 0.16,
                    5,
                );
                trunk = merge_meshes(trunk, limb_mesh);
            }
        }
        trunk
    };

    // Crown: FBM-displaced icosphere lobes; normals bent from the CROWN centroid afterwards.
    let mut canopy_vertices: Vec<GeometryVertex> = Vec::new();
    let mut canopy_indices: Vec<u32> = Vec::new();
    let mut lobe_centers = Vec::new();
    for lobe in 0..params.lobes {
        let (center, radius) = if params.crown_stack_height > 0.0 {
            // Stacked (conifer) crown: lobe `t` of the way up the stack, radius tapering to
            // the tip fraction — the cone is the construction, the FBM only roughs its skirt.
            let t = lobe as f32 / (params.lobes - 1).max(1) as f32;
            let center = Vec3::new(
                rng.signed() * params.crown_spread,
                params.crown_height + t * params.crown_stack_height,
                rng.signed() * params.crown_spread,
            );
            let taper = 1.0 - (1.0 - params.crown_tip_frac) * t;
            (center, params.lobe_radius * taper * (0.85 + rng.unit() * 0.3))
        } else {
            let angle = lobe as f32 / params.lobes as f32 * std::f32::consts::TAU + rng.unit();
            let scatter = if params.lobes == 1 { 0.0 } else { params.crown_spread };
            let center = Vec3::new(
                angle.cos() * scatter * (0.6 + rng.unit() * 0.4),
                params.crown_height + rng.signed() * 0.35 * params.lobe_radius,
                angle.sin() * scatter * (0.6 + rng.unit() * 0.4),
            );
            (center, params.lobe_radius * (0.75 + rng.unit() * 0.4))
        };
        lobe_centers.push(center);
        let phase = rng.next() as u32 as f32 * 1.0e-6;
        let base = canopy_vertices.len() as u32;
        let (positions, indices) = icosphere(lobe_subdiv);
        for unit in &positions {
            let wobble = 1.0
                + params.fbm_amplitude
                    * (0.6 * (unit.x * 3.1 + unit.y * 5.3 + phase).sin()
                        + 0.4 * (unit.z * 7.7 - unit.y * 2.9 + phase * 1.7).sin());
            canopy_vertices.push(GeometryVertex::new(
                center + *unit * radius * wobble,
                *unit,
                WorldMaterial::Canopy.carrier(),
                SmoothingGroup(1),
            ));
        }
        canopy_indices.extend(indices.iter().map(|index| index + base));
    }
    // The painterly trick: light the canopy as ONE mass — every normal points away from the
    // crown centroid, so lobes shade like a single soft volume, not intersecting balls.
    let centroid = lobe_centers.iter().copied().sum::<Vec3>() / lobe_centers.len().max(1) as f32;
    for vertex in &mut canopy_vertices {
        vertex.normal = (vertex.position - centroid).normalize_or_zero();
    }

    let mut tree = BakedTree {
        species,
        trunk,
        canopy: GeometryMesh::new(canopy_vertices, canopy_indices),
        leaves: Vec::new(),
    };
    // LOD must not shrink the silhouette (Świat 2.0 PR1): Mid's coarser lobes undershoot the
    // FBM peaks Close reaches. Lift Mid onto Close's tip so a rung swap moves triangles, never
    // metres. Close is baked once here as the reference — Mid is the shipping far rung, so
    // paying Close's cost at Mid bake time is the price of an honest ladder.
    if lod == TreeLod::Mid {
        let close_tip = canopy_tip(&bake_tree_lod(species, seed, TreeLod::Close));
        let mid_tip = canopy_tip(&tree);
        if mid_tip > 0.01 && close_tip > mid_tip * 1.002 {
            tree = scale_tree_y(tree, close_tip / mid_tip);
        }
    }
    tree
}

fn canopy_tip(tree: &BakedTree) -> f32 {
    tree.canopy.bounds().map(|bounds| bounds.max.y).unwrap_or(0.0)
}

/// Uniform Y scale around the ground plane — keeps the butt planted and lifts the tip.
fn scale_tree_y(tree: BakedTree, scale: f32) -> BakedTree {
    let scale_mesh = |mesh: GeometryMesh| {
        let vertices: Vec<GeometryVertex> = mesh
            .vertices()
            .iter()
            .map(|vertex| {
                let mut scaled = *vertex;
                scaled.position.y *= scale;
                scaled
            })
            .collect();
        GeometryMesh::new(vertices, mesh.indices().to_vec())
    };
    // Legacy-only: the migrated path never lifts (its rungs share one skeleton), so the empty
    // card deck passes through untouched.
    BakedTree {
        species: tree.species,
        trunk: scale_mesh(tree.trunk),
        canopy: scale_mesh(tree.canopy),
        leaves: tree.leaves,
    }
}

/// A tapered open tube from `a` to `b` (no caps: the base sits in the ground, the tip inside
/// the canopy). Flat side facets — bark reads hard-edged at battle range.
fn tapered_tube(a: Vec3, b: Vec3, radius_a: f32, radius_b: f32, sides: u32) -> GeometryMesh {
    let axis = (b - a).normalize_or_zero();
    let reference = if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = axis.cross(reference).normalize_or_zero();
    let v = axis.cross(u);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for side in 0..sides {
        let a0 = side as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (side + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let mid = (a0 + a1) * 0.5;
        let normal = u * mid.cos() + v * mid.sin();
        let ring = |angle: f32, radius: f32, origin: Vec3| {
            origin + (u * angle.cos() + v * angle.sin()) * radius
        };
        let base = vertices.len() as u32;
        for corner in [
            ring(a0, radius_a, a),
            ring(a1, radius_a, a),
            ring(a1, radius_b, b),
            ring(a0, radius_b, b),
        ] {
            vertices.push(GeometryVertex::new(
                corner,
                normal,
                WorldMaterial::Bark.carrier(),
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_species_bakes_deterministic_within_budget_and_on_its_golden() {
        for (species, golden) in TREE_GOLDEN_HASHES {
            let first = bake_tree(species, 0);
            let second = bake_tree(species, 0);
            assert_eq!(first.deterministic_hash(), second.deterministic_hash(), "{species:?}");
            assert!(
                TREE_LOD0_TRIS.contains(&first.triangle_count()),
                "{species:?} LOD0 budget: {} tris",
                first.triangle_count()
            );
            let mid = bake_tree_lod(species, 0, TreeLod::Mid);
            assert!(
                mid.triangle_count() <= TREE_LOD1_MAX_TRIS,
                "{species:?} LOD1 budget: {} tris",
                mid.triangle_count()
            );
            assert_eq!(
                first.deterministic_hash(),
                golden,
                "{species:?}: the silhouette changed — bless deliberately (0x{:016x})",
                first.deterministic_hash()
            );
        }
    }

    /// The scale floor: the trees-to-scale pass put mature species at realistic heights, and a
    /// later edit must not silently shrink them back to the diorama saplings they were (Oak was
    /// 8.6 m, Pine 7.5). One number per species, comfortably below the blessed heights.
    #[test]
    fn the_canopy_reaches_a_realistic_mature_height() {
        // `tip()` covers both crown representations: lobes for the legacy species, the card
        // deck for the migrated ones.
        let top = |species| bake_tree(species, 0).tip();
        assert!(top(TreeSpecies::Oak) > 15.0, "oak: {}", top(TreeSpecies::Oak));
        assert!(top(TreeSpecies::Poplar) > 19.0, "poplar: {}", top(TreeSpecies::Poplar));
        assert!(top(TreeSpecies::Willow) > 12.0, "willow: {}", top(TreeSpecies::Willow));
        assert!(top(TreeSpecies::Pine) > 18.0, "pine: {}", top(TreeSpecies::Pine));
    }

    /// LOD must not shrink the tree (Świat 2.0 PR1): Mid stands at the same tip as Close, so a
    /// rung swap moves triangles, never metres. Checked across a handful of seeds per species.
    #[test]
    fn mid_lod_does_not_shrink_the_canopy_tip() {
        for species in TreeSpecies::ALL {
            for seed in [0_u64, 1, 7, 42] {
                let close = bake_tree_lod(species, seed, TreeLod::Close);
                let mid = bake_tree_lod(species, seed, TreeLod::Mid);
                let close_tip = close.tip();
                let mid_tip = mid.tip();
                assert!(
                    (mid_tip - close_tip).abs() < 0.05,
                    "{species:?} seed {seed}: Mid tip {mid_tip} vs Close tip {close_tip}"
                );
                assert!(
                    mid.triangle_count() <= close.triangle_count(),
                    "{species:?}: Mid must be the cheaper rung"
                );
            }
        }
    }

    #[test]
    fn individuals_differ_but_the_species_family_holds() {
        let a = bake_tree(TreeSpecies::Oak, 1);
        let b = bake_tree(TreeSpecies::Oak, 2);
        assert_ne!(a.deterministic_hash(), b.deterministic_hash(), "no two oaks alike");
        // Family: both stay inside the LOD0 budget and within ~30% height of each other.
        let (ha, hb) = (a.tip(), b.tip());
        assert!((ha - hb).abs() / ha.max(hb) < 0.3, "oaks stay oak-sized: {ha} vs {hb}");
    }

    /// The painterly one-mass law on the LEGACY lobed species (the willow, until its wave
    /// migrates it). The oak's cards carry the law's heir in their shade lane — locked in
    /// `leaves::tests::the_crown_core_shades_darker_than_the_rim`.
    #[test]
    fn canopy_normals_point_away_from_the_crown_centroid() {
        let tree = bake_tree(TreeSpecies::Willow, 0);
        let centroid =
            tree.canopy.vertices().iter().map(|vertex| vertex.position).sum::<glam::Vec3>()
                / tree.canopy.vertex_count().max(1) as f32;
        let aligned = tree
            .canopy
            .vertices()
            .iter()
            .filter(|vertex| {
                (vertex.position - centroid).normalize_or_zero().dot(vertex.normal) > 0.5
            })
            .count();
        assert!(
            aligned * 10 >= tree.canopy.vertex_count() * 9,
            "the painterly one-mass normal trick must hold: {aligned}/{}",
            tree.canopy.vertex_count()
        );
    }
}
