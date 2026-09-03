//! The authored oak on its own (route 2, trees as data): one tree on Prokhorovka's ground,
//! four eyes — the whole silhouette from a tank's eye at 22 m, a close approach at 9 m, the
//! Mid rung at 90 m and the impostor at 220 m — so every rung of the ladder is judged on the
//! same individual under the same light. Four PNGs, `target/oak_<view>.png`:
//! `cargo run -p client --example probe -- oak_probe`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{SceneryInstance, SceneryKind};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1600u32, 900u32);
    let mut battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    battlefield.static_cover.clear();
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
    let (tx, tz) = (500.0_f32, 500.0_f32);
    battlefield.scenery = vec![SceneryInstance {
        kind: SceneryKind::Oak,
        position: [tx, ground(tx, tz), tz],
        yaw_rad: 0.6,
        scale: 1.0,
    }];

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
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);

    let base_y = ground(tx, tz);
    // (name, distance from the trunk, eye height above ground, aim height on the tree)
    let views = [
        ("whole_22m", 22.0_f32, 2.2_f32, 8.5_f32),
        ("near_100m", 100.0, 2.2, 8.0),
        ("mid_140m", 140.0, 2.2, 8.0),
        ("mid_280m", 280.0, 2.2, 8.0),
        // Inside the 300 m cross-fade band: both rungs, screen-door interleaved.
        ("fade_300m", 300.0, 2.2, 8.0),
        ("impostor_350m", 350.0, 2.2, 8.0),
    ];
    let projection = CameraProjectionPolicy::webgpu_default();
    for (name, distance, eye_up, aim_up) in views {
        // The eye stands south of the tree, looking north into the light the profile sets.
        let ex = tx - 0.35 * distance;
        let ez = tz - 0.94 * distance;
        let eye = [ex, ground(ex, ez) + eye_up, ez];
        let look = [tx, base_y + aim_up, tz];
        renderer.shadow_focus = Some(look);
        let mut lod_state = scene_build::tree_lod::TreeLodState::default();
        let tree_frame = renderer_api::RenderFrame {
            objects: scene_build::tree_lod::tree_frame_objects_with_backdrop(
                &battlefield,
                &[],
                glam::Vec3::from_array(eye),
                &mut lod_state,
            ),
            ..renderer_api::RenderFrame::default()
        };
        renderer.set_render_frame(&ctx, &tree_frame);
        let camera = Camera { eye, target: look, vertical_fov_degrees: 45.0 };
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/oak_{name}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path} ({:?})", lod_state.levels());
    }
    Ok(())
}
