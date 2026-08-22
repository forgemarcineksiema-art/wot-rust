//! Instanced battlefield trees with runtime LOD (hero-flora phase 2, retargeted to
//! procedural species in Świat 2.0).
//!
//! Trees left the statics bake: a ~1k-tri oak baked once per instance into the shared vertex
//! buffer meant every copy paid full price in every pass, and the min-spec measurement put the
//! ceiling at ten trees a map. As registered meshes they cost ONE upload and a matrix per
//! instance — and, more importantly, the copy the camera sees at 200 m can be a different mesh
//! from the one it sees at 20 m.
//!
//! Three rungs of ONE procedural oak: the near mesh (full LOD0 bake), a sparse mid mesh
//! (LOD1), and a painted frustum impostor. Each rung stands the same height by construction
//! (one species, one parameter set), so a swap moves texels, never the tree's size.

use glam::{Mat4, Quat, Vec3};
use renderer_api::{MaterialHandle, MeshAsset, MeshHandle, RenderObject};
use terrain::{SceneryInstance, SceneryKind};
use world_forge::tree::{TreeLod as BakeLod, TreeSpecies};

/// Mesh handles for the three rungs. They sit BELOW [`renderer_api::SHADOWLESS_DRESSING_MESH_BASE`]
/// on purpose: grass may skip the depth passes, a tree may not — its shadow is half of what a
/// tree contributes to a battlefield.
pub const TREE_NEAR_MESH: MeshHandle = MeshHandle(0xFEE0_0001);
pub const TREE_MID_MESH: MeshHandle = MeshHandle(0xFEE0_0002);
pub const TREE_IMPOSTOR_MESH: MeshHandle = MeshHandle(0xFEE0_0003);

const _: () = assert!(TREE_IMPOSTOR_MESH.0 < renderer_api::SHADOWLESS_DRESSING_MESH_BASE);

/// The procedural species behind the battlefield trees. One species ships the instanced
/// ladder; the rarer kinds (poplar/willow/pine) stay in the statics bake where their counts
/// are small enough not to need it.
pub const BATTLE_TREE: TreeSpecies = TreeSpecies::Oak;

/// Rung boundaries in metres, and the band a tree must re-cross before it swaps back. Without
/// the hysteresis a tree parked exactly on a boundary would flicker between two meshes as the
/// hull idles; 8 m is wider than any camera jitter and far narrower than a deliberate approach.
pub const NEAR_MAX_M: f32 = 55.0;
pub const MID_MAX_M: f32 = 150.0;
pub const HYSTERESIS_M: f32 = 8.0;

/// How deep a trunk is set into the ground it stands on, metres.
///
/// A tree planted at the sampled terrain height stands exactly ON the surface, and the moment
/// the ground tilts, the downhill side of the butt lifts clear and the tree reads as a prop
/// set down on the field. Real trunks meet soil. Sinking a third of a metre keeps the butt in
/// contact across the terrain's ordinary grade.
///
/// Applied identically to all three rungs, so a LOD swap never shifts a tree vertically. The
/// trunk's cover box is deliberately NOT moved with it: that column blocks from the ground
/// line up, and the part being buried here is the part below it.
const TRUNK_SINK_M: f32 = 0.35;

/// A reference seed for the registered rung meshes. Per-instance variety rides the instance
/// matrix (yaw/scale), and the canopy's deterministic limb/lobe phases come from the species
/// bake; the ladder ships one representative individual per rung so every copy on the map
/// shares the silhouette the species table promises.
const RUNG_SEED: u64 = 0xDAB_0001;

/// The rendered canopy tip of one instanced battle tree, metres above the instance's map
/// position: the rung mesh's tip at the instance scale, minus the trunk sink. The number a
/// TreeLine collision box must tower over to honestly contain the oaks it hosts (PR 5).
/// Test-only: the honesty lock `tree_line_boxes_contain_the_trees_they_host` is its caller.
#[cfg(test)]
pub(crate) fn battle_tree_rendered_top_m(scale: f32) -> f32 {
    // `tip()` covers every crown representation — lobes then, the card deck now (PR6): the
    // TreeLine box must tower over whatever actually draws.
    let tip = world_forge::tree::bake_tree_lod(BATTLE_TREE, RUNG_SEED, BakeLod::Close).tip();
    tip * scale - TRUNK_SINK_M
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeLod {
    Near,
    Mid,
    Impostor,
}

impl TreeLod {
    pub fn mesh(self) -> MeshHandle {
        match self {
            Self::Near => TREE_NEAR_MESH,
            Self::Mid => TREE_MID_MESH,
            Self::Impostor => TREE_IMPOSTOR_MESH,
        }
    }
}

/// Pick a rung for one tree. `previous` is the rung it drew last frame; passing `None` (first
/// sight) takes the plain bands. The asymmetric thresholds are the whole point: a tree only
/// coarsens once it is [`HYSTERESIS_M`] BEYOND the boundary, and only refines once it is that
/// far inside — so a boundary crossing is a single event, not a per-frame coin flip.
pub fn select_lod(distance_m: f32, previous: Option<TreeLod>) -> TreeLod {
    let (near_edge, mid_edge) = match previous {
        Some(TreeLod::Near) => (NEAR_MAX_M + HYSTERESIS_M, MID_MAX_M + HYSTERESIS_M),
        Some(TreeLod::Mid) => (NEAR_MAX_M - HYSTERESIS_M, MID_MAX_M + HYSTERESIS_M),
        Some(TreeLod::Impostor) => (NEAR_MAX_M - HYSTERESIS_M, MID_MAX_M - HYSTERESIS_M),
        None => (NEAR_MAX_M, MID_MAX_M),
    };
    if distance_m <= near_edge {
        TreeLod::Near
    } else if distance_m <= mid_edge {
        TreeLod::Mid
    } else {
        TreeLod::Impostor
    }
}

/// One procedural rung as a registerable mesh in LOCAL space: grounded at y = 0 and centred
/// in XZ, exactly as `bake_tree_lod` returns it. Position, yaw and scale ride the instance
/// matrix, so the same upload serves every copy on the map.
///
/// Trunk and canopy are merged into one `SceneVertex` stream with the same colouring the
/// statics bake uses (`foliage::push_baked_tree` — painterly crown shading, bark surface lane
/// on the trunk), so the instanced path and the baked path agree while both exist.
pub fn tree_mesh_asset(lod: TreeLod) -> MeshAsset {
    let bake_lod = match lod {
        TreeLod::Near => BakeLod::Close,
        TreeLod::Mid | TreeLod::Impostor => BakeLod::Mid,
    };
    let tree = world_forge::tree::bake_tree_lod(BATTLE_TREE, RUNG_SEED, bake_lod);
    let height = tree.tip().max(0.01);
    // Wind is a NEAR-rung luxury. The gust field is per-vertex noise, and past the near band
    // a 28 cm sway is under a pixel — the coarser rungs carry sway 0, so the shader's
    // `sway > 0` branch skips them entirely and the cost lands only on the handful of trees
    // a crew can actually watch move.
    let windy = lod == TreeLod::Near;

    let mut vertices: Vec<renderer_api::SceneVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let canopy_color = crate::foliage::canopy_color_for_species(BATTLE_TREE);
    for (mesh, (color, gloss), is_canopy) in
        [(&tree.trunk, crate::foliage::TRUNK_TONE, false), (&tree.canopy, canopy_color, true)]
    {
        let start = vertices.len() as u32;
        for vertex in mesh.vertices() {
            let position = vertex.position;
            // The painterly gradient, same as the statics bake: crown tops toward the light,
            // undersides into shade; the trunk wears bark down the surface lane.
            let shade = if is_canopy { 0.82 + 0.18 * vertex.normal.y.max(0.0) } else { 1.0 };
            let role = if is_canopy {
                renderer_api::surface_role::FOLIAGE
            } else {
                renderer_api::surface_role::BARK
            };
            vertices.push(
                renderer_api::SceneVertex::surfaced(
                    position.to_array(),
                    vertex.normal.to_array(),
                    [color[0] * shade, color[1] * shade, color[2] * shade],
                    gloss,
                )
                .with_surface(role)
                .with_sway(if windy {
                    sway_allowance(position.to_array(), height, is_canopy)
                } else {
                    0.0
                }),
            );
        }
        indices.extend(mesh.indices().iter().map(|index| index + start));
    }
    // The card canopy (Drzewa 3.0 PR6): each cluster card becomes one quad with DUAL winding
    // (4 vertices, 4 triangles) — backface culling stays on and never eats the far side, and
    // FOLIAGE's derivative normals do not care which face won. Color is the authored canopy
    // tone times the card's shade lane (the one-mass law); the atlas mask cuts the shape and
    // the dome page curves the lighting.
    let (color, gloss) = canopy_color;
    for card in &tree.leaves {
        let rect = world_forge::tree::leaf_atlas::atlas_rect(card.slot);
        let start = vertices.len() as u32;
        // The cluster stem sits at -half_up (v1, the bottom of the slot).
        let corners = [
            (card.center - card.half_right - card.half_up, [rect[0], rect[3]]),
            (card.center + card.half_right - card.half_up, [rect[2], rect[3]]),
            (card.center + card.half_right + card.half_up, [rect[2], rect[1]]),
            (card.center - card.half_right + card.half_up, [rect[0], rect[1]]),
        ];
        // Two vertex rings, one per face: the far side carries the NEGATED normal, or a card
        // seen from behind lights only by the transmission lobe and reads as a black cutout.
        // 8 vertices, still 4 triangles.
        for face_normal in [card.normal, -card.normal] {
            for (position, uv) in corners {
                vertices.push(
                    renderer_api::SceneVertex::surfaced(
                        position.to_array(),
                        face_normal.to_array(),
                        [color[0] * card.shade, color[1] * card.shade, color[2] * card.shade],
                        gloss,
                    )
                    .with_surface(renderer_api::surface_role::FOLIAGE)
                    .with_uv(uv)
                    .with_sway(if windy {
                        sway_allowance(position.to_array(), height, true)
                    } else {
                        0.0
                    }),
                );
            }
        }
        indices.extend_from_slice(&[
            start,
            start + 1,
            start + 2,
            start,
            start + 2,
            start + 3,
            // The far side, wound the other way, on its own normal ring.
            start + 4,
            start + 6,
            start + 5,
            start + 4,
            start + 7,
            start + 6,
        ]);
    }
    MeshAsset::new(vertices, indices)
}

/// How far a vertex may ride the wind, in mesh-local metres (the shader scales it by the
/// instance). A tree bends the way a cantilever does — nothing at the root, most at the
/// tips — so the allowance grows with BOTH the height up the trunk and the reach out from
/// its axis. The two together are what separates a swaying canopy from a wobbling trunk:
/// bark near the axis keeps a fraction of a centimetre, canopy lobes at the crown edge get
/// a quarter metre. Trunk vertices opt out entirely (they are the planted end).
fn sway_allowance(position: [f32; 3], height_m: f32, is_canopy: bool) -> f32 {
    if !is_canopy {
        return 0.0;
    }
    const TIP_SWAY_M: f32 = 0.28;
    const FULL_REACH_M: f32 = 4.0;
    let height01 = (position[1] / height_m).clamp(0.0, 1.0);
    let reach = (position[0] * position[0] + position[2] * position[2]).sqrt();
    let reach01 = (reach / FULL_REACH_M).clamp(0.0, 1.0);
    // Squared height: the lower trunk is stiff, the crown is where a gust actually shows.
    TIP_SWAY_M * height01 * height01 * (0.25 + 0.75 * reach01)
}

/// Every rung's mesh, ready for `register_mesh`. All three ship by construction — the species
/// table always bakes.
pub fn tree_lod_meshes() -> Vec<(MeshHandle, MeshAsset)> {
    [TreeLod::Near, TreeLod::Mid, TreeLod::Impostor]
        .into_iter()
        .map(|lod| (lod.mesh(), tree_mesh_asset(lod)))
        .collect()
}

/// Mirrors the statics bake's rule (`battlefield::scenery_stands_in_cleared_cover`): phase 2 is
/// "gone", and dressing inside a gone box goes with it.
fn stands_in_cleared_cover(
    instance: &SceneryInstance,
    cover: &[terrain::StaticCoverObject],
    cover_states: &[u8],
) -> bool {
    let p = instance.position;
    cover.iter().enumerate().any(|(index, object)| {
        cover_states.get(index).copied().unwrap_or(0) == 2
            && (p[0] - object.center[0]).abs() <= object.half_extents_m[0]
            && (p[2] - object.center[2]).abs() <= object.half_extents_m[2]
    })
}

/// Which rung each tree drew last frame, in scenery order. Owned by the caller (the app) so
/// the selection stays deterministic per instance without a map lookup per frame.
#[derive(Debug, Clone, Default)]
pub struct TreeLodState {
    levels: Vec<Option<TreeLod>>,
}

impl TreeLodState {
    pub fn levels(&self) -> &[Option<TreeLod>] {
        &self.levels
    }
}

/// The battle frame's tree instances. Distance is measured in the XZ plane — a tank's eye is
/// always within a couple of metres of the ground, so the vertical leg only adds noise to the
/// band a tree sits in.
///
/// `cover` and `cover_states` keep the honesty rule the statics bake used to keep for free: a
/// tree standing inside a levelled `TreeLine` box is GONE, because the box it dressed is gone
/// and the bake has already put stumps and a fallen trunk in its place. An instanced tree that
/// ignored this would hang in the air over its own wreckage.
pub fn tree_frame_objects(
    scenery: &[SceneryInstance],
    cover: &[terrain::StaticCoverObject],
    cover_states: &[u8],
    eye: Vec3,
    state: &mut TreeLodState,
) -> Vec<RenderObject> {
    let trees: Vec<&SceneryInstance> = scenery
        .iter()
        .filter(|instance| instance.kind == SceneryKind::Oak)
        .filter(|instance| !stands_in_cleared_cover(instance, cover, cover_states))
        .collect();
    if state.levels.len() != trees.len() {
        state.levels = vec![None; trees.len()];
    }
    let mut objects = Vec::with_capacity(trees.len());
    for (index, instance) in trees.iter().enumerate() {
        let base = Vec3::from_array(instance.position);
        let distance = (Vec3::new(base.x, 0.0, base.z) - Vec3::new(eye.x, 0.0, eye.z)).length();
        let lod = select_lod(distance, state.levels[index]);
        state.levels[index] = Some(lod);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(instance.scale),
            Quat::from_rotation_y(instance.yaw_rad),
            base - Vec3::Y * TRUNK_SINK_M,
        );
        objects.push(RenderObject {
            tank_id: None,
            mesh: lod.mesh(),
            material: MaterialHandle(0),
            transform: transform.to_cols_array_2d(),
            // The canopy's painterly shade is baked into the vertex colours; the per-instance
            // tint stays neutral (tint-weighted vertices are a vehicle-livery mechanism).
            tint: [1.0, 1.0, 1.0],
        });
    }
    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bands, and the hysteresis that keeps a tree from flickering on a boundary: a hull
    /// idling at 55 m must not swap meshes every frame.
    #[test]
    fn rungs_switch_on_their_bands_and_stick_through_jitter() {
        assert_eq!(select_lod(10.0, None), TreeLod::Near);
        assert_eq!(select_lod(90.0, None), TreeLod::Mid);
        assert_eq!(select_lod(400.0, None), TreeLod::Impostor);

        // Sitting just past the near band: a tree already drawing Near holds it, while one
        // arriving from farther out stays coarse — the same distance, two answers, on purpose.
        let just_past = NEAR_MAX_M + HYSTERESIS_M * 0.5;
        assert_eq!(select_lod(just_past, Some(TreeLod::Near)), TreeLod::Near);
        assert_eq!(select_lod(just_past, Some(TreeLod::Mid)), TreeLod::Mid);
        // Far enough past it, even the holdout coarsens.
        assert_eq!(select_lod(NEAR_MAX_M + HYSTERESIS_M * 1.5, Some(TreeLod::Near)), TreeLod::Mid);
        // And the impostor refines only once well inside the mid band.
        assert_eq!(
            select_lod(MID_MAX_M - HYSTERESIS_M * 0.5, Some(TreeLod::Impostor)),
            TreeLod::Impostor
        );
        assert_eq!(
            select_lod(MID_MAX_M - HYSTERESIS_M * 1.5, Some(TreeLod::Impostor)),
            TreeLod::Mid
        );
    }

    /// The shipped ladder descends in cost: the near rung is the heaviest, the impostor the
    /// lightest — the whole point of the ladder.
    #[test]
    fn the_shipped_ladder_descends_in_cost() {
        let meshes = tree_lod_meshes();
        assert_eq!(meshes.len(), 3, "three rungs ship");
        let tris = |handle: MeshHandle| {
            meshes
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, mesh)| mesh.index_count() / 3)
                .expect("rung ships")
        };
        assert!(tris(TREE_NEAR_MESH) > tris(TREE_MID_MESH));
        assert!(tris(TREE_MID_MESH) >= tris(TREE_IMPOSTOR_MESH));
        // The Near rung is bark (the species mesh budget) PLUS the card deck at 4 tris a
        // card. Ceiling re-set with PR6's composition (measured 2,980 = ~1,900 bark +
        // ~270 cards); the MX330 verdict is the flora_frame_probe's two views, not this
        // number — this only catches silent geometric growth.
        const NEAR_RUNG_MAX_TRIS: usize = 3_100;
        assert!(
            tris(TREE_NEAR_MESH) <= NEAR_RUNG_MAX_TRIS,
            "the near rung outgrew its ceiling: {}",
            tris(TREE_NEAR_MESH)
        );
    }

    /// LOD must not shrink the tree (Świat 2.0 PR1): Near and Mid stand the same height, so a
    /// rung swap moves triangles, never metres. Impostor shares Mid's bake today.
    #[test]
    fn lod_rungs_agree_in_height() {
        let near = tree_mesh_asset(TreeLod::Near);
        let mid = tree_mesh_asset(TreeLod::Mid);
        let tip = |mesh: &MeshAsset| {
            mesh.vertices().iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max)
        };
        let near_tip = tip(&near);
        let mid_tip = tip(&mid);
        assert!(
            (near_tip - mid_tip).abs() < 0.05,
            "Near tip {near_tip} vs Mid tip {mid_tip} — a swap must not resize the oak"
        );
        assert!(near_tip > 15.0, "the battlefield oak stays mature: {near_tip}");
    }

    /// The frame builder emits one instance per authored tree, grounded where the map put it.
    #[test]
    fn every_authored_tree_draws_once_at_its_map_position() {
        let scenery = vec![
            SceneryInstance {
                kind: SceneryKind::Oak,
                position: [100.0, 5.0, 100.0],
                yaw_rad: 0.4,
                scale: 1.0,
            },
            SceneryInstance {
                kind: SceneryKind::Rock,
                position: [110.0, 5.0, 100.0],
                yaw_rad: 0.0,
                scale: 1.0,
            },
        ];
        let mut state = TreeLodState::default();
        let objects =
            tree_frame_objects(&scenery, &[], &[], Vec3::new(100.0, 3.0, 90.0), &mut state);
        assert_eq!(objects.len(), 1, "rocks are not trees");
        assert_eq!(objects[0].mesh, TREE_NEAR_MESH, "ten metres away is the near rung");
        let translation = objects[0].transform[3];
        // Planted, not parked on top: the trunk is set into its ground by `TRUNK_SINK_M`,
        // because a tree left at exactly the sampled height shows daylight under its butt the
        // moment the field tilts.
        assert_eq!(
            [translation[0], translation[1], translation[2]],
            [100.0, 5.0 - TRUNK_SINK_M, 100.0]
        );
        assert_eq!(state.levels(), &[Some(TreeLod::Near)]);
    }

    /// The wind lane bends a tree like a cantilever: planted at the root, loosest at the crown
    /// edge. A trunk that swayed would read as rubber, and a canopy that did not would read as
    /// plastic — so the two ends are locked apart here.
    #[test]
    fn the_wind_lane_plants_the_root_and_frees_the_canopy() {
        let height = 13.0;
        let root = sway_allowance([0.0, 0.0, 0.0], height, true);
        let trunk_mid = sway_allowance([0.2, height * 0.4, 0.0], height, false);
        let crown_edge = sway_allowance([4.5, height * 0.95, 0.0], height, true);
        assert_eq!(root, 0.0, "the root is planted");
        assert_eq!(trunk_mid, 0.0, "the trunk stays out of the wind lane");
        assert!(crown_edge > 0.2, "the crown edge rides the gust, got {crown_edge}");

        // And the shipped near mesh actually carries it: some vertices planted, some free.
        let mesh = tree_mesh_asset(TreeLod::Near);
        let max = mesh.vertices().iter().fold(0.0_f32, |acc, v| acc.max(v.sway));
        assert!(max > 0.15, "the canopy opted into the wind, peak {max}");
        assert!(mesh.vertices().iter().any(|v| v.sway == 0.0), "the trunk stays planted");

        // The coarse rungs opt OUT entirely: their sway would cost gust-noise passes for motion
        // that lands under a pixel.
        for far in [TreeLod::Mid, TreeLod::Impostor] {
            let coarse = tree_mesh_asset(far);
            assert!(
                coarse.vertices().iter().all(|v| v.sway == 0.0),
                "{far:?} must not pay for wind nobody can see"
            );
        }
    }

    /// The fill budget (Drzewa 3.0): card COUNT is the axis that prices the under-crown view
    /// on the MX330 — three depth passes sample the atlas per fragment. The ceilings are the
    /// program's plan numbers; the floors keep a thinned deck from balding into glitter.
    #[test]
    fn the_card_deck_stays_inside_its_fill_budget() {
        let cards = |lod: TreeLod| {
            // 8 vertices a card: two normal rings, one per face.
            tree_mesh_asset(lod).vertices().iter().filter(|vertex| vertex.uv != [0.0, 0.0]).count()
                / 8
        };
        let near = cards(TreeLod::Near);
        let mid = cards(TreeLod::Mid);
        assert!((120..=260).contains(&near), "Near deck: {near} cards");
        assert!((40..=90).contains(&mid), "Mid deck: {mid} cards");
        assert_eq!(cards(TreeLod::Impostor), mid, "the impostor shares Mid's bake until PR10");
    }

    /// The honesty rule: level the tree line and the oak dressing it goes with it, instead of
    /// hanging over the stumps the bake leaves behind.
    #[test]
    fn a_tree_in_a_levelled_cover_box_stops_drawing() {
        let scenery = vec![SceneryInstance {
            kind: SceneryKind::Oak,
            position: [100.0, 5.0, 100.0],
            yaw_rad: 0.0,
            scale: 1.0,
        }];
        let cover = vec![terrain::StaticCoverObject {
            id: "test-line".to_string(),
            name: "test line".to_string(),
            kind: terrain::StaticCoverKind::TreeLine,
            center: [100.0, 5.0, 100.0],
            half_extents_m: [6.0, 3.0, 6.0],
        }];
        let eye = Vec3::new(100.0, 3.0, 90.0);
        let mut state = TreeLodState::default();
        assert_eq!(tree_frame_objects(&scenery, &cover, &[0], eye, &mut state).len(), 1, "intact");
        assert!(
            tree_frame_objects(&scenery, &cover, &[2], eye, &mut state).is_empty(),
            "a levelled box takes its dressing with it"
        );
    }
}
