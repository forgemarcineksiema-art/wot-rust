//! THE flora look gate (FL-4): imported foliage side by side with the baked procedural
//! trees, one frame, same light — the render the accept/reject verdict is made on. Front
//! row: the hero oak behind `FloraTree` (photoscan-textured, alpha-cut, atlas-fed) and the
//! two retired slots, which now bake to nothing on purpose — their emptiness beside the
//! procedural back row (Oak, Pine, Bush) IS the state of the family. One PNG:
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
        // Front row: the imported CC0 set.
        plant(SceneryKind::FloraTree, 488.0, 492.0),
        plant(SceneryKind::FloraPine, 500.0, 492.0),
        plant(SceneryKind::FloraBush, 510.0, 492.0),
        // Back row: the procedural species they compete with.
        plant(SceneryKind::Oak, 488.0, 510.0),
        plant(SceneryKind::Pine, 500.0, 510.0),
        plant(SceneryKind::Bush, 510.0, 510.0),
    ];

    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    let flora = scene_build::flora_pack::flora_catalog();
    renderer.set_foliage_atlas(&ctx, &flora.atlas_mips);
    renderer.set_battlefield_ground(
        &ctx,
        &ground_v,
        &ground_i,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
    );
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;

    // Pulled back and tilted up for the hero oak: at ~22 m it is three times the stylized
    // assets this shot was framed for, and a look gate that crops the canopy judges nothing.
    let eye = [498.0, ground(498.0, 455.0) + 4.0, 455.0];
    let look = [499.0, ground(499.0, 500.0) + 11.0, 500.0];
    renderer.shadow_focus = Some(look);
    // The hero oak draws from the instanced LOD ladder now, not the statics bake — the look
    // gate submits it the way the battle frame does, at the rung this camera distance picks.
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
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
