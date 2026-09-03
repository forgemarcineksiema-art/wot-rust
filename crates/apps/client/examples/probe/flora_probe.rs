//! THE flora look gate (Świat 2.0 — procedural-only): the procedural species side by side,
//! one frame, same light — the render the accept/reject verdict is made on. The battlefield
//! oak (front) draws through the instanced LOD ladder exactly the way the battle frame
//! submits it; the back row is the statics-bake set (Poplar, Pine, Bush, Willow, FruitTree)
//! plus the retired imported kinds, which draw nothing on purpose — their emptiness IS the
//! state of the family. One PNG:
//! `cargo run -p client --example probe -- flora_probe`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{SceneryInstance, SceneryKind};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    let mut battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    battlefield.static_cover.clear();
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
    let plant = |kind: SceneryKind, x: f32, z: f32| SceneryInstance {
        kind,
        position: [x, ground(x, z), z],
        yaw_rad: 0.6,
        scale: 1.0,
    };
    battlefield.scenery = vec![
        // Front row: the battlefield oak (instanced LOD) — the tree that carries a trunk box.
        plant(SceneryKind::Oak, 486.0, 490.0),
        // The retired imported kinds: they draw nothing, on purpose.
        plant(SceneryKind::FloraTree, 498.0, 490.0),
        plant(SceneryKind::FloraPine, 508.0, 490.0),
        // Back row: EVERY other species, spread wide enough that none hides behind the
        // front row — the lineup is the accept/reject frame, and a species it cannot show is
        // a species nobody reviewed (the fruit tree was missing entirely). Since F7 every
        // tree species draws from the ladder at its rung for THIS eye; the bush alone is
        // the statics bake's.
        plant(SceneryKind::Poplar, 464.0, 516.0),
        plant(SceneryKind::Pine, 476.0, 516.0),
        plant(SceneryKind::FruitTree, 505.0, 512.0),
        plant(SceneryKind::Bush, 515.0, 510.0),
    ];

    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_v,
        &ground_i,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
    );
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;

    // High enough that the mid-field rise cannot hide the short species (fruit, bush).
    let eye = [498.0, ground(498.0, 452.0) + 8.0, 452.0];
    let look = [499.0, ground(499.0, 500.0) + 7.0, 500.0];
    renderer.shadow_focus = Some(look);
    // The battlefield oak draws from the instanced LOD ladder, not the statics bake — the
    // look gate submits it the way the battle frame does, at the rung this camera distance
    // picks.
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    // The leaf atlas, exactly as the battle binds it (Drzewa 3.0 PR6) — without it the card
    // canopy renders against the white no-op texel and every card is a solid rectangle.
    let (foliage_color, foliage_normal) = scene_build::foliage_atlas_paint::foliage_atlas_chains();
    renderer.set_foliage_atlas(&ctx, &foliage_color, Some(&foliage_normal));
    renderer.set_bark_textures(&ctx, &scene_build::foliage_atlas_paint::bark_texture_layers());
    let mut lod_state = scene_build::tree_lod::TreeLodState::default();
    let tree_frame = renderer_api::RenderFrame {
        objects: scene_build::tree_lod::tree_frame_objects(
            &battlefield.scenery,
            &battlefield.static_cover,
            &[],
            glam::Vec3::from_array(eye),
            &mut lod_state,
        ),
        ..renderer_api::RenderFrame::default()
    };
    renderer.set_render_frame(&ctx, &tree_frame);
    let camera = Camera { eye, target: look, vertical_fov_degrees: 55.0 };
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        width as f32 / height as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
    let pixels = target.read_rgba8(&ctx)?;
    let path = "target/flora_lineup.png";
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("wrote {path}");
    Ok(())
}
