//! Leaf cards (Drzewa 3.0 PR6): the canopy as CLUSTER CARDS — oriented quads carrying an
//! atlas mask, the SpeedTree move. A card is not a leaf; it is 0.8–1.2 m of twig-with-leaves,
//! so a crown is a few hundred alpha shapes, never ten thousand triangles of geometry.
//!
//! Cards are pure DATA out of this crate: `GeometryVertex` has no UV lane and `world_forge`
//! may not know the renderer, so `scene_build` expands each card into four `SceneVertex`
//! corners (dual winding — backface culling stays on and never eats the far side).
//!
//! The painterly law, transplanted: the lobed crown lit as one mass through centroid-bent
//! normals; cards carry the same idea in the `shade` lane — the crown CORE darkens, the rim
//! stays lit, so hundreds of separate quads still read as one volume in the sun.

use glam::Vec3;

use super::skeleton::TreeSkeleton;
use super::{TreeLod, TreeSpecies, leaf_atlas};
use crate::shape::Rng;

/// One canopy cluster card, in tree-local metres. `half_right`/`half_up` span the quad from
/// its `center`; `normal` is the lighting normal the whole card carries (the atlas dome page
/// curves it per-texel in the shader); `slot` picks the atlas mask; `shade` multiplies the
/// authored canopy color (the one-mass law).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeafCard {
    pub center: Vec3,
    pub half_right: Vec3,
    pub half_up: Vec3,
    pub normal: Vec3,
    pub slot: u8,
    pub shade: f32,
}

/// CLUSTERS per anchor at the Close rung — and every cluster is a CROSS-PAIR: two
/// perpendicular quads sharing one center, slot and shade (the user's verdict, 2026-08-22:
/// a single fixed quad reads as a levitating paper dash the moment the camera catches it
/// edge-on; a cross reads as foliage from every angle, which is also how the impostor
/// already works). Mid keeps every [`MID_KEEP_EVERY`]-th CLUSTER, scaled by
/// `sqrt(MID_KEEP_EVERY)` so the crown's covered AREA survives the thinning — a rung swap
/// thins the deck, never balds the tree.
const CLUSTERS_PER_ANCHOR: u32 = 2;
const MID_KEEP_EVERY: usize = 2;

/// How far off its twig a cluster's center sits, as a fraction of its half-extent. The
/// levitation fix: 0.35 pushed whole clusters visibly clear of their wood; a cluster now
/// HUGS the twig and the cross-pair supplies the volume the offset used to fake.
const CLUSTER_OFFSET: f32 = 0.12;

/// Anchors in the top of the crown pull their clusters inward by this much (horizontally —
/// the tip height is the ladder's invariant and stays untouched): the topmost twigs are the
/// loneliest, and a single cluster hanging off one of them reads as a detached leaf.
const TOP_STRAY_PULL: f32 = 0.35;

/// How much of the crown's radial depth the shade lane spans: rim cards at 1.0, core cards
/// down to this floor. The heir of the centroid-normal trick — locked by the shade-mass test.
const CORE_SHADE: f32 = 0.68;

/// The bush goes DEEPER: the steppe's overcast value structure leans on bushes for its dark
/// plane (rule 1), and the wrapped-diffuse foliage model floors how dark a card can light —
/// the lobed blob's occluded underside must come back through the shade lane instead.
fn core_shade(species: TreeSpecies) -> f32 {
    match species {
        TreeSpecies::Bush => 0.38,
        _ => CORE_SHADE,
    }
}

/// The rim end of the shade span. A tree crown's rim catches full light; scrub is matte and
/// light-eating to its very edge — its rim cap sits under the tree's, which is what keeps a
/// card tuft as dark on the steppe as the solid blob it replaced.
fn rim_shade(species: TreeSpecies) -> f32 {
    match species {
        TreeSpecies::Bush => 0.72,
        _ => 1.0,
    }
}

/// Grow the card canopy for one rung. Deterministic per (skeleton, seed): every card draws
/// from a seed hashed off its anchor ordinal, so no card's look depends on its neighbours.
pub(crate) fn grow_cards(
    skeleton: &TreeSkeleton,
    species: TreeSpecies,
    seed: u64,
    lod: TreeLod,
) -> Vec<LeafCard> {
    let anchors = &skeleton.anchors;
    if anchors.is_empty() {
        return Vec::new();
    }
    let centroid =
        anchors.iter().map(|anchor| anchor.position).sum::<Vec3>() / anchors.len() as f32;
    let max_reach = anchors
        .iter()
        .map(|anchor| anchor.position.distance(centroid))
        .fold(0.0_f32, f32::max)
        .max(0.01);
    let slots = leaf_atlas::species_slots(species);

    // The FULL deck grows first, always — Mid then filters and rescales it. One growth path
    // means the rungs share every card-level decision by construction. Clusters are grown as
    // PAIRS (the two quads of one cross), so the Mid thinning drops crosses whole and never
    // strands half a cluster.
    let crown_top = anchors.iter().map(|anchor| anchor.position.y).fold(f32::MIN, f32::max);
    let crown_base = anchors.iter().map(|anchor| anchor.position.y).fold(f32::MAX, f32::min);
    let mut clusters: Vec<[LeafCard; 2]> = Vec::new();
    for (anchor_index, anchor) in anchors.iter().enumerate() {
        for burst in 0..CLUSTERS_PER_ANCHOR {
            let mut rng = Rng(seed
                ^ 0xCA4D_0000
                ^ ((anchor_index as u64) << 20)
                ^ ((u64::from(anchor.branch)) << 8)
                ^ u64::from(burst));
            // Facing: outward from the crown centroid, blended toward the twig's own growth —
            // a cluster grows off its twig toward the light, and ~a quarter of the cards flatten
            // toward horizontal so the canopy reads in layers from below as well as in profile.
            let radial = (anchor.position - centroid).normalize_or_zero();
            let mut normal = (radial * 0.6 + anchor.tangent * 0.4).normalize_or_zero();
            if normal.length_squared() < 0.5 {
                normal = Vec3::Y;
            }
            if rng.unit() < 0.25 {
                normal = (normal * 0.35 + Vec3::Y * 0.65).normalize_or_zero();
            }
            // The card plane: `up` leans along world-up projected into the plane (the cluster
            // stem hangs from the twig), `right` completes the frame. The willow OVERRIDES
            // the roll: its cards are CURTAINS — strongly elongated down their hang, pinned
            // vertical, root at the top of the slot — a weeping crown is streamers, never
            // confetti.
            let (right_scale, up_scale, hangs) = match species {
                TreeSpecies::Willow => (0.5, 2.1, true),
                _ => (1.0, 0.92, false),
            };
            let reference = if normal.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
            let (spun_right, spun_up) = if hangs {
                let up = (Vec3::Y - normal * normal.y).normalize_or_zero();
                let up = if up.length_squared() < 0.5 { Vec3::Y } else { up };
                (up.cross(normal).normalize_or_zero(), up)
            } else {
                let right = normal.cross(reference).normalize_or_zero();
                let up = right.cross(normal);
                let roll = rng.unit() * std::f32::consts::TAU;
                let (sin, cos) = roll.sin_cos();
                (right * cos + up * sin, up * cos - right * sin)
            };

            // Wide size variance on purpose: a canopy of same-size quads reads as confetti;
            // mixed clusters read as growth.
            let half = card_half_extent_m(species) * (0.72 + 0.56 * rng.unit());
            // The cluster HUGS its twig (the levitation fix) instead of floating off it.
            let mut center = anchor.position + normal * half * CLUSTER_OFFSET;
            // The topmost twigs pull their clusters inward, horizontally only — a lone
            // cluster on the crown's highest sprig read as a detached leaf, and the tip
            // height must not move (the ladder's invariant).
            let crown_span = (crown_top - crown_base).max(0.01);
            if (anchor.position.y - crown_base) / crown_span > 0.85 {
                center.x += (centroid.x - center.x) * TOP_STRAY_PULL;
                center.z += (centroid.z - center.z) * TOP_STRAY_PULL;
            }
            // No card digs into the soil: a low tuft may kiss the ground (25 cm of embed
            // reads as growth), never bury a metre of its mask under the terrain.
            let dip = center.y - (half * up_scale + half * right_scale);
            if dip < -0.25 {
                center.y += -0.25 - dip;
            }
            let depth01 = (anchor.position.distance(centroid) / max_reach).clamp(0.0, 1.0);
            let slot = slots[(rng.next() % 2) as usize];
            let shade = core_shade(species) + (rim_shade(species) - core_shade(species)) * depth01;
            // The cross-pair: quad A faces the cluster's growth; quad B stands in the
            // perpendicular plane (the old facing becomes its span, the old right its
            // facing), sharing center, mask, shade and — downstream — the wind personality
            // keyed off the shared center. From any angle at least one quad shows its face.
            clusters.push([
                LeafCard {
                    center,
                    half_right: spun_right * half * right_scale,
                    half_up: spun_up * half * up_scale,
                    normal,
                    slot,
                    shade,
                },
                LeafCard {
                    center,
                    half_right: normal * half * right_scale,
                    half_up: spun_up * half * up_scale,
                    normal: spun_right,
                    slot,
                    shade,
                },
            ]);
        }
    }
    if lod == TreeLod::Mid {
        thin_for_mid(&mut clusters);
    }
    clusters.into_iter().flatten().collect()
}

/// Mid keeps every [`MID_KEEP_EVERY`]-th CLUSTER of the SAME deck (crosses stay whole — a
/// stranded half-cluster would bring the edge-on dash right back), each survivor scaled by
/// `sqrt(keep)` so the covered area survives — except where that growth would raise the
/// crown's tip: the tip is the ladder's invariant (a rung swap moves triangles, never
/// metres), so a top cluster only grows into the headroom the Close deck actually had.
fn thin_for_mid(clusters: &mut Vec<[LeafCard; 2]>) {
    let card_top = |card: &LeafCard| card.center.y + card.half_right.y.abs() + card.half_up.y.abs();
    let cluster_top = |cluster: &[LeafCard; 2]| card_top(&cluster[0]).max(card_top(&cluster[1]));
    let close_tip = clusters.iter().map(cluster_top).fold(0.0_f32, f32::max);
    // The cluster that DEFINES the tip always survives the thinning — without it the Mid
    // crown is shorter than Close by whatever the tallest dropped cluster carried.
    let tip_index = clusters
        .iter()
        .enumerate()
        .max_by(|a, b| cluster_top(a.1).total_cmp(&cluster_top(b.1)))
        .map(|(index, _)| index);
    let scale = (MID_KEEP_EVERY as f32).sqrt();
    let mut ordinal = 0usize;
    clusters.retain(|_| {
        let keep = ordinal.is_multiple_of(MID_KEEP_EVERY) || Some(ordinal) == tip_index;
        ordinal += 1;
        keep
    });
    for cluster in clusters.iter_mut() {
        for card in cluster.iter_mut() {
            let span = card.half_right.y.abs() + card.half_up.y.abs();
            let headroom = (close_tip - card.center.y).max(span);
            let fit = (headroom / span).clamp(1.0, scale);
            card.half_right *= fit;
            card.half_up *= fit;
        }
    }
}

/// Species card half-extent, metres: the cluster's reach, not a single leaf's.
fn card_half_extent_m(species: TreeSpecies) -> f32 {
    match species {
        TreeSpecies::Oak => 0.72,
        TreeSpecies::Poplar => 0.48,
        TreeSpecies::Willow => 0.55,
        TreeSpecies::FruitTree => 0.38,
        // Big enough that the tuft keeps the DARK MASS the lobed blob gave the steppe's
        // value structure (rule 1's dark plane rides partly on bushes in the overcast frames).
        TreeSpecies::Bush => 0.46,
        TreeSpecies::Pine => 0.52,
    }
}

#[cfg(test)]
mod tests {
    use super::super::skeleton::grow;
    use super::*;

    fn oak_cards(lod: TreeLod) -> Vec<LeafCard> {
        let architecture = TreeSpecies::Oak.architecture().expect("oak is branched");
        grow_cards(&grow(&architecture, 0), TreeSpecies::Oak, 0, lod)
    }

    /// The deck is deterministic and the Mid rung thins it by exactly the keep-rate, with the
    /// sqrt size compensation that preserves covered area.
    #[test]
    fn the_deck_is_deterministic_and_mid_thins_with_area_compensation() {
        let close = oak_cards(TreeLod::Close);
        let mid = oak_cards(TreeLod::Mid);
        assert_eq!(close, oak_cards(TreeLod::Close), "cards bake deterministic");
        assert!(
            close.len().is_multiple_of(2) && mid.len().is_multiple_of(2),
            "the deck is whole cross-pairs: {} close, {} mid",
            close.len(),
            mid.len()
        );
        let close_clusters = close.len() / 2;
        let expected = close_clusters.div_ceil(MID_KEEP_EVERY);
        let mid_clusters = mid.len() / 2;
        assert!(
            mid_clusters == expected || mid_clusters == expected + 1,
            "Mid keeps every {MID_KEEP_EVERY}th cluster plus the tip cluster: {mid_clusters} of \
             {close_clusters}"
        );
        let area = |cards: &[LeafCard]| -> f32 {
            cards.iter().map(|c| c.half_right.length() * c.half_up.length() * 4.0).sum()
        };
        let ratio = area(&mid) / area(&close);
        assert!(
            (0.75..=1.30).contains(&ratio),
            "thinned deck keeps its covered area: ratio {ratio:.2}"
        );
    }

    /// The one-mass law's heir: core cards darker than rim cards, by construction and by band.
    #[test]
    fn the_crown_core_shades_darker_than_the_rim() {
        let cards = oak_cards(TreeLod::Close);
        let centroid = cards.iter().map(|c| c.center).sum::<Vec3>() / cards.len() as f32;
        let mut by_depth: Vec<(f32, f32)> =
            cards.iter().map(|c| (c.center.distance(centroid), c.shade)).collect();
        by_depth.sort_by(|a, b| a.0.total_cmp(&b.0));
        let third = by_depth.len() / 3;
        let inner: f32 = by_depth[..third].iter().map(|(_, s)| s).sum::<f32>() / third as f32;
        let outer: f32 =
            by_depth[by_depth.len() - third..].iter().map(|(_, s)| s).sum::<f32>() / third as f32;
        assert!(
            inner + 0.08 < outer,
            "the crown must read as one lit mass: inner {inner:.2} vs outer {outer:.2}"
        );
        assert!(cards.iter().all(|c| (CORE_SHADE..=1.0).contains(&c.shade)));
    }

    /// Cards are honest quads: orthogonal spans, unit normals, cluster-scale extents, and both
    /// authored atlas slots of the species in play.
    #[test]
    fn cards_are_well_formed_and_use_both_species_slots() {
        let cards = oak_cards(TreeLod::Close);
        assert!(cards.len() >= 120, "a mature oak deals a real deck: {}", cards.len());
        let slots = leaf_atlas::species_slots(TreeSpecies::Oak);
        for card in &cards {
            assert!(card.half_right.dot(card.half_up).abs() < 1.0e-3, "spans stay orthogonal");
            assert!((card.normal.length() - 1.0).abs() < 1.0e-3);
            let reach = card.half_right.length();
            assert!((0.3..=1.4).contains(&reach), "cluster-scale card: {reach}");
            assert!(slots.contains(&card.slot));
        }
        assert!(cards.iter().any(|c| c.slot == slots[0]));
        assert!(cards.iter().any(|c| c.slot == slots[1]));
    }
}
