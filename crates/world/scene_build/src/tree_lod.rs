//! Instanced hero trees with runtime LOD (hero-flora phase 2).
//!
//! Trees left the statics bake: a 4.8k-tri oak baked once per instance into the shared vertex
//! buffer meant every copy paid full price in every pass, and the min-spec measurement put the
//! ceiling at ten trees a map. As registered meshes they cost ONE upload and a matrix per
//! instance — and, more importantly, the copy the camera sees at 200 m can be a different mesh
//! from the one it sees at 20 m.
//!
//! Three rungs, all distilled from the same Blender master so the silhouette never changes:
//! the near mesh (fronds on real branches), a sparse mid mesh, and a two-quad impostor. Each
//! rung is height-corrected to the near mesh, so a swap moves texels, never the tree's size.

use glam::{Mat4, Quat, Vec3};
use renderer_api::{MaterialHandle, MeshAsset, MeshHandle, RenderObject};
use terrain::{SceneryInstance, SceneryKind};

/// Mesh handles for the three rungs. They sit BELOW [`renderer_api::SHADOWLESS_DRESSING_MESH_BASE`]
/// on purpose: grass may skip the depth passes, a tree may not — its shadow is half of what a
/// tree contributes to a battlefield.
pub const TREE_NEAR_MESH: MeshHandle = MeshHandle(0xFEE0_0001);
pub const TREE_MID_MESH: MeshHandle = MeshHandle(0xFEE0_0002);
pub const TREE_IMPOSTOR_MESH: MeshHandle = MeshHandle(0xFEE0_0003);

const _: () = assert!(TREE_IMPOSTOR_MESH.0 < renderer_api::SHADOWLESS_DRESSING_MESH_BASE);

/// The shipped asset behind each rung.
const NEAR_ASSET: &str = "dab-hero";
const MID_ASSET: &str = "dab-hero-lod2";
const IMPOSTOR_ASSET: &str = "dab-hero-imp";

/// Rung boundaries in metres, and the band a tree must re-cross before it swaps back. Without
/// the hysteresis a tree parked exactly on a boundary would flicker between two meshes as the
/// hull idles; 8 m is wider than any camera jitter and far narrower than a deliberate approach.
pub const NEAR_MAX_M: f32 = 55.0;
pub const MID_MAX_M: f32 = 150.0;
pub const HYSTERESIS_M: f32 = 8.0;

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

    fn asset(self) -> &'static str {
        match self {
            Self::Near => NEAR_ASSET,
            Self::Mid => MID_ASSET,
            Self::Impostor => IMPOSTOR_ASSET,
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

/// One flora asset as a registerable mesh in ASSET-LOCAL space: grounded at y = 0 and centred
/// in XZ, exactly as the import gate normalized it. Position, yaw and scale ride the instance
/// matrix, so the same upload serves every copy on the map.
///
/// The vertex build mirrors `foliage::push_imported_flora` — same atlas region remap, same
/// FOLIAGE surface role, same baked vertex light — because the two paths must agree pixel for
/// pixel while both exist.
pub fn flora_mesh_asset(name: &str) -> Option<MeshAsset> {
    let (asset, region) = crate::flora_pack::flora_catalog().get(name)?;
    let vertices = asset
        .positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let uv = asset.uvs[index];
            renderer_api::SceneVertex::surfaced(
                *position,
                asset.normals[index],
                asset.colors[index],
                0.07,
            )
            .with_surface(renderer_api::surface_role::FOLIAGE)
            .with_uv([
                region.u_offset + uv[0] * region.u_scale,
                region.v_offset + uv[1] * region.v_scale,
            ])
        })
        .collect();
    Some(MeshAsset::new(vertices, asset.indices.clone()))
}

/// Every rung's mesh, ready for `register_mesh`. A rung whose asset is missing is skipped
/// rather than faked: [`tree_frame_objects`] then falls back to the near mesh, so a partial
/// asset set degrades to "always full detail" instead of to an invisible tree.
pub fn tree_lod_meshes() -> Vec<(MeshHandle, MeshAsset)> {
    [TreeLod::Near, TreeLod::Mid, TreeLod::Impostor]
        .into_iter()
        .filter_map(|lod| flora_mesh_asset(lod.asset()).map(|mesh| (lod.mesh(), mesh)))
        .collect()
}

/// Per-rung uniform scale that makes every mesh stand as tall as the near one. The distilled
/// meshes land within a few percent of each other (decimation moves the canopy tip, the
/// impostor's billboard is measured off a render), and a few percent of 22 m is a visible pop.
fn rung_height_fix(lod: TreeLod) -> f32 {
    let catalog = crate::flora_pack::flora_catalog();
    let near = catalog.get(NEAR_ASSET).map(|(asset, _)| asset.height_m).unwrap_or(1.0);
    let own = catalog.get(lod.asset()).map(|(asset, _)| asset.height_m).unwrap_or(near);
    if own > 0.01 { near / own } else { 1.0 }
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
    let catalog = crate::flora_pack::flora_catalog();
    let have_mid = catalog.get(MID_ASSET).is_some();
    let have_impostor = catalog.get(IMPOSTOR_ASSET).is_some();
    let trees: Vec<&SceneryInstance> = scenery
        .iter()
        .filter(|instance| instance.kind == SceneryKind::FloraTree)
        .filter(|instance| !stands_in_cleared_cover(instance, cover, cover_states))
        .collect();
    if state.levels.len() != trees.len() {
        state.levels = vec![None; trees.len()];
    }
    let mut objects = Vec::with_capacity(trees.len());
    for (index, instance) in trees.iter().enumerate() {
        let base = Vec3::from_array(instance.position);
        let distance = (Vec3::new(base.x, 0.0, base.z) - Vec3::new(eye.x, 0.0, eye.z)).length();
        let mut lod = select_lod(distance, state.levels[index]);
        // Degrade to the rung we actually shipped rather than drawing nothing.
        if (lod == TreeLod::Mid && !have_mid) || (lod == TreeLod::Impostor && !have_impostor) {
            lod = TreeLod::Near;
        }
        state.levels[index] = Some(lod);
        let scale = instance.scale
            * crate::foliage::imported_flora_scale(instance.kind)
            * rung_height_fix(lod);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(instance.yaw_rad),
            base,
        );
        objects.push(RenderObject {
            tank_id: None,
            mesh: lod.mesh(),
            material: MaterialHandle(0),
            transform: transform.to_cols_array_2d(),
            // The asset's own vertex colours carry the canopy's baked light; the per-instance
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

    /// Every shipped rung registers a real mesh, and the near rung is the heaviest — the whole
    /// point of the ladder. Heights agree after the per-rung fix, so a swap does not resize.
    #[test]
    fn the_shipped_ladder_descends_in_cost_and_agrees_in_height() {
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
        assert!(tris(TREE_MID_MESH) > tris(TREE_IMPOSTOR_MESH));
        assert!(tris(TREE_IMPOSTOR_MESH) <= 8, "the impostor is a crossed pair of quads");

        let catalog = crate::flora_pack::flora_catalog();
        let near_h = catalog.get(NEAR_ASSET).expect("near ships").0.height_m;
        for lod in [TreeLod::Mid, TreeLod::Impostor] {
            let own = catalog.get(lod.asset()).expect("rung ships").0.height_m;
            let corrected = own * rung_height_fix(lod);
            assert!(
                (corrected - near_h).abs() < 0.01,
                "{lod:?} stands {corrected} m against the near rung's {near_h} m"
            );
        }
    }

    /// The frame builder emits one instance per authored tree, grounded where the map put it.
    #[test]
    fn every_authored_tree_draws_once_at_its_map_position() {
        let scenery = vec![
            SceneryInstance {
                kind: SceneryKind::FloraTree,
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
        assert_eq!([translation[0], translation[1], translation[2]], [100.0, 5.0, 100.0]);
        assert_eq!(state.levels(), &[Some(TreeLod::Near)]);
    }

    /// The honesty rule: level the tree line and the oak dressing it goes with it, instead of
    /// hanging over the stumps the bake leaves behind.
    #[test]
    fn a_tree_in_a_levelled_cover_box_stops_drawing() {
        let scenery = vec![SceneryInstance {
            kind: SceneryKind::FloraTree,
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
