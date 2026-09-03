//! Instanced battlefield trees with runtime LOD (hero-flora phase 2, retargeted to
//! procedural species in Świat 2.0).
//!
//! Trees left the statics bake: a ~1k-tri oak baked once per instance into the shared vertex
//! buffer meant every copy paid full price in every pass, and the min-spec measurement put the
//! ceiling at ten trees a map. As registered meshes they cost ONE upload and a matrix per
//! instance — and, more importantly, the copy the camera sees at 200 m can be a different mesh
//! from the one it sees at 20 m.
//!
//! Three rungs per SPECIES (Inny Poziom F7): the near mesh (full LOD0 bake), a sparse mid
//! mesh (LOD1), and a crossed-quad impostor over the species' sprite pair. Each rung stands
//! the same height by construction (one species, one parameter set), so a swap moves texels,
//! never the tree's size. Every planted tree species rides the ladder — until F7 only the oak
//! did, and a willow ten metres from the eye was the statics bake's thinned Mid deck with no
//! wind, paid in every shadow cascade at every distance.

use glam::{Mat4, Quat, Vec3};
use renderer_api::{MaterialHandle, MeshAsset, MeshHandle, RenderObject};
use terrain::{SceneryInstance, SceneryKind};
use world_forge::tree::{TreeLod as BakeLod, TreeSpecies};

/// The base of the ladder's mesh-handle block: one handle per (species, rung), see
/// [`ladder_mesh`]. The block sits BELOW [`renderer_api::SHADOWLESS_DRESSING_MESH_BASE`] on
/// purpose: grass may skip the depth passes, a tree may not — its shadow is half of what a
/// tree contributes to a battlefield.
const TREE_MESH_BASE: u32 = 0xFEE0_0000;
/// Handles per variant: three rungs and a spare; per species: `VARIANTS` variants — so the
/// oak's original three (`0xFEE0_0001..3`) are its first variant's and keep their numbers.
const HANDLES_PER_VARIANT: u32 = 4;
pub const VARIANTS: u32 = world_forge::tree::authored::VARIANTS;
const HANDLES_PER_SPECIES: u32 = HANDLES_PER_VARIANT * VARIANTS;

/// The oak's first variant's rungs under the original handles — the names the tests and
/// probes grew up with.
pub const TREE_NEAR_MESH: MeshHandle = ladder_mesh(TreeSpecies::Oak, 0, TreeLod::Near);
pub const TREE_MID_MESH: MeshHandle = ladder_mesh(TreeSpecies::Oak, 0, TreeLod::Mid);
pub const TREE_IMPOSTOR_MESH: MeshHandle = ladder_mesh(TreeSpecies::Oak, 0, TreeLod::Impostor);

const _: () = assert!(
    TREE_MESH_BASE + LADDER_SPECIES.len() as u32 * HANDLES_PER_SPECIES
        < renderer_api::SHADOWLESS_DRESSING_MESH_BASE
);

/// The species that ride the instanced ladder, in handle order — append-only, because a handle
/// is a species × variant × rung slot. Every planted species is here: the trees since Inny
/// Poziom F7, the bush since route 2 gave it an authored shrub of its own.
pub const LADDER_SPECIES: [TreeSpecies; 6] = [
    TreeSpecies::Oak,
    TreeSpecies::Poplar,
    TreeSpecies::Willow,
    TreeSpecies::FruitTree,
    TreeSpecies::Pine,
    TreeSpecies::Bush,
];

/// The mesh handle of one species' variant's rung: the block base, the species' slot, the
/// variant's slot, the rung's index. `const` so the oak's named handles above are
/// compile-time constants.
pub const fn ladder_mesh(species: TreeSpecies, variant: u32, lod: TreeLod) -> MeshHandle {
    // A `const fn` cannot search `LADDER_SPECIES`; this match IS that order, and
    // `ladder_slots_follow_the_species_order` proves the two agree.
    let slot: u32 = match species {
        TreeSpecies::Oak => 0,
        TreeSpecies::Poplar => 1,
        TreeSpecies::Willow => 2,
        TreeSpecies::FruitTree => 3,
        TreeSpecies::Pine => 4,
        TreeSpecies::Bush => 5,
    };
    MeshHandle(
        TREE_MESH_BASE
            + slot * HANDLES_PER_SPECIES
            + (variant % VARIANTS) * HANDLES_PER_VARIANT
            + lod.rung_index(),
    )
}

/// The seed an instance grows from — its position bits, the statics bake's own rule
/// (`foliage::statics_tree_seed`) — which names its variant and mirror on every route.
pub fn instance_seed(instance: &SceneryInstance) -> u64 {
    instance.position[0].to_bits() as u64 ^ ((instance.position[2].to_bits() as u64) << 32)
}

/// The variant an instance draws on the ladder (the mirror is a statics-only luxury: a shared
/// mesh cannot flip per instance without flipping its winding).
pub fn instance_variant(instance: &SceneryInstance) -> u32 {
    world_forge::tree::authored::variant_of_seed(instance_seed(instance)).0
}

/// Which scenery kinds ride the instanced ladder, and as which species. `None` is a kind the
/// statics bake still owns (the bush, rocks, street furniture, the retired imports). ONE
/// answer for the frame builder, the statics bake's skip rule and the instruments that must
/// draw exactly what the battle draws.
pub fn ladder_species(kind: SceneryKind) -> Option<TreeSpecies> {
    match kind {
        SceneryKind::Oak => Some(TreeSpecies::Oak),
        SceneryKind::Poplar => Some(TreeSpecies::Poplar),
        SceneryKind::Willow => Some(TreeSpecies::Willow),
        SceneryKind::FruitTree => Some(TreeSpecies::FruitTree),
        SceneryKind::Pine => Some(TreeSpecies::Pine),
        SceneryKind::Bush => Some(TreeSpecies::Bush),
        SceneryKind::Rock
        | SceneryKind::Lamppost
        | SceneryKind::DebrisHeap
        | SceneryKind::FloraTree
        | SceneryKind::FloraPine
        | SceneryKind::FloraBush => None,
    }
}

/// Rung boundaries in metres, and the band a tree must re-cross before it swaps back. Without
/// the hysteresis a tree parked exactly on a boundary would flicker between two meshes as the
/// hull idles; 8 m is wider than any camera jitter and far narrower than a deliberate approach.
/// Re-set 2026-09-03 after the owner played (route 2): "approaching a tree it visibly changes
/// — no game does that; from afar they look tragic; don't overdo the budgets". Near reaches
/// 120 m and Mid 300 m; Mid draws the SAME card deck as Near (only the wood is coarser), so
/// the first swap is invisible, and the impostor takes over where a 2D sprite reads.
pub const NEAR_MAX_M: f32 = 120.0;
pub const MID_MAX_M: f32 = 300.0;
pub const HYSTERESIS_M: f32 = 15.0;

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
pub(crate) const TRUNK_SINK_M: f32 = 0.35;

/// The seed the ladder grows variant `variant` from (unmirrored).
pub(crate) fn ladder_variant_seed(variant: u32) -> u64 {
    world_forge::tree::authored::variant_seed(variant)
}

/// The rendered canopy tip of one instanced battle tree, metres above the instance's map
/// position: ITS variant's rung tip at the instance scale, minus the trunk sink. The number
/// a TreeLine collision box must tower over to honestly contain the trees it hosts (PR 5).
/// Test-only: the honesty lock `tree_line_boxes_contain_the_trees_they_host` is its caller.
#[cfg(test)]
pub(crate) fn battle_tree_rendered_top_m(species: TreeSpecies, seed: u64, scale: f32) -> f32 {
    // `tip()` covers every crown representation — lobes then, the card deck now (PR6): the
    // TreeLine box must tower over whatever actually draws.
    let tip = world_forge::tree::bake_tree_lod(species, seed, BakeLod::Close).tip();
    tip * scale - TRUNK_SINK_M
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeLod {
    Near,
    Mid,
    Impostor,
}

impl TreeLod {
    pub const ALL: [TreeLod; 3] = [TreeLod::Near, TreeLod::Mid, TreeLod::Impostor];

    /// The rung's index inside a species' handle slot (1-based: slot 0 stays free).
    pub const fn rung_index(self) -> u32 {
        match self {
            Self::Near => 1,
            Self::Mid => 2,
            Self::Impostor => 3,
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

/// One procedural rung of one species as a registerable mesh in LOCAL space: grounded at
/// y = 0 and centred in XZ, exactly as `bake_tree_lod` returns it. Position, yaw and scale
/// ride the instance matrix, so the same upload serves every copy on the map.
///
/// Trunk and canopy are merged into one `SceneVertex` stream with the same colouring the
/// statics bake uses (`foliage::push_baked_tree` — painterly crown shading, bark surface lane
/// on the trunk), so the instanced path and the baked path agree while both exist.
pub fn tree_mesh_asset(species: TreeSpecies, variant: u32, lod: TreeLod) -> MeshAsset {
    if lod == TreeLod::Impostor {
        return impostor_mesh_asset(species);
    }
    let bake_lod = match lod {
        TreeLod::Near => BakeLod::Close,
        TreeLod::Mid | TreeLod::Impostor => BakeLod::Mid,
    };
    let tree = world_forge::tree::bake_tree_lod(species, ladder_variant_seed(variant), bake_lod);
    let bark_role = renderer_api::surface_role::bark_for_layer(
        world_forge::tree::authored::species_index(species),
    );
    let height = tree.tip().max(0.01);
    // Wind is a NEAR-rung luxury. The gust field is per-vertex noise, and past the near band
    // a 28 cm sway is under a pixel — the coarser rungs carry sway 0, so the shader's
    // `sway > 0` branch skips them entirely and the cost lands only on the handful of trees
    // a crew can actually watch move.
    let windy = lod == TreeLod::Near;

    let mut vertices: Vec<renderer_api::SceneVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let canopy_color = crate::foliage::canopy_color_for_species(species);
    for (mesh, (color, gloss), is_canopy) in
        [(&tree.trunk, crate::foliage::TRUNK_TONE, false), (&tree.canopy, canopy_color, true)]
    {
        let start = vertices.len() as u32;
        for vertex in mesh.vertices() {
            let position = vertex.position;
            // The painterly gradient, same as the statics bake: crown tops toward the light,
            // undersides into shade; the trunk wears bark down the surface lane.
            let shade = if is_canopy { 0.82 + 0.18 * vertex.normal.y.max(0.0) } else { 1.0 };
            let role = if is_canopy { renderer_api::surface_role::FOLIAGE } else { bark_role };
            vertices.push(
                renderer_api::SceneVertex::surfaced(
                    position.to_array(),
                    vertex.normal.to_array(),
                    [color[0] * shade, color[1] * shade, color[2] * shade],
                    gloss,
                )
                .with_surface(role)
                .with_sway(if windy {
                    sway_allowance(species, position.to_array(), height, is_canopy)
                } else {
                    0.0
                }),
            );
        }
        indices.extend(mesh.indices().iter().map(|index| index + start));
    }
    // The card canopy (Drzewa 3.0 PR6): the shared expansion in `foliage::push_leaf_cards` —
    // one code path for the instanced ladder and the statics bake, so a card can never render
    // differently by route. Here the tree stays in local space and the Near rung opts into
    // the cantilever wind.
    crate::foliage::push_leaf_cards(
        &mut vertices,
        &mut indices,
        &tree,
        crate::foliage::card_color_for_species(species),
        |local| local,
        |direction| direction,
        // L2 of the wind hierarchy (PR11): every card's allowance carries its own baked
        // ±15% jitter, keyed off the card center so the quad never shears — a crown is many
        // branches answering one gust a beat apart, not a sheet.
        |local, center| {
            if windy {
                sway_allowance(species, local.to_array(), height, true) * card_wind_jitter(center)
            } else {
                0.0
            }
        },
    );
    MeshAsset::new(vertices, indices)
}

/// The per-card wind personality: a deterministic ±15% on the sway allowance, hashed from
/// the card's center. Pure function — the same card always answers the wind the same way.
pub(crate) fn card_wind_jitter(center: Vec3) -> f32 {
    let mut hash = u64::from(center.x.to_bits())
        ^ (u64::from(center.y.to_bits()) << 21)
        ^ (u64::from(center.z.to_bits()) << 42);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    hash ^= hash >> 33;
    0.85 + 0.30 * ((hash >> 40) as f32 / (1u64 << 24) as f32)
}

/// The TRUE impostor (Drzewa 3.0 PR10): two crossed vertical quads sampling the pre-splatted
/// sprite pair in the foliage atlas — 8 triangles where the fake used to resubmit Mid's
/// whole bake. The expansion lives in `foliage::push_impostor_quads`, shared with the
/// backdrop ring's statics bake (Inny Poziom F1), so a far oak on the field and a far oak on
/// the enclosing hills are the same quads over the same sprite. Here the tree stays in local
/// space; position, yaw and scale ride the instance matrix.
fn impostor_mesh_asset(species: TreeSpecies) -> MeshAsset {
    let mut vertices: Vec<renderer_api::SceneVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    crate::foliage::push_impostor_quads(
        &mut vertices,
        &mut indices,
        species,
        |local| local,
        |direction| direction,
    );
    MeshAsset::new(vertices, indices)
}

/// How far a species' crown tip rides a gust, as a factor on the oak's 28 cm. A weeping
/// willow's pendulous curtain answers the wind most; a poplar's tall slender crown flutters
/// but reaches little; an orchard tree is short and stiff; a pine's stacked conical crown
/// hardly moves. Authored, not measured — the frame the owner looks at decides.
fn species_sway_factor(species: TreeSpecies) -> f32 {
    match species {
        TreeSpecies::Oak => 1.0,
        TreeSpecies::Poplar => 1.1,
        TreeSpecies::Willow => 1.4,
        TreeSpecies::FruitTree => 0.8,
        TreeSpecies::Pine => 0.55,
        TreeSpecies::Bush => 0.6,
    }
}

/// How far a vertex may ride the wind, in mesh-local metres (the shader scales it by the
/// instance). A tree bends the way a cantilever does — nothing at the root, most at the
/// tips — so the allowance grows with BOTH the height up the trunk and the reach out from
/// its axis. The two together are what separates a swaying canopy from a wobbling trunk:
/// bark near the axis keeps a fraction of a centimetre, canopy lobes at the crown edge get
/// a quarter metre. Trunk vertices opt out entirely (they are the planted end).
fn sway_allowance(species: TreeSpecies, position: [f32; 3], height_m: f32, is_canopy: bool) -> f32 {
    if !is_canopy {
        return 0.0;
    }
    const TIP_SWAY_M: f32 = 0.28;
    const FULL_REACH_M: f32 = 4.0;
    let height01 = (position[1] / height_m).clamp(0.0, 1.0);
    let reach = (position[0] * position[0] + position[2] * position[2]).sqrt();
    let reach01 = (reach / FULL_REACH_M).clamp(0.0, 1.0);
    // Squared height: the lower trunk is stiff, the crown is where a gust actually shows.
    TIP_SWAY_M * species_sway_factor(species) * height01 * height01 * (0.25 + 0.75 * reach01)
}

/// Every rung of every variant of every ladder species, ready for `register_mesh`: one
/// upload per (species, variant, rung) at deployment serves every copy on the map. The
/// impostor is per species (one sprite pair), registered under every variant's handle.
pub fn tree_lod_meshes() -> Vec<(MeshHandle, MeshAsset)> {
    LADDER_SPECIES
        .into_iter()
        .flat_map(|species| {
            (0..VARIANTS).flat_map(move |variant| {
                TreeLod::ALL.into_iter().map(move |lod| {
                    (ladder_mesh(species, variant, lod), tree_mesh_asset(species, variant, lod))
                })
            })
        })
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
    /// Per tree, the instance scale after the hosting fit (`hosted_scale`) — computed once
    /// per tree set, because it parses the variant.
    scales: Vec<f32>,
}

impl TreeLodState {
    pub fn levels(&self) -> &[Option<TreeLod>] {
        &self.levels
    }
}

/// The scale a ladder tree draws at: its own, unless it stands inside a `TreeLine` box, in
/// which case it is fitted under the wall's top the way the line's own stations are — the
/// LOS wall must tower over every crown it hosts (PR 5's honesty lock), and an old-variant
/// oak (route 2) would otherwise poke 1.6 m over a wall sized for the mature one.
pub fn hosted_scale(
    instance: &SceneryInstance,
    species: TreeSpecies,
    cover: &[terrain::StaticCoverObject],
) -> f32 {
    let mut scale = instance.scale;
    let hosts = cover.iter().filter(|object| {
        object.kind == terrain::StaticCoverKind::TreeLine
            && (instance.position[0] - object.center[0]).abs() <= object.half_extents_m[0]
            && (instance.position[2] - object.center[2]).abs() <= object.half_extents_m[2]
    });
    let mut tip = None;
    for host in hosts {
        let tip = *tip.get_or_insert_with(|| {
            world_forge::tree::bake_tree_lod(species, instance_seed(instance), BakeLod::Close).tip()
        });
        let box_top = host.center[1] + host.half_extents_m[1];
        let available = (box_top - instance.position[1]).max(0.0);
        // rendered top = tip × scale − sink ≤ available − margin
        let fitted = (available - 0.05 + TRUNK_SINK_M) / tip.max(0.01);
        scale = scale.min(fitted);
    }
    scale.max(0.0)
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
    let trees: Vec<(&SceneryInstance, TreeSpecies)> = scenery
        .iter()
        .filter_map(|instance| ladder_species(instance.kind).map(|species| (instance, species)))
        .filter(|(instance, _)| !stands_in_cleared_cover(instance, cover, cover_states))
        .collect();
    if state.levels.len() != trees.len() {
        state.levels = vec![None; trees.len()];
        state.scales = trees
            .iter()
            .map(|(instance, species)| hosted_scale(instance, *species, cover))
            .collect();
    }
    let mut objects = Vec::with_capacity(trees.len());
    for (index, (instance, species)) in trees.iter().enumerate() {
        let base = Vec3::from_array(instance.position);
        let distance = (Vec3::new(base.x, 0.0, base.z) - Vec3::new(eye.x, 0.0, eye.z)).length();
        let lod = select_lod(distance, state.levels[index]);
        state.levels[index] = Some(lod);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(state.scales[index]),
            Quat::from_rotation_y(instance.yaw_rad),
            base - Vec3::Y * TRUNK_SINK_M,
        );
        objects.push(RenderObject {
            tank_id: None,
            mesh: ladder_mesh(*species, instance_variant(instance), lod),
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
        assert_eq!(select_lod(90.0, None), TreeLod::Near);
        assert_eq!(select_lod(200.0, None), TreeLod::Mid);
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

    /// The shipped ladder descends in cost for EVERY species: the near rung is the heaviest,
    /// the impostor the lightest — the whole point of the ladder.
    #[test]
    fn the_shipped_ladder_descends_in_cost() {
        let meshes = tree_lod_meshes();
        assert_eq!(
            meshes.len(),
            LADDER_SPECIES.len() * VARIANTS as usize * 3,
            "three rungs per variant per species ship"
        );
        let tris = |handle: MeshHandle| {
            meshes
                .iter()
                .find(|(h, _)| *h == handle)
                .map(|(_, mesh)| mesh.index_count() / 3)
                .expect("rung ships")
        };
        for species in LADDER_SPECIES {
            for variant in 0..VARIANTS {
                let near = tris(ladder_mesh(species, variant, TreeLod::Near));
                let mid = tris(ladder_mesh(species, variant, TreeLod::Mid));
                let impostor = tris(ladder_mesh(species, variant, TreeLod::Impostor));
                eprintln!("{species:?} v{variant}: near {near} / mid {mid} / impostor {impostor}");
                assert!(near > mid, "{species:?} v{variant}: near {near} must outweigh mid {mid}");
                assert!(mid >= impostor, "{species:?} v{variant}: mid {mid} under impostor");
                // The Near rung is the authored wood (≤ 9,000, `TREE_LOD0_TRIS`) plus the
                // capped deck (≤ 640 cards × 4); the MX330 verdict is the flora_frame_probe's
                // views, not this number — this only catches silent growth.
                assert!(near <= NEAR_RUNG_MAX_TRIS, "{species:?} v{variant}: near rung {near}");
            }
        }
    }

    /// The near rung's ceiling for every authored variant: the old willow measures 13,360
    /// (its pendulous wood plus 600 cards × 4); the MX330 verdict is the probe's.
    const NEAR_RUNG_MAX_TRIS: usize = 14_000;

    /// The handle block: every variant of every ladder species owns three distinct handles,
    /// in `LADDER_SPECIES` then variant order, all below the shadowless dressing base; the
    /// oak's first variant keeps the three handles the oak shipped with.
    #[test]
    fn ladder_slots_follow_the_species_order() {
        let mut seen = std::collections::BTreeSet::new();
        let mut last = 0;
        for species in LADDER_SPECIES {
            for variant in 0..VARIANTS {
                for lod in TreeLod::ALL {
                    let handle = ladder_mesh(species, variant, lod);
                    assert!(handle.0 < renderer_api::SHADOWLESS_DRESSING_MESH_BASE);
                    assert!(handle.0 > last, "{species:?} v{variant} {lod:?}: {:#x}", handle.0);
                    assert!(seen.insert(handle.0), "{species:?} v{variant} {lod:?} reuses");
                    last = handle.0;
                }
            }
        }
        assert_eq!(TREE_NEAR_MESH, MeshHandle(0xFEE0_0001));
        assert_eq!(TREE_MID_MESH, MeshHandle(0xFEE0_0002));
        assert_eq!(TREE_IMPOSTOR_MESH, MeshHandle(0xFEE0_0003));
        // The registration ships exactly this set, nothing twice.
        let shipped: std::collections::BTreeSet<u32> =
            tree_lod_meshes().into_iter().map(|(handle, _)| handle.0).collect();
        assert_eq!(shipped, seen);
    }

    /// The kind rule (F7, route 2): every planted species rides the ladder — the trees and
    /// the bush; the stone, the street furniture and the retired imports do not. One function
    /// answers for the frame builder, the statics bake and the instruments.
    #[test]
    fn every_planted_tree_species_rides_the_ladder_and_nothing_else_does() {
        for kind in SceneryKind::ALL {
            let expected = match kind {
                SceneryKind::Oak => Some(TreeSpecies::Oak),
                SceneryKind::Poplar => Some(TreeSpecies::Poplar),
                SceneryKind::Willow => Some(TreeSpecies::Willow),
                SceneryKind::FruitTree => Some(TreeSpecies::FruitTree),
                SceneryKind::Pine => Some(TreeSpecies::Pine),
                SceneryKind::Bush => Some(TreeSpecies::Bush),
                _ => None,
            };
            assert_eq!(ladder_species(kind), expected, "{kind:?}");
        }
        for (index, species) in LADDER_SPECIES.iter().enumerate() {
            assert!(!LADDER_SPECIES[..index].contains(species), "{species:?} listed twice");
        }
        assert_eq!(LADDER_SPECIES.len(), TreeSpecies::ALL.len());
    }

    /// LOD must not shrink the tree (Świat 2.0 PR1): Near and Mid stand the same height, so a
    /// rung swap moves triangles, never metres. Impostor shares Mid's bake today. For every
    /// species on the ladder (F7).
    #[test]
    fn lod_rungs_agree_in_height() {
        let tip = |mesh: &MeshAsset| {
            mesh.vertices().iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max)
        };
        for species in LADDER_SPECIES {
            let variant = world_forge::tree::authored::REFERENCE_VARIANT;
            let near_tip = tip(&tree_mesh_asset(species, variant, TreeLod::Near));
            let mid_tip = tip(&tree_mesh_asset(species, variant, TreeLod::Mid));
            assert!(
                (near_tip - mid_tip).abs() < 0.05,
                "{species:?}: Near tip {near_tip} vs Mid tip {mid_tip} — a swap must not resize"
            );
            // The impostor quad spans the RENDERED sprite's window (route 2, 2026-09-03): a
            // 1:2 frame that contains the tree with a little air above it, so the quad tops
            // at or above the tip and never far above — the tree inside it is to scale.
            let impostor_tip = tip(&tree_mesh_asset(species, variant, TreeLod::Impostor));
            assert!(
                impostor_tip >= near_tip - 0.05 && impostor_tip <= near_tip * 2.0,
                "{species:?}: Near tip {near_tip} vs impostor window top {impostor_tip}"
            );
            assert!(
                near_tip > species.trunk_height(),
                "{species:?}: the crown stands above its authored trunk: {near_tip}"
            );
        }
        let oak_tip = tip(&tree_mesh_asset(
            TreeSpecies::Oak,
            world_forge::tree::authored::REFERENCE_VARIANT,
            TreeLod::Near,
        ));
        assert!(oak_tip > 15.0, "the battlefield oak stays mature: {oak_tip}");
    }

    /// The frame builder emits one instance per authored tree, grounded where the map put it,
    /// drawing ITS species' rung (F7: a willow ten metres away is a near-rung willow, not an
    /// oak and not a statics deck).
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
            SceneryInstance {
                kind: SceneryKind::Willow,
                position: [104.0, 5.0, 100.0],
                yaw_rad: 0.0,
                scale: 1.1,
            },
            SceneryInstance {
                kind: SceneryKind::Bush,
                position: [106.0, 5.0, 100.0],
                yaw_rad: 0.0,
                scale: 1.0,
            },
        ];
        let mut state = TreeLodState::default();
        let objects =
            tree_frame_objects(&scenery, &[], &[], Vec3::new(100.0, 3.0, 90.0), &mut state);
        assert_eq!(objects.len(), 3, "rocks are not trees; the bush rides the ladder too");
        assert_eq!(
            objects[0].mesh,
            ladder_mesh(TreeSpecies::Oak, instance_variant(&scenery[0]), TreeLod::Near),
            "ten metres away is the near rung of the instance's own variant"
        );
        assert_eq!(
            objects[1].mesh,
            ladder_mesh(TreeSpecies::Willow, instance_variant(&scenery[2]), TreeLod::Near),
            "the willow draws the willow's near rung"
        );
        assert_eq!(
            objects[2].mesh,
            ladder_mesh(TreeSpecies::Bush, instance_variant(&scenery[3]), TreeLod::Near)
        );
        let translation = objects[0].transform[3];
        // Planted, not parked on top: the trunk is set into its ground by `TRUNK_SINK_M`,
        // because a tree left at exactly the sampled height shows daylight under its butt the
        // moment the field tilts.
        assert_eq!(
            [translation[0], translation[1], translation[2]],
            [100.0, 5.0 - TRUNK_SINK_M, 100.0]
        );
        assert_eq!(
            state.levels(),
            &[Some(TreeLod::Near), Some(TreeLod::Near), Some(TreeLod::Near)]
        );
    }

    /// The wind lane bends a tree like a cantilever: planted at the root, loosest at the crown
    /// edge. A trunk that swayed would read as rubber, and a canopy that did not would read as
    /// plastic — so the two ends are locked apart here.
    #[test]
    fn the_wind_lane_plants_the_root_and_frees_the_canopy() {
        let height = 13.0;
        let oak = TreeSpecies::Oak;
        let root = sway_allowance(oak, [0.0, 0.0, 0.0], height, true);
        let trunk_mid = sway_allowance(oak, [0.2, height * 0.4, 0.0], height, false);
        let crown_edge = sway_allowance(oak, [4.5, height * 0.95, 0.0], height, true);
        assert_eq!(root, 0.0, "the root is planted");
        assert_eq!(trunk_mid, 0.0, "the trunk stays out of the wind lane");
        assert!(crown_edge > 0.2, "the crown edge rides the gust, got {crown_edge}");

        // F5's wind half: EVERY species' near rung carries the wind — some vertices planted,
        // some free — and the coarse rungs opt OUT entirely: their sway would cost gust-noise
        // passes for motion that lands under a pixel.
        let reference = world_forge::tree::authored::REFERENCE_VARIANT;
        for species in LADDER_SPECIES {
            let mesh = tree_mesh_asset(species, reference, TreeLod::Near);
            let max = mesh.vertices().iter().fold(0.0_f32, |acc, v| acc.max(v.sway));
            assert!(
                max > 0.10 * species_sway_factor(species),
                "{species:?}: the canopy opted into the wind, peak {max}"
            );
            assert!(mesh.vertices().iter().any(|v| v.sway == 0.0), "{species:?}: trunk planted");
            for far in [TreeLod::Mid, TreeLod::Impostor] {
                let coarse = tree_mesh_asset(species, reference, far);
                assert!(
                    coarse.vertices().iter().all(|v| v.sway == 0.0),
                    "{species:?} {far:?} must not pay for wind nobody can see"
                );
            }
        }
        // The authored order of the table: the willow's curtain answers most, the pine least.
        let peak = |species: TreeSpecies| {
            tree_mesh_asset(species, reference, TreeLod::Near)
                .vertices()
                .iter()
                .fold(0.0_f32, |acc, v| acc.max(v.sway))
        };
        // (The authored willow's curtains hang LOW, and the cantilever law scales sway by
        // height — so its factor of 1.4 buys motion per card, not a taller peak than the oak.)
        assert!(peak(TreeSpecies::Willow) > 0.0);
        assert!(peak(TreeSpecies::Pine) < peak(TreeSpecies::Oak));
    }

    /// The fill budget (Drzewa 3.0): card COUNT is the axis that prices the under-crown view
    /// on the MX330 — three depth passes sample the atlas per fragment. The ceilings are the
    /// program's plan numbers; the floors keep a thinned deck from balding into glitter.
    #[test]
    fn the_card_deck_stays_inside_its_fill_budget() {
        let reference = world_forge::tree::authored::REFERENCE_VARIANT;
        let cards = |species: TreeSpecies, lod: TreeLod| {
            // 8 vertices a card: two normal rings, one per face.
            tree_mesh_asset(species, reference, lod)
                .vertices()
                .iter()
                .filter(|vertex| vertex.uv != [0.0, 0.0])
                .count()
                / 8
        };
        // Re-banded with the cross-pair clusters (user verdict 2026-08-22): every cluster
        // is two quads now, and Mid keeps every second CLUSTER — the far oak stopped being
        // a bare pole with confetti.
        let near = cards(TreeSpecies::Oak, TreeLod::Near);
        let mid = cards(TreeSpecies::Oak, TreeLod::Mid);
        // Re-banded for the authored oak (route 2): 478 near, and Mid carries the same deck.
        assert!((240..=640).contains(&near), "Near deck: {near} cards");
        assert_eq!(mid, near, "Mid deck: {mid} cards");
        for species in LADDER_SPECIES {
            // Every species thins toward the far rung: the ladder is a ladder for all of them.
            let near = cards(species, TreeLod::Near);
            let mid = cards(species, TreeLod::Mid);
            eprintln!("{species:?}: near deck {near} / mid deck {mid} cards");
            // Route 2 LOD honesty: Mid draws the SAME deck — the swap must not change the crown.
            assert_eq!(mid, near, "{species:?}: Mid deck {mid} must equal the Near deck {near}");
            assert!(near <= NEAR_DECK_MAX_CARDS, "{species:?}: Near deck {near} cards");
            // The TRUE impostor (PR10): exactly two crossed sprite quads, nothing else.
            assert_eq!(cards(species, TreeLod::Impostor), 2, "{species:?}: two crossed quads");
            let impostor_tris =
                tree_mesh_asset(species, reference, TreeLod::Impostor).index_count() / 3;
            assert!(impostor_tris <= 16, "{species:?}: impostor stays trivial: {impostor_tris}");
        }
    }

    /// The widest near deck on the ladder (F7): card COUNT is what prices the under-crown view
    /// on the MX330, so no species may quietly out-deal the deck the fill budget was set on.
    /// Measured at RUNG_SEED: oak 344, poplar 256, willow 360, fruit tree 200, pine 432 — the
    /// pine's stacked conical crown deals the widest deck, and it is the one crown a hull
    /// never parks under (the bare lower trunk keeps the eye below the cards).
    const NEAR_DECK_MAX_CARDS: usize = 760;

    /// L2 of the wind hierarchy (PR11): the per-card jitter is a pure deterministic function
    /// inside its authored band, and it actually VARIES — a crown answering a gust in
    /// lockstep is a sheet, not a tree.
    #[test]
    fn every_card_carries_its_own_wind_personality() {
        let mut seen = std::collections::BTreeSet::new();
        for index in 0..40 {
            let center = Vec3::new(index as f32 * 0.73, 8.0 + index as f32 * 0.31, -1.2);
            let jitter = card_wind_jitter(center);
            assert!((0.85..=1.15).contains(&jitter), "jitter left its band: {jitter}");
            assert_eq!(jitter, card_wind_jitter(center), "a card's personality is stable");
            seen.insert(jitter.to_bits());
        }
        assert!(seen.len() >= 30, "the crown must disagree with itself: {} values", seen.len());
        // And the shipped Near mesh carries the spread: not every card peaks at the same
        // allowance.
        let mesh = tree_mesh_asset(
            TreeSpecies::Oak,
            world_forge::tree::authored::REFERENCE_VARIANT,
            TreeLod::Near,
        );
        let sways: std::collections::BTreeSet<u32> = mesh
            .vertices()
            .iter()
            .filter(|vertex| vertex.uv != [0.0, 0.0] && vertex.sway > 0.0)
            .map(|vertex| vertex.sway.to_bits())
            .collect();
        assert!(sways.len() >= 40, "card sway values collapsed: {}", sways.len());
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
