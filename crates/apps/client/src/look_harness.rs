//! The one path that renders a canonical review view (`docs/art-direction-policy.md`).
//!
//! The policy promises that "the frame a human reviews is exactly the frame the harness locks".
//! That used to be a convention: the `prokhorovka_views` example and the `look_goldens` test each
//! hand-rolled the same ~50 lines of scene setup. They drifted, and **both** stopped binding the
//! foliage atlas — so every imported tree in the committed goldens rendered as untextured white
//! against a 1x1 default, and nobody could see it because the reviewed frame carried the same
//! bug. Conventions rot; a shared function does not.
//!
//! Everything a review frame needs is therefore assembled here, once. A caller supplies the map,
//! the views and the frame size — nothing else, so nothing else can drift.

use renderer_api::{Camera, CameraProjectionPolicy, RenderFrame, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use scene_build::review_views::ReviewView;
use terrain::MapId;

/// Anything that can go wrong while standing a review frame up.
pub type LookHarnessError = Box<dyn std::error::Error>;

/// The scene clock every review frame is rendered on. Fixed, because the cloud sheet and the
/// grass sway both crawl on it: a wall-clock here would make the goldens unlockable.
const REVIEW_SCENE_TIME_S: f32 = 12.0;

/// The vertical FOV the review camera reads the world through.
const REVIEW_FOV_DEGREES: f32 = 55.0;

/// Render `views` on `map` at `width` x `height`, returning one RGBA8 buffer per view in order.
///
/// The whole world is built for the map first (ground, statics, water, the card meadow, the
/// foliage atlas), then each view swaps in its own lighting, sky, shadow focus and grass scatter.
/// The render is a pure function of scene + profile + the fixed clock, which is what lets the
/// goldens compare byte-exactly on one machine.
pub fn render_review_views(
    map: MapId,
    views: &[ReviewView],
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>, LookHarnessError> {
    let battlefield = map_forge::battlefield(map);
    let materials = crate::terrain_material_set_for(map);
    let ((ground_vertices, ground_indices), (statics_vertices, statics_indices)) =
        crate::battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = crate::bake_terrain_ground_maps(&battlefield);
    let (water_vertices, water_indices) = crate::battlefield_water_mesh(&battlefield);
    let (dressing_vertices, dressing_indices) =
        crate::grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_vertices, &statics_indices)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_vertices,
        &ground_indices,
        &ground_maps,
        &materials,
    );
    renderer.set_water(&ctx, &water_vertices, &water_indices);
    renderer.set_dressing(&ctx, &dressing_vertices, &dressing_indices);
    // The lane the drift cost us: imported flora samples this atlas, and without it every leaf
    // and trunk reads as the 1x1 white default. The live client has always bound it
    // (`app::render`); the review path had not.
    renderer.set_foliage_atlas(&ctx, &scene_build::flora_pack::flora_catalog().atlas_mips);
    renderer.scene_time_s = REVIEW_SCENE_TIME_S;
    renderer.register_mesh(&ctx, crate::GRASS_MESH_HANDLE, &crate::grass_tuft_mesh());

    let projection = CameraProjectionPolicy::webgpu_default();
    let mut frames = Vec::with_capacity(views.len());
    for view in views {
        let grass = crate::grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &ground_maps,
            &materials,
            glam::Vec3::from_array(view.eye),
        );
        renderer.set_render_frame(&ctx, &RenderFrame { objects: grass, ..RenderFrame::default() });
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
        renderer.shadow_focus = Some(view.target);

        let camera =
            Camera { eye: view.eye, target: view.target, vertical_fov_degrees: REVIEW_FOV_DEGREES };
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        frames.push(target.read_rgba8(&ctx)?);
    }
    Ok(frames)
}
