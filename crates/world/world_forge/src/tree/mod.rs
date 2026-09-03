//! Trees 3.0 (Drzewa 3.0): the species stays a PARAMETER SET, not a model — that law survives
//! every generation of this module. What changed is what the parameters describe. Trees 2.0
//! wrote "never L-systems" and meant it about UNBOUNDED grammar recursion; that stays banned.
//! Its successor is [`skeleton`]: a bounded Weber–Penn-style parametric recursion (2–3 authored
//! levels, counts in the table, budgets analytic) grown ONCE as pure data, which every LOD rung
//! only filters — the SpeedTree ancestor, homegrown, procedural-only per map-forge policy #10.
//!
//! The lobed painterly crown is GONE (wave 3 closed the migration): its purpose — a canopy
//! lit as one soft mass, never a triangle salad — lives on in the card shade lane, exactly as
//! planned when the trick was invented. Bark, occlusion hull and the card deck come back as
//! separate parts so the consumer colors them without any material-enum churn.

pub mod authored;
mod bark;
pub mod leaf_atlas;
pub mod leaves;
pub mod skeleton;

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use crate::WorldMaterial;
use crate::shape::icosphere;

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
            // Wave 2 (PR8): the weeping willow — the sweep kernel's showcase. Long limbs
            // launch RISING (down 0.55 off the bole), then the negative tropism arcs them
            // over into the weep; the level-2 curtains launch off the limbs and fall hard.
            // The riverside willow is a TALL tree (the 12 m mature floor): the drama is the
            // fall, not a squat.
            TreeSpecies::Willow => Some(skeleton::TreeArchitecture {
                trunk: TrunkParams {
                    height_m: 10.4,
                    radius_m: 0.46,
                    taper: 0.45,
                    flare: 1.4,
                    stations: 6,
                    lean: 0.12,
                },
                crown_begin_frac: 0.45,
                levels: vec![
                    BranchLevelParams {
                        count: 6,
                        count_variance: 1,
                        along_range: (0.5, 0.95),
                        length_ratio: 0.75,
                        length_variance: 0.2,
                        radius_ratio: 0.36,
                        taper: 0.3,
                        down_angle_rad: 0.55,
                        down_angle_variance_rad: 0.2,
                        curve_rad: 0.7,
                        curve_variance_rad: 0.2,
                        tropism: -0.16,
                        stations: 5,
                    },
                    BranchLevelParams {
                        count: 6,
                        count_variance: 2,
                        along_range: (0.35, 1.0),
                        length_ratio: 0.55,
                        length_variance: 0.25,
                        radius_ratio: 0.4,
                        taper: 0.25,
                        down_angle_rad: 1.25,
                        down_angle_variance_rad: 0.2,
                        curve_rad: 0.3,
                        curve_variance_rad: 0.15,
                        tropism: -0.38,
                        stations: 4,
                    },
                ],
                envelope: ShapeEnvelope::Weeping,
            }),
            // Wave 2 (PR8): the bush — a one-level skeleton: a hand-high stub fanning wide
            // into dense small cards. Still knee-to-chest scenery that HONESTLY conceals
            // nothing (its cover box does not exist).
            TreeSpecies::Bush => Some(skeleton::TreeArchitecture {
                trunk: TrunkParams {
                    height_m: 0.45,
                    radius_m: 0.10,
                    taper: 0.6,
                    flare: 1.2,
                    stations: 3,
                    lean: 0.15,
                },
                crown_begin_frac: 0.1,
                // Dense on purpose: the steppe's overcast value structure leans on bushes
                // for its dark plane (rule 1), and an airy tuft bleaches the whole frame.
                levels: vec![BranchLevelParams {
                    count: 12,
                    count_variance: 2,
                    along_range: (0.15, 0.9),
                    // The lobed blob stood ~2.6-3 m across and the steppe's read was sized
                    // to it — the skeleton tuft matches that footprint, not a garden shrub's.
                    length_ratio: 2.3,
                    length_variance: 0.3,
                    radius_ratio: 0.5,
                    taper: 0.35,
                    down_angle_rad: 1.1,
                    down_angle_variance_rad: 0.3,
                    curve_rad: 0.35,
                    curve_variance_rad: 0.2,
                    tropism: 0.08,
                    stations: 4,
                }],
                envelope: ShapeEnvelope::Dome,
            }),
            // Wave 3 (PR9), the last migrant: the pine — a monopodial pole running the full
            // height, dense near-horizontal branches all the way up the crown, and the Cone
            // envelope shortening them toward the leader: the cone is the CONSTRUCTION, as it
            // always was. Needle-frond cards ride the branches. The mature floor is 18 m.
            TreeSpecies::Pine => Some(skeleton::TreeArchitecture {
                trunk: TrunkParams {
                    height_m: 20.0,
                    radius_m: 0.47,
                    taper: 0.20,
                    flare: 1.3,
                    stations: 8,
                    lean: 0.04,
                },
                crown_begin_frac: 0.30,
                levels: vec![BranchLevelParams {
                    count: 26,
                    count_variance: 4,
                    along_range: (0.30, 0.98),
                    length_ratio: 0.16,
                    length_variance: 0.25,
                    radius_ratio: 0.22,
                    taper: 0.3,
                    down_angle_rad: 1.35,
                    down_angle_variance_rad: 0.15,
                    curve_rad: 0.15,
                    curve_variance_rad: 0.1,
                    tropism: -0.04,
                    // Six stations: a pine branch offers twice the frond anchors of a
                    // broadleaf twig — the needle mass lives ON the branches, and a sparse
                    // deck reads as a dead pole, not a conifer.
                    stations: 6,
                }],
                envelope: ShapeEnvelope::Cone,
            }),
        }
    }

    /// The gameplay anatomy the fells and stumps are sized from — mirrors each species'
    /// [`Self::architecture`] trunk. (The lobed era carried a 13-field visual table here;
    /// the skeleton owns all of that now, and only the gameplay numbers remain.)
    fn params(self) -> SpeciesParams {
        match self {
            TreeSpecies::Oak => SpeciesParams { trunk_height: 9.2, trunk_radius: 0.52 },
            TreeSpecies::Poplar => SpeciesParams { trunk_height: 19.6, trunk_radius: 0.37 },
            TreeSpecies::Willow => SpeciesParams { trunk_height: 10.4, trunk_radius: 0.46 },
            TreeSpecies::FruitTree => SpeciesParams { trunk_height: 2.1, trunk_radius: 0.18 },
            TreeSpecies::Bush => SpeciesParams { trunk_height: 0.45, trunk_radius: 0.10 },
            TreeSpecies::Pine => SpeciesParams { trunk_height: 20.0, trunk_radius: 0.47 },
        }
    }
}

struct SpeciesParams {
    trunk_height: f32,
    trunk_radius: f32,
}

/// A baked tree: bark, the interior occlusion hull (where the species keeps one), and the
/// card canopy — every species rides the skeleton now (Drzewa 3.0, complete with wave 3).
/// The consumer colors each part.
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

/// LOD0/LOD1 MESH budgets (bark + occlusion hull; the card decks have their own counts in
/// the consumers). The backdrop ring keeps the painted frusta in `foliage.rs` — at
/// kilometres they read identically and cost almost nothing.
///
/// Narrowed to the migration's FINAL numbers (Drzewa 3.0 PR12), measured across seeds
/// 0/1/7/42/100: Close spans 388 (bush) to 2,298 (a dense pine); Mid spans 36 to 136 (a
/// Mid bake is the bole alone — past 55 m the limbs live inside the card mass). The floor
/// refuses a species silently degenerating into a stick figure; the ceilings refuse silent
/// growth. The frame verdict stays the flora_frame_probe's two views.
/// Re-measured after the user's quality verdict (2026-08-22, cross-pair clusters + limbs at
/// Mid): Close spans 388 (bush) to 1,838 (oak — the 12 cm thin-stick rule slimmed the card-
/// covered wood everywhere); Mid spans 124 to 976 (a dense pine keeps its whole whorl fan,
/// because the 55 m swap must never amputate the tree's anatomy).
/// Ceiling raised 2,000 → 3,500 on 2026-09-02 (route 2): the AUTHORED oak's wood — Sapling's
/// trunk and limbs at eight sides plus every twig over 4.5 cm at four, so the cluster cards
/// sit at the crown's envelope instead of pulled onto bare limbs — measures 3,128; the cards
/// still count in the consumers, and the frame verdict is still the flora_frame_probe's.
/// Widened again for the variants (2026-09-02, late): the old willow keeps 131 pieces of
/// pendulous wood at 8,784 triangles; the Mid rung of the same tree 2,100.
pub const TREE_LOD0_TRIS: std::ops::RangeInclusive<usize> = 300..=12_000;
pub const TREE_LOD1_MAX_TRIS: usize = 6_000;

/// The review gate for the whole species table at seed 0 (goldens; bless deliberately).
pub const TREE_GOLDEN_HASHES: [(TreeSpecies, u64); 6] = [
    // Blessed 2026-08-03 with the trees-to-scale pass (Oak 17.6 m, Poplar 21.9, Willow 14.5,
    // Pine 20.4 — realistic mature heights; FruitTree/Bush unchanged).
    // Oak re-blessed 2026-08-22 (Drzewa 3.0 PR6): the lobes are DEAD for the oak — the crown
    // is the card deck (~200 cluster cards on the twig anchors, shade lane carrying the
    // one-mass law), the bark meshes the whole skeleton including twigs, and the limbs grew
    // upright so the card crown clears the 15 m mature floor the spheres used to clear.
    // The whole table re-blessed 2026-08-22 with the USER'S QUALITY VERDICT: cross-pair
    // clusters (every card is two perpendicular quads — no more edge-on paper dashes),
    // clusters hugging their twigs (the levitation fix), top strays pulled into the mass,
    // and Mid keeping the LIMBS plus every second cluster — the 55 m swap moves triangles,
    // never the tree's anatomy.
    // Oak re-blessed 2026-09-02 (route 2): the AUTHORED oak — Sapling's skeleton (trunk and
    // 13 limbs as wood, 15.9 m), 161 cross pairs of Blender-rendered leaf clusters to 18.7 m.
    (TreeSpecies::Oak, 0x0ce2_6287_5c68_8c8d),
    // Poplar re-blessed 2026-08-22 (Drzewa 3.0 PR7): skeleton + cards — one bole honestly
    // grown to 19.6 m, the Column envelope, hard up-tropism.
    (TreeSpecies::Poplar, 0x1f55_74a5_e42c_7d1c),
    // Willow re-blessed 2026-08-22 (Drzewa 3.0 PR8): the sweep showcase — rising limbs arced
    // over by negative tropism, level-2 curtains falling hard, elongated hanging cards.
    (TreeSpecies::Willow, 0xae22_ee7e_22cb_cd4f),
    // FruitTree re-blessed 2026-08-22 (Drzewa 3.0 PR7): skeleton + cards — a short bole
    // opening into a low orchard dome with heavy down-angles.
    (TreeSpecies::FruitTree, 0xa198_90ab_2dcc_7cc8),
    // Bush re-blessed 2026-08-22 (Drzewa 3.0 PR8): a one-level skeleton stub fanning wide
    // into dense small cards; still honestly concealing nothing.
    // Bush re-blessed 2026-08-22 (Drzewa 3.0 PR8, final): dense one-level skeleton, deep
    // scrub shade, and the interior OCCLUSION HULL — a dense shrub shows no daylight through
    // its middle, and the steppe's rule-1 dark plane rides on that.
    (TreeSpecies::Bush, 0xf0f1_858e_6502_e441),
    // Pine re-blessed 2026-08-22 (Drzewa 3.0 PR9, the LAST migrant): a monopodial pole,
    // dense near-horizontal branches, the Cone envelope tapering them to the leader, needle
    // fronds on cards. The lobes died with this bless.
    (TreeSpecies::Pine, 0x8212_3d9a_ed9a_c6a5),
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
    // Route 2 (2026-09-02): a species with an AUTHORED tree ships that tree — grown once in
    // Blender by Sapling, exported per rung, the same individual for every seed.
    if let Some(tree) = authored::tree(species, seed, lod) {
        return tree;
    }
    // ONE path (Drzewa 3.0, complete): the skeleton grows ONCE, the rung meshes its bark and
    // deals its card deck. No entropy burn, no Mid tip-lift — every branch and card draws
    // from hashed seeds off one skeleton, so the rungs share identity by construction. The
    // lobed generator, its 13-field visual table, `tapered_tube` and the burn discipline all
    // died with wave 3 (PR9), exactly as scheduled.
    let architecture =
        species.architecture().expect("every species migrated to the skeleton in wave 3");
    let skeleton = skeleton::grow(&architecture, seed);
    BakedTree {
        species,
        trunk: bark::mesh_bark(&skeleton, lod),
        canopy: occlusion_hull(species),
        leaves: leaves::grow_cards(&skeleton, species, seed, lod),
    }
}

/// The interior occlusion hull of a migrated species — the SpeedTree move for optically
/// OPAQUE growth: a card deck is all silhouette and gaps, but a dense shrub shows no daylight
/// through its middle. A small dark mass inside the tuft gives distance rendering the
/// occluded core the cards cannot (the steppe's rule-1 dark plane rides on it); up close the
/// cards fully dress it. Tall trees keep an empty hull — their crowns honestly show sky.
fn occlusion_hull(species: TreeSpecies) -> GeometryMesh {
    match species {
        TreeSpecies::Bush => {
            let (positions, indices) = icosphere(0);
            let vertices = positions
                .iter()
                .map(|unit| {
                    let scaled = Vec3::new(unit.x * 1.62, 0.82 + unit.y * 0.86, unit.z * 1.62);
                    GeometryVertex::new(
                        scaled,
                        *unit,
                        WorldMaterial::Canopy.carrier(),
                        SmoothingGroup(1),
                    )
                })
                .collect();
            GeometryMesh::new(vertices, indices)
        }
        _ => GeometryMesh::new(Vec::new(), Vec::new()),
    }
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
        // The reference (mature) variant: the young and the sparse are meant to be shorter.
        let top =
            |species| bake_tree(species, authored::variant_seed(authored::REFERENCE_VARIANT)).tip();
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
        let a = bake_tree(TreeSpecies::Oak, authored::seed_for(0, false));
        let b = bake_tree(TreeSpecies::Oak, authored::seed_for(2, true));
        assert_ne!(a.deterministic_hash(), b.deterministic_hash(), "no two oaks alike");
        // Family: a young oak and an old one are still both oaks — the variants span about
        // 2:1 in height (route 2's "from small to big"), never a sapling next to a giant.
        let (ha, hb) = (a.tip(), b.tip());
        let ratio = ha.max(hb) / ha.min(hb);
        assert!((1.0..=2.2).contains(&ratio), "oaks stay oak-sized: {ha} vs {hb}");
    }

    // `canopy_normals_point_away_from_the_crown_centroid` died here with the lobes (wave 3):
    // the painterly one-mass law it locked lives on in the card shade lane, and its heir is
    // `leaves::tests::the_crown_core_shades_darker_than_the_rim`.

    /// Wave 3: the pine's cone is a construction — branch reach shrinks with height above
    /// the crown base, so the silhouette tapers to the leader by table, not luck.
    #[test]
    fn the_pine_cone_tapers_toward_its_leader() {
        for seed in 0..4 {
            let skeleton = skeleton::grow(&TreeSpecies::Pine.architecture().expect("wave 3"), seed);
            let mut branches: Vec<(f32, f32)> = skeleton
                .branches_of_level(1)
                .map(|branch| (branch.base().position.y, branch.length_m()))
                .collect();
            branches.sort_by(|a, b| a.0.total_cmp(&b.0));
            let third = branches.len() / 3;
            let lower: f32 =
                branches[..third].iter().map(|(_, len)| len).sum::<f32>() / third as f32;
            let upper: f32 =
                branches[branches.len() - third..].iter().map(|(_, len)| len).sum::<f32>()
                    / third as f32;
            assert!(
                lower > upper * 1.6,
                "seed {seed}: the cone must taper: lower {lower:.2} vs upper {upper:.2}"
            );
        }
    }
}
