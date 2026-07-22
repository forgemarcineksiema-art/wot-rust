//! THE Flora 2.0 look gate (FL-4): imported CC0 foliage side by side with the baked
//! procedural trees, one frame, same light — the render the per-species accept/reject
//! verdict is made on. Front row: imported tree, pine, bush (textured, alpha-cut, atlas-fed).
//! Back row: procedural Oak, Pine, Bush (trees 2.0). One PNG:
//! `cargo run -p client --example flora_probe`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{SceneryInstance, SceneryKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    renderer.set_foliage_atlas(&ctx, &flora.atlas_rgba, flora.atlas_size, flora.atlas_size);
    renderer.set_battlefield_ground(
        &ctx,
        &ground_v,
        &ground_i,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
    );
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;

    let eye = [498.0, ground(498.0, 478.0) + 3.2, 478.0];
    let look = [499.0, ground(499.0, 500.0) + 3.5, 500.0];
    renderer.shadow_focus = Some(look);
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
