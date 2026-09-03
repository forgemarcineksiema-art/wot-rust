//! The shelterbelt as trees (Inny Poziom F3).
//!
//! A `TreeLine` cover box is the honest LOS wall: 32–52 m long, 6 m thick, 16–22 m tall on
//! the shipped maps, and it blocks shells, hulls and eyes exactly as drawn. What DRESSED it
//! was a slab — two rows of stick boles under a run of crown-coloured boxes ("boxes on
//! sticks", the F3 register row) — while the map's own species stood fully grown twenty
//! metres away. The line is planted now: real Mid-rung trees from the map's species mix,
//! each fitted to the box it stands in, baked through the same statics path a scattered
//! poplar takes. The box does not move. What blocks the shell still blocks the eye, and
//! nothing the eye sees reaches past the wall.
//!
//! Three fits per tree, all against the box (the honesty doctrine in both directions):
//! - the tip stays under the box top (`HEADROOM`): the wall towers over its trees, so a crew
//!   is never spotted over a hedge its eyes cannot see through (Świat 2.0 PR 5's rule);
//! - the crown stays within `CROWN_OVERHANG_M` of the thin faces: foliage may lean over its
//!   wall the way a hedge does, never stand a lane away from it;
//! - the crown stays inside the ENDS: the gap between two hedgerow boxes is a door a hull
//!   drives through, and a crown that plugged it would hide an opening the map promises.
//!
//! A station whose tree cannot meet all three at `SCALE_MIN` is left unplanted; the
//! undergrowth mass (`battlefield::append_tree_line`) carries the wall at hull height either
//! way. A felled line leaves a stump at every station it was planted from — the same
//! stations, so the wreckage stands where the trees stood.
//!
//! The line is OPAQUE, because the box is. A Mid-rung crown is a deck of cards with gaps
//! between them, and the first planted lines let the eye through where the sim says a hedge
//! blocks the sight — a tank visible behind a wall it cannot be spotted through, the worst
//! direction of the honesty doctrine — and `prokhorovka_clear_afternoon` lost its dark plane
//! (0.005 → 0.004) with the slab's shade. Two answers, both measured against pictures:
//! - a station is planted with the species that FILLS its box (`FILL_MIN` of the height): a
//!   17 m wall 6 m thick is a windbreak's shape, and an oak fits it only at half size — so the
//!   drawn species stands where it fills the wall and the mix's poplar or pine where it does
//!   not. An inset dark box under the crowns was tried first and read, from 300 m, as the slab
//!   with sticks on top;
//! - every planted tree carries a CROWN HULL — an ellipsoid of shaded canopy under its card
//!   deck, the dense interior every crown has — and the stations stand `STATION_SPACING_M`
//!   apart so the hulls overlap along the run. `the_line_is_opaque_where_the_box_is` walks the
//!   run and proves a sight line through the wall meets a hull at every height the hulls span.

use std::f32::consts::TAU;

use glam::Vec3;
use renderer_api::SceneVertex;
use terrain::{BattlefieldMap, SceneryInstance, SceneryKind, StaticCoverKind, StaticCoverObject};
use world_forge::tree::{BakedTree, TreeLod, bake_tree_lod};

use crate::foliage;

/// Metres between planting stations along the run — windbreak density, so the crown hulls of
/// neighbouring stations overlap into one opaque mass: the narrowest poplar hull (reach 2.2 m
/// at a 0.87 height fit, 0.85 of it just above the hedge body) reaches 1.6 m along the run,
/// and 3.5 m stations left a half-metre window between two of them at 7 m up.
pub const STATION_SPACING_M: f32 = 3.0;
/// A station's species must reach this fraction of the box height at its fitted scale, or
/// the mix's best filler stands there instead. Lowered 0.80 → 0.70 with the authored trees
/// (route 2, 2026-09-02): their crowns are wider for their height than the procedural ones,
/// and a 6 m wall fits a mature pine only at 0.6 of its size; the hulls carry the opacity.
pub const FILL_MIN: f32 = 0.70;
/// The crown hull: bottom at this fraction of the box height...
pub const HULL_BOTTOM: f32 = 0.30;
/// ...top at this fraction of the fitted tip (under the cards' own tips)...
pub const HULL_TOP_OF_TIP: f32 = 0.88;
/// ...radius at this fraction of the fitted crown reach (the full reach: the hull is the
/// crown's body and the cards its surface)...
pub const HULL_RADIUS_OF_REACH: f32 = 1.0;
/// ...and never narrower than this: a poplar at a 0.72 height fit reaches 1.6 m, which at
/// 3 m stations left a sight line through just above the hedge body on the smallest box. A
/// columnar crown 3.6 m wide is still a poplar.
pub const HULL_MIN_RADIUS_M: f32 = 1.8;
/// ...and this much of the species' canopy tone: the shaded inside of a crown.
pub const HULL_SHADE: f32 = 0.55;
/// Stations alternate across the box by this fraction of its thin half-extent.
pub const ACROSS_STAGGER: f32 = 0.30;
/// How far a crown may lean past the box's thin faces. A hedgerow's crowns overhang its
/// trunk line; at 1.5 m no oak or willow of the mix fitted a 6 m wall at `SCALE_MIN` and
/// every line on Prokhorovka came up poplar — a monoculture hedge by the fit, not by the
/// map. At 2.5 m the widest station carries an 11 m crown over a 6 m wall.
pub const CROWN_OVERHANG_M: f32 = 2.5;
/// A tree's tip stays at or under this fraction of the box height.
pub const HEADROOM: f32 = 0.97;
/// Fitting-scale floor: a tree that needs to shrink further than this to fit is not planted.
/// Measured Mid-rung crown reach (2026-09-02, twelve seeds): oak 6.8–9.3 m, willow
/// 9.0–12.6 m, pine 4.2–4.8 m, poplar 2.2–2.8 m. Against a 6 m wall with 2.5 m of overhang
/// an oak fits at 0.5–0.65 — an 8–10 m hedgerow oak, which is what a hedgerow grows; at
/// 0.60 half the oaks fell back to pine or poplar and Mazurski's lines came up pine alone.
pub const SCALE_MIN: f32 = 0.50;
/// Fitting-scale ceiling: a small box never grows a giant to fill itself.
pub const SCALE_MAX: f32 = 1.15;
/// The ends of the run kept free of station CENTRES; the crown fit keeps the rest. At 3 m the
/// end stations' pines were end-fitted to 0.64 and filled 76 % of the wall; at 4.5 m a pine's
/// reach (4.2–4.8 m) clears the end at its full height fit.
const END_MARGIN_M: f32 = 4.5;
/// The wall is opaque from the ground to this fraction of its height at every point of the
/// run (`the_line_is_opaque_where_the_box_is`); above it the crowns' tips undulate. The LOS
/// box is opaque to its top; the last quarter, where the trees reach only with their tips,
/// is the honesty debt a wall this tall leaves to the trees that fit it.
pub const OPAQUE_TO: f32 = 0.75;
/// The hull's vertical profile: a superellipse of this exponent (2 = an ellipsoid, which
/// tapered to nothing just above the hedge body and let a sight line between two stations
/// through at 7 m; 3 keeps 0.9 of the radius at two thirds of the way to the top).
pub const HULL_EXPONENT: f32 = 3.0;
/// The mix a map without an authored horizon plants from — the backdrop ring's own fallback.
const DEFAULT_MIX: [(SceneryKind, f32); 2] =
    [(SceneryKind::Oak, 0.65), (SceneryKind::Poplar, 0.35)];

/// One planted tree of a line: exactly the instance the statics bake draws, with the
/// dimensions of the tree that bake grows (at scale 1) for the hull and the locks.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeLineStation {
    pub instance: SceneryInstance,
    /// The bake seed: names the variant that fills this wall (route 2) and the mirror.
    pub seed: u64,
    pub tip_m: f32,
    pub reach_m: f32,
}

/// The shaded interior of a planted crown: an ellipsoid under the card deck.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrownHull {
    pub center: Vec3,
    pub radius_m: f32,
    pub half_height_m: f32,
}

impl CrownHull {
    /// The hull's plan radius at a signed height fraction `t` (−1 at the bottom, 1 at the
    /// top): the superellipse profile.
    pub fn radius_at(&self, t: f32) -> f32 {
        let t = t.abs().min(1.0);
        self.radius_m * (1.0 - t.powf(HULL_EXPONENT)).max(0.0).powf(1.0 / HULL_EXPONENT)
    }

    /// Whether a sight line crossing the wall at `along` (box frame) and height `y` (world)
    /// passes through this hull: its cross-section at that height reaches it.
    pub fn covers(&self, along_center: f32, along: f32, y: f32) -> bool {
        let dy = (y - self.center.y) / self.half_height_m.max(1.0e-3);
        if dy.abs() >= 1.0 {
            return false;
        }
        (along - along_center).abs() <= self.radius_at(dy)
    }
}

/// What a hedgerow or windbreak is planted with. Orchard trees and shrubs are not.
pub fn plants_in_a_line(kind: SceneryKind) -> bool {
    matches!(kind, SceneryKind::Oak | SceneryKind::Poplar | SceneryKind::Willow | SceneryKind::Pine)
}

/// The species a map's lines are planted from: its horizon mix (`HorizonSpec::flora`, the
/// climate the ring past the border grows) minus what nobody plants in a hedgerow. One truth
/// per map for what grows on it — the wall by the road and the treeline on the horizon are
/// the same country.
pub fn tree_line_mix(battlefield: &BattlefieldMap) -> Vec<(SceneryKind, f32)> {
    let authored: Vec<(SceneryKind, f32)> = map_forge::cached_blueprint_by_id(&battlefield.id)
        .and_then(|blueprint| blueprint.horizon.as_ref())
        .map(|horizon| horizon.flora.clone())
        .unwrap_or_default();
    let mix: Vec<(SceneryKind, f32)> = authored
        .into_iter()
        .filter(|(kind, weight)| *weight > 0.0 && plants_in_a_line(*kind))
        .collect();
    if mix.is_empty() { DEFAULT_MIX.to_vec() } else { mix }
}

/// A station this close (plan, metres) to a tree the map already stands in the box is left
/// to that tree.
pub const HOSTED_KEEP_M: f32 = 4.4;

/// The plan positions of the map's own trees standing inside a line's footprint — the
/// scatter oaks a line was raised to host. Shrubs are not trees and hold no ground.
pub fn hosted_trees(battlefield: &BattlefieldMap, cover: &StaticCoverObject) -> Vec<[f32; 2]> {
    battlefield
        .scenery
        .iter()
        .filter(|instance| {
            instance.kind != SceneryKind::Bush
                && foliage::tree_species(instance.kind).is_some()
                && (instance.position[0] - cover.center[0]).abs() <= cover.half_extents_m[0]
                && (instance.position[2] - cover.center[2]).abs() <= cover.half_extents_m[2]
        })
        .map(|instance| [instance.position[0], instance.position[2]])
        .collect()
}

/// The crown's reach from the trunk axis in plan, metres at scale 1: the farthest any card
/// corner or bark vertex stands from the axis. Radial, so it holds for every yaw.
pub fn crown_reach(tree: &BakedTree) -> f32 {
    let plan = |point: Vec3| (point.x * point.x + point.z * point.z).sqrt();
    let mut reach = 0.0_f32;
    for card in &tree.leaves {
        for (right, up) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            reach = reach.max(plan(card.center + card.half_right * right + card.half_up * up));
        }
    }
    for mesh in [&tree.trunk, &tree.canopy] {
        for vertex in mesh.vertices() {
            reach = reach.max(plan(vertex.position));
        }
    }
    reach
}

/// Every station a line offers, in plan (world XZ): `STATION_SPACING_M` apart along the run
/// inside the end margins, alternating across the box. What grows there is
/// `tree_line_stations`; a station within `HOSTED_KEEP_M` of a hosted tree grows nothing.
pub fn station_plan(cover: &StaticCoverObject) -> Vec<[f32; 2]> {
    let half = Vec3::from_array(cover.half_extents_m);
    let along_x = half.x >= half.z;
    let (run, thin) = if along_x { (half.x, half.z) } else { (half.z, half.x) };
    let usable = (run - END_MARGIN_M).max(0.0);
    let count = (usable * 2.0 / STATION_SPACING_M).floor() as usize + 1;
    let step = if count > 1 { usable * 2.0 / (count - 1) as f32 } else { 0.0 };
    (0..count)
        .map(|index| {
            let along = -usable + step * index as f32;
            let across = if index % 2 == 0 { -ACROSS_STAGGER } else { ACROSS_STAGGER } * thin;
            if along_x {
                [cover.center[0] + along, cover.center[2] + across]
            } else {
                [cover.center[0] + across, cover.center[2] + along]
            }
        })
        .collect()
}

/// The planting stations of a line, fitted tree by tree. Deterministic: species and yaw
/// come from the cover id and the station index, and the tree measured for the fit is the
/// one the statics bake draws (`foliage::push_baked_tree` seeds from the position bits).
pub fn tree_line_stations(
    battlefield: &BattlefieldMap,
    cover: &StaticCoverObject,
) -> Vec<TreeLineStation> {
    if cover.kind != StaticCoverKind::TreeLine {
        return Vec::new();
    }
    let mix = tree_line_mix(battlefield);
    let total_weight: f32 = mix.iter().map(|(_, weight)| weight.max(0.0)).sum();
    let center = Vec3::from_array(cover.center);
    let half = Vec3::from_array(cover.half_extents_m);
    let ground_y = center.y - half.y;
    let box_height = half.y * 2.0;
    let along_x = half.x >= half.z;
    let (run, thin) = if along_x { (half.x, half.z) } else { (half.z, half.x) };

    // The drawn species stands where it FILLS the wall (`FILL_MIN` of the box height at its
    // fitted scale); elsewhere the mix's best filler stands — the poplar or pine that a 17 m
    // wall 6 m thick is shaped for. A line is planted with what fills it, never left half
    // empty by a species that fits the box only at half size.
    let by_weight = {
        let mut sorted = mix.clone();
        sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
        sorted
    };

    // Trees the map already stands inside this box (a scatter the line was raised to host,
    // Świat 2.0 PR 5) keep their ground: a station within most of a spacing of one is not
    // planted, so a hosted oak never wears a second crown.
    let hosted = hosted_trees(battlefield, cover);

    let mut hash = seed_from_id(&cover.id);
    let plan = station_plan(cover);
    let mut stations = Vec::with_capacity(plan.len());
    for [x, z] in plan {
        let (along, across) =
            if along_x { (x - center.x, z - center.z) } else { (z - center.z, x - center.x) };
        let drawn = crate::backdrop::weighted_kind(
            &mix,
            game_core::math::next_hash_unit(&mut hash) * total_weight,
        );
        let yaw_rad = game_core::math::next_hash_unit(&mut hash) * TAU;
        if hosted
            .iter()
            .any(|[hx, hz]| ((hx - x).powi(2) + (hz - z).powi(2)).sqrt() < HOSTED_KEEP_M)
        {
            continue;
        }
        let position = [x, ground_y, z];
        let candidates = std::iter::once(drawn)
            .chain(by_weight.iter().map(|(kind, _)| *kind).filter(|k| *k != drawn));
        // (kind, seed, fitted scale, tip, reach, fill) for every species of the mix — at
        // the VARIANT that fills this wall best (route 2: a 16 m wall is a windbreak of young
        // poplars, a 22 m one of mature; the mirror follows the position).
        let mirrored =
            world_forge::tree::authored::variant_of_seed(foliage::statics_tree_seed(position)).1;
        let mut fitted: Vec<(SceneryKind, u64, f32, f32, f32, f32)> = Vec::new();
        for kind in candidates {
            let Some(species) = foliage::tree_species(kind) else {
                continue;
            };
            let mut best: Option<(SceneryKind, u64, f32, f32, f32, f32)> = None;
            for variant in 0..world_forge::tree::authored::VARIANTS {
                let seed = world_forge::tree::authored::seed_for(variant, mirrored);
                let tree = bake_tree_lod(species, seed, TreeLod::Mid);
                let tip = tree.tip().max(0.01);
                let reach = crown_reach(&tree).max(0.01);
                let fit = (box_height * HEADROOM / tip)
                    .min((thin + CROWN_OVERHANG_M - across.abs()) / reach)
                    .min((run - along.abs()) / reach)
                    .min(SCALE_MAX);
                if fit < SCALE_MIN {
                    continue;
                }
                let fill = tip * fit / box_height;
                if best.is_none_or(|b| fill > b.5) {
                    best = Some((kind, seed, fit, tip, reach, fill));
                }
            }
            if let Some(best) = best {
                fitted.push(best);
            }
        }
        let chosen = fitted
            .first()
            .filter(|(kind, _, _, _, _, fill)| *kind == drawn && *fill >= FILL_MIN)
            .or_else(|| fitted.iter().max_by(|a, b| a.5.total_cmp(&b.5)));
        if let Some((kind, seed, scale, tip_m, reach_m, _)) = chosen.copied() {
            stations.push(TreeLineStation {
                instance: SceneryInstance { kind, position, yaw_rad, scale },
                seed,
                tip_m,
                reach_m,
            });
        }
    }
    stations
}

/// The crown hull of a station, fitted like its tree: inside the faces' overhang and the
/// ends, under the cards' tips. `None` for a crown too small to carry one.
pub fn crown_hull(station: &TreeLineStation, cover: &StaticCoverObject) -> Option<CrownHull> {
    let half = Vec3::from_array(cover.half_extents_m);
    let along_x = half.x >= half.z;
    let (run, thin) = if along_x { (half.x, half.z) } else { (half.z, half.x) };
    let box_height = half.y * 2.0;
    let [x, ground_y, z] = station.instance.position;
    let (along, across) = if along_x {
        (x - cover.center[0], z - cover.center[2])
    } else {
        (z - cover.center[2], x - cover.center[0])
    };
    let scale = station.instance.scale;
    let radius = (station.reach_m * scale * HULL_RADIUS_OF_REACH)
        .max(HULL_MIN_RADIUS_M)
        .min(thin + CROWN_OVERHANG_M - across.abs())
        .min(run - along.abs());
    let bottom = box_height * HULL_BOTTOM;
    let top = (station.tip_m * scale * HULL_TOP_OF_TIP).min(box_height * HEADROOM);
    if radius < 0.6 || top - bottom < 1.0 {
        return None;
    }
    Some(CrownHull {
        center: Vec3::new(x, ground_y + (bottom + top) * 0.5, z),
        radius_m: radius,
        half_height_m: (top - bottom) * 0.5,
    })
}

/// The hull as geometry: a low-poly ellipsoid in the species' shaded canopy tone, lit as
/// foliage like the cards over it.
fn push_crown_hull(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    hull: &CrownHull,
    species: world_forge::tree::TreeSpecies,
) {
    const RINGS: u32 = 6;
    const SEGMENTS: u32 = 10;
    let (canopy, gloss) = foliage::canopy_color_for_species(species);
    let tone = [canopy[0] * HULL_SHADE, canopy[1] * HULL_SHADE, canopy[2] * HULL_SHADE];
    let start = vertices.len() as u32;
    for ring in 0..=RINGS {
        let theta = std::f32::consts::PI * ring as f32 / RINGS as f32;
        for segment in 0..=SEGMENTS {
            let phi = TAU * segment as f32 / SEGMENTS as f32;
            // The ring's height fraction and the superellipse radius there; the normal is
            // the ellipsoid's, close enough for a shaded interior the cards half-cover.
            let unit = Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
            let ring_radius = hull.radius_at(unit.y);
            let position = hull.center
                + Vec3::new(
                    phi.cos() * ring_radius,
                    unit.y * hull.half_height_m,
                    phi.sin() * ring_radius,
                );
            let normal = Vec3::new(
                unit.x / hull.radius_m,
                unit.y / hull.half_height_m,
                unit.z / hull.radius_m,
            )
            .normalize_or_zero();
            let shade = 0.82 + 0.18 * normal.y.max(0.0);
            vertices.push(
                SceneVertex::surfaced(
                    position.to_array(),
                    normal.to_array(),
                    [tone[0] * shade, tone[1] * shade, tone[2] * shade],
                    gloss,
                )
                .with_surface(renderer_api::surface_role::FOLIAGE),
            );
        }
    }
    let stride = SEGMENTS + 1;
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let a = start + ring * stride + segment;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
}

/// Bake the line's trees into the statics mesh — the intact state's crowns and boles.
pub fn push_tree_line_trees(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    battlefield: &BattlefieldMap,
    cover: &StaticCoverObject,
) {
    for station in tree_line_stations(battlefield, cover) {
        if let (Some(hull), Some(species)) =
            (crown_hull(&station, cover), foliage::tree_species(station.instance.kind))
        {
            push_crown_hull(vertices, indices, &hull, species);
        }
        foliage::push_baked_tree_seeded(vertices, indices, &station.instance, station.seed);
    }
}

/// FNV-1a over the cover id: the same hash the slab and the wreckage seeded from, so a hedge
/// keeps its identity across states.
fn seed_from_id(id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain::MapId;

    /// The box in its own frame: `(along, across, height above the box floor)`.
    fn box_frame(cover: &StaticCoverObject, point: [f32; 3]) -> (f32, f32, f32) {
        let along_x = cover.half_extents_m[0] >= cover.half_extents_m[2];
        let dx = point[0] - cover.center[0];
        let dz = point[2] - cover.center[2];
        let floor = cover.center[1] - cover.half_extents_m[1];
        if along_x { (dx, dz, point[1] - floor) } else { (dz, dx, point[1] - floor) }
    }

    fn lines(map: &BattlefieldMap) -> Vec<&StaticCoverObject> {
        map.static_cover.iter().filter(|cover| cover.kind == StaticCoverKind::TreeLine).collect()
    }

    /// The whole promise of F3, measured on every line of every shipped map: the wall is
    /// planted (most stations grow a tree), and no vertex of any tree leaves the box — under
    /// the top, inside the ends, within the overhang of the thin faces.
    #[test]
    fn every_line_on_every_shipped_map_is_planted_inside_its_box() {
        for id in MapId::SHIPPED {
            let map = map_forge::battlefield(*id);
            let covers = lines(&map);
            assert!(!covers.is_empty(), "{id:?} ships tree lines");
            for cover in covers {
                let stations = tree_line_stations(&map, cover);
                let along_x = cover.half_extents_m[0] >= cover.half_extents_m[2];
                let run = if along_x { cover.half_extents_m[0] } else { cover.half_extents_m[2] };
                let thin = if along_x { cover.half_extents_m[2] } else { cover.half_extents_m[0] };
                // Offered = the plan minus the ground the hosted trees keep.
                let hosted = hosted_trees(&map, cover);
                let offered = station_plan(cover)
                    .into_iter()
                    .filter(|[x, z]| {
                        !hosted.iter().any(|[hx, hz]| {
                            ((hx - x).powi(2) + (hz - z).powi(2)).sqrt() < HOSTED_KEEP_M
                        })
                    })
                    .count();
                assert!(
                    stations.len() * 10 >= offered * 6,
                    "{id:?} {}: {} of {offered} offered stations planted ({} hosted) — the wall reads bare",
                    cover.id,
                    stations.len(),
                    hosted.len()
                );
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                push_tree_line_trees(&mut vertices, &mut indices, &map, cover);
                assert!(!vertices.is_empty(), "{id:?} {}: a planted line draws", cover.id);
                let box_height = cover.half_extents_m[1] * 2.0;
                for vertex in &vertices {
                    let (along, across, height) = box_frame(cover, vertex.position);
                    assert!(
                        height <= box_height + 0.05,
                        "{id:?} {}: a crown at {height:.2} m tops the {box_height:.1} m wall",
                        cover.id
                    );
                    assert!(
                        along.abs() <= run + 0.05,
                        "{id:?} {}: a crown reaches {along:.2} m past the end of a {run:.0} m half-run — it plugs a door",
                        cover.id
                    );
                    assert!(
                        across.abs() <= thin + CROWN_OVERHANG_M + 0.05,
                        "{id:?} {}: a crown stands {across:.2} m off the axis of a {thin:.1} m half-thick wall",
                        cover.id
                    );
                }
            }
        }
    }

    /// The line grows the map's own trees — the species its horizon names — and every station
    /// grows one that FILLS the wall: a 17 m box 6 m thick is a windbreak's shape, and the
    /// species that fills it (poplar on the steppe and in the valley, pine on the pass and the
    /// isthmus) is what stands there. An oak that fits such a wall only at half size does not;
    /// the first fit rule planted it and left half the wall empty.
    #[test]
    fn a_line_is_planted_from_its_maps_own_species_and_every_tree_fills_the_wall() {
        for id in MapId::SHIPPED {
            let map = map_forge::battlefield(*id);
            let mix: Vec<SceneryKind> =
                tree_line_mix(&map).into_iter().map(|(kind, _)| kind).collect();
            assert!(mix.iter().all(|kind| plants_in_a_line(*kind)), "{id:?}: {mix:?}");
            for cover in lines(&map) {
                let box_height = cover.half_extents_m[1] * 2.0;
                for station in tree_line_stations(&map, cover) {
                    assert!(
                        mix.contains(&station.instance.kind),
                        "{id:?} {}: {:?} is not in the map's mix {mix:?}",
                        cover.id,
                        station.instance.kind
                    );
                    let fill = station.tip_m * station.instance.scale / box_height;
                    // A station squeezed against the wall's END fits the end, not the wall:
                    // its crown must stay inside the box (the door between two boxes stays a
                    // door), so the fill rule holds where there is room for a crown.
                    let half = cover.half_extents_m;
                    let along_x = half[0] >= half[2];
                    let (run, along) = if along_x {
                        (half[0], station.instance.position[0] - cover.center[0])
                    } else {
                        (half[2], station.instance.position[2] - cover.center[2])
                    };
                    let end_room = run - along.abs();
                    assert!(
                        fill >= FILL_MIN || end_room < station.reach_m * SCALE_MAX,
                        "{id:?} {}: a {:?} at scale {:.2} fills {:.0} % of a {box_height:.1} m wall",
                        cover.id,
                        station.instance.kind,
                        station.instance.scale,
                        fill * 100.0
                    );
                }
            }
        }
    }

    /// The wall is opaque where the box is. Walk the run at half-metre steps: at every metre
    /// of height from the hedge body's top to `OPAQUE_TO` of the wall, a sight line crossing
    /// the wall meets a crown hull — or stands within reach of a tree the map hosts in the
    /// box. Below the hulls the hedge body is one solid mass by construction.
    #[test]
    fn the_line_is_opaque_where_the_box_is() {
        for id in MapId::SHIPPED {
            let map = map_forge::battlefield(*id);
            for cover in lines(&map) {
                let stations = tree_line_stations(&map, cover);
                let hulls: Vec<(f32, CrownHull)> = stations
                    .iter()
                    .filter_map(|station| {
                        crown_hull(station, cover)
                            .map(|hull| (box_frame(cover, station.instance.position).0, hull))
                    })
                    .collect();
                assert!(!hulls.is_empty(), "{id:?} {}: a planted line has hulls", cover.id);
                let hosted = hosted_trees(&map, cover);
                let along_x = cover.half_extents_m[0] >= cover.half_extents_m[2];
                let run = if along_x { cover.half_extents_m[0] } else { cover.half_extents_m[2] };
                let floor = cover.center[1] - cover.half_extents_m[1];
                let box_height = cover.half_extents_m[1] * 2.0;
                let lowest_top = hulls
                    .iter()
                    .map(|(_, hull)| hull.center.y + hull.half_height_m)
                    .fold(f32::MAX, f32::min);
                let usable = (run - END_MARGIN_M).max(0.0);
                let mut along = -usable;
                while along <= usable {
                    let (x, z) = if along_x {
                        (cover.center[0] + along, cover.center[2])
                    } else {
                        (cover.center[0], cover.center[2] + along)
                    };
                    let near_hosted = hosted.iter().any(|[hx, hz]| {
                        ((hx - x).powi(2) + (hz - z).powi(2)).sqrt() < HOSTED_KEEP_M
                    });
                    // From just above the hedge body (one solid mass) to `OPAQUE_TO` of the
                    // wall, and never past the shortest hull's top.
                    let body_top =
                        (box_height * crate::battlefield::TREE_LINE_BODY_HEIGHT).clamp(2.6, 9.0);
                    let ceiling = (floor + box_height * OPAQUE_TO).min(lowest_top);
                    let mut y = floor + body_top + 0.5;
                    while y < ceiling - 0.5 {
                        let covered = near_hosted
                            || hulls.iter().any(|(center, hull)| hull.covers(*center, along, y));
                        assert!(
                            covered,
                            "{id:?} {}: a sight line through the wall at {along:.1} m along, {:.1} m up, meets no crown — the eye sees what the box hides",
                            cover.id,
                            y - floor
                        );
                        y += 1.0;
                    }
                    along += 0.5;
                }
            }
        }
    }

    /// The planted lines are a statics cost, measured per map and capped with slack. The
    /// numbers print on every run; raising the cap is a measured decision, never a tuning
    /// accident. Measured 2026-09-02 (Mid rung + crown hull, ~1 090 tris a tree): at 5.5 m
    /// stations Bystra 58 trees / 56 k tris over eight lines, Prokhorovka 30 / 30 k over four;
    /// at windbreak density (3.0 m, shipped, 4.5 m end margins, hulls): Bystra 96 / 102 k,
    /// Mazurski 58 / 106 k (a pine is ~1.8 k), Ostrogorsk 38 / 41 k, Prokhorovka 37 / 40 k,
    /// Orliny 22 / 40 k.
    #[test]
    fn the_planted_lines_stay_under_their_triangle_budget() {
        // Raised 120 k → 260 k with the authored stations (route 2): a Mid-rung authored
        // tree is 1–2 k of wood plus its deck; Bystra's 96 stations measure ~206 k. The frame
        // cost of the lines is F7b's measurement (the stations onto the ladder).
        // Raised again 260 k → 420 k on 2026-09-03: the Mid rung carries the Near deck now.
        const MAP_LINES_MAX_TRIS: usize = 420_000;
        for id in MapId::SHIPPED {
            let map = map_forge::battlefield(*id);
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut stations = 0usize;
            for cover in lines(&map) {
                stations += tree_line_stations(&map, cover).len();
                push_tree_line_trees(&mut vertices, &mut indices, &map, cover);
            }
            let tris = indices.len() / 3;
            println!(
                "TREE LINES {id:?}: {stations} trees, {tris} tris, {} vertices",
                vertices.len()
            );
            assert!(
                tris <= MAP_LINES_MAX_TRIS,
                "{id:?}: the planted lines cost {tris} tris (cap {MAP_LINES_MAX_TRIS})"
            );
        }
    }

    /// Same cover, same trees, every time — the scene bake's determinism rides on it (the
    /// partial-rebake locks compare buckets bit for bit).
    #[test]
    fn a_line_grows_the_same_trees_every_bake() {
        let map = map_forge::battlefield(MapId::BystraValley);
        for cover in lines(&map) {
            assert_eq!(tree_line_stations(&map, cover), tree_line_stations(&map, cover));
        }
    }
}
