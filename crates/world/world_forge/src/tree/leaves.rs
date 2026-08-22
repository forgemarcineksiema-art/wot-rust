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

/// Cards per anchor at the Close rung. Mid keeps every [`MID_KEEP_EVERY`]-th CARD, scaled by
/// `sqrt(MID_KEEP_EVERY)` so the crown's covered AREA survives the thinning — a rung swap
/// thins the deck, never balds the tree.
const CARDS_PER_ANCHOR: u32 = 3;
const MID_KEEP_EVERY: usize = 3;

/// How much of the crown's radial depth the shade lane spans: rim cards at 1.0, core cards
/// down to this floor. The heir of the centroid-normal trick — locked by the shade-mass test.
const CORE_SHADE: f32 = 0.68;

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
    // means the rungs share every card-level decision by construction.
    let mut cards = Vec::new();
    for (anchor_index, anchor) in anchors.iter().enumerate() {
        for burst in 0..CARDS_PER_ANCHOR {
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
            // stem hangs from the twig), `right` completes the frame.
            let reference = if normal.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
            let right = normal.cross(reference).normalize_or_zero();
            let up = right.cross(normal);
            let roll = rng.unit() * std::f32::consts::TAU;
            let (sin, cos) = roll.sin_cos();
            let spun_right = right * cos + up * sin;
            let spun_up = up * cos - right * sin;

            // Wide size variance on purpose: a canopy of same-size quads reads as confetti;
            // mixed clusters read as growth.
            let half = card_half_extent_m(species) * (0.72 + 0.56 * rng.unit());
            // The cluster sits a little OFF its twig along the facing, so cards ring the wood
            // instead of slicing through it.
            let center = anchor.position + normal * half * 0.35;
            let depth01 = (anchor.position.distance(centroid) / max_reach).clamp(0.0, 1.0);
            cards.push(LeafCard {
                center,
                half_right: spun_right * half,
                half_up: spun_up * half * 0.92,
                normal,
                slot: slots[(rng.next() % 2) as usize],
                shade: CORE_SHADE + (1.0 - CORE_SHADE) * depth01,
            });
        }
    }
    if lod == TreeLod::Mid {
        thin_for_mid(&mut cards);
    }
    cards
}

/// Mid keeps every [`MID_KEEP_EVERY`]-th card of the SAME deck, each survivor scaled by
/// `sqrt(keep)` so the covered area survives — except where that growth would raise the
/// crown's tip: the tip is the ladder's invariant (a rung swap moves triangles, never
/// metres), so a top card only grows into the headroom the Close deck actually had.
fn thin_for_mid(cards: &mut Vec<LeafCard>) {
    let card_top = |card: &LeafCard| card.center.y + card.half_right.y.abs() + card.half_up.y.abs();
    let close_tip = cards.iter().map(card_top).fold(0.0_f32, f32::max);
    // The card that DEFINES the tip always survives the thinning — without it the Mid crown
    // is shorter than Close by whatever the tallest dropped card carried.
    let tip_index = cards
        .iter()
        .enumerate()
        .max_by(|a, b| card_top(a.1).total_cmp(&card_top(b.1)))
        .map(|(index, _)| index);
    let scale = (MID_KEEP_EVERY as f32).sqrt();
    let mut ordinal = 0usize;
    cards.retain(|_| {
        let keep = ordinal.is_multiple_of(MID_KEEP_EVERY) || Some(ordinal) == tip_index;
        ordinal += 1;
        keep
    });
    for card in cards.iter_mut() {
        let span = card.half_right.y.abs() + card.half_up.y.abs();
        let headroom = (close_tip - card.center.y).max(span);
        let fit = (headroom / span).clamp(1.0, scale);
        card.half_right *= fit;
        card.half_up *= fit;
    }
}

/// Species card half-extent, metres: the cluster's reach, not a single leaf's.
fn card_half_extent_m(species: TreeSpecies) -> f32 {
    match species {
        TreeSpecies::Oak => 0.72,
        TreeSpecies::Poplar => 0.48,
        TreeSpecies::Willow => 0.55,
        TreeSpecies::FruitTree => 0.38,
        TreeSpecies::Bush => 0.34,
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
        let expected = close.len().div_ceil(MID_KEEP_EVERY);
        assert!(
            mid.len() == expected || mid.len() == expected + 1,
            "Mid keeps every {MID_KEEP_EVERY}th card plus the tip card: {} of {}",
            mid.len(),
            close.len()
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
