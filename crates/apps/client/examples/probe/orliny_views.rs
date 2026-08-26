use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, grass_card_dressing_mesh,
    grass_frame_objects, grass_species_meshes, terrain_material_set_for,
};
use renderer_api::RenderFrame;
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// Render Orliny Pereval offscreen from its landmark viewpoints — the pass approach up the
/// serpentine, the hamlet on the col, the crest walk onto the Oryol summit, the Dolina lane
/// under the wall, the defile slot — plus the summit again in the alpenglow look. One PNG
/// per view under `target/`: `cargo run -p client --example probe -- orliny_views`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1280u32;
    let height = 720u32;

    let battlefield = map_forge::battlefield(terrain::MapId::OrlinyPereval);
    let ((ground_vertices, ground_indices), (vertices, indices)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let materials = terrain_material_set_for(terrain::MapId::OrlinyPereval);
    let ground =
        |x: f32, z: f32| -> f32 { battlefield.heightmap.sample_height(x, z).unwrap_or(10.0) };

    struct View {
        name: &'static str,
        eye: [f32; 3],
        target: [f32; 3],
        lighting: SceneLighting,
        sky: (f64, f64, f64),
    }
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];
    let clear_sky = (0.52, 0.63, 0.78);
    let views = [
        View {
            name: "orliny_pass_approach",
            eye: at(500.0, 4.0, 220.0),
            target: at(500.0, 8.0, 500.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: clear_sky,
        },
        View {
            name: "orliny_hamlet",
            eye: at(460.0, 4.0, 440.0),
            target: at(545.0, 4.0, 505.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: clear_sky,
        },
        View {
            name: "orliny_crest_walk",
            eye: at(585.0, 3.5, 492.0),
            target: at(680.0, 8.0, 500.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: clear_sky,
        },
        View {
            name: "orliny_dolina_lane",
            eye: at(195.0, 4.0, 260.0),
            target: at(280.0, 20.0, 500.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: clear_sky,
        },
        View {
            name: "orliny_defile",
            eye: at(848.0, 3.5, 368.0),
            target: at(838.0, 6.0, 500.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: clear_sky,
        },
        View {
            name: "orliny_pine_belt",
            eye: at(430.0, 3.0, 395.0),
            target: at(560.0, 12.0, 445.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: clear_sky,
        },
        View {
            name: "orliny_summit_alpenglow",
            eye: at(680.0, 5.0, 500.0),
            target: at(340.0, 12.0, 500.0),
            lighting: SceneLighting::prokhorovka_golden_evening(),
            sky: (0.82, 0.60, 0.42),
        },
    ];

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_vertices,
        &ground_indices,
        &ground_maps,
        &materials,
    );
    let (dressing_v, dressing_i) = grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
    renderer.scene_time_s = 12.0;
    for (handle, mesh) in grass_species_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }

    for view in &views {
        // The near-field grass ring the live client conjures around the camera each frame.
        let grass = grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water_view(),
            &battlefield.static_cover,
            &ground_maps,
            &materials,
            glam::Vec3::from_array(view.eye),
        );
        renderer.set_render_frame(&ctx, &RenderFrame { objects: grass, ..RenderFrame::default() });
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
        renderer.shadow_focus = Some(view.target);

        let camera = Camera { eye: view.eye, target: view.target, vertical_fov_degrees: 55.0 };
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/{}.png", view.name);
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
