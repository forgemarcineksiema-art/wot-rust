use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, battlefield_water_mesh,
    grass_card_dressing_mesh, grass_frame_objects, grass_species_meshes, terrain_material_set_for,
};
use renderer_api::RenderFrame;
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// Render the Bystra valley offscreen from its landmark viewpoints — the stone bridge over
/// the river, a town lane under the gable roofs, the valley panorama from the Mill Hill —
/// plus the same lane soaked in the rain look. One PNG per view under `target/`:
/// `cargo run -p client --example probe -- bystra_views`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1280u32;
    let height = 720u32;

    let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);
    let ((ground_vertices, ground_indices), (vertices, indices)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let materials = terrain_material_set_for(terrain::MapId::BystraValley);
    let (water_vertices, water_indices) = battlefield_water_mesh(&battlefield);
    let ground =
        |x: f32, z: f32| -> f32 { battlefield.heightmap.sample_height(x, z).unwrap_or(5.0) };

    // (name, eye xz + height above ground, target xz + height, lighting, rain, wetness)
    struct View {
        name: &'static str,
        eye: [f32; 3],
        target: [f32; 3],
        lighting: SceneLighting,
        sky: (f64, f64, f64),
        rain: f32,
        wetness: f32,
    }
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];
    let views = [
        View {
            name: "bystra_bridge",
            eye: at(500.0, 6.0, 460.0),
            target: at(600.0, 2.0, 505.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: (0.62, 0.66, 0.72),
            rain: 0.0,
            wetness: 0.0,
        },
        View {
            name: "bystra_town_lane",
            eye: at(680.0, 5.0, 470.0),
            target: at(760.0, 3.0, 510.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: (0.62, 0.66, 0.72),
            rain: 0.0,
            wetness: 0.0,
        },
        View {
            name: "bystra_panorama",
            eye: at(250.0, 14.0, 500.0),
            target: at(700.0, 4.0, 500.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: (0.62, 0.66, 0.72),
            rain: 0.0,
            wetness: 0.0,
        },
        View {
            name: "bystra_wall_close",
            eye: at(706.0, 2.5, 498.0),
            target: at(725.0, 3.0, 512.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: (0.62, 0.66, 0.72),
            rain: 0.0,
            wetness: 0.0,
        },
        View {
            // Świat 2.0 PR 5: the field hedgerow as a szpaler — undergrowth, staggered boles,
            // an undulating crown rising to its 17 m box, not the old dark slab.
            name: "bystra_treeline",
            eye: at(300.0, 10.0, 315.0),
            target: at(300.0, 8.0, 240.0),
            lighting: SceneLighting::bystra_clear_afternoon(),
            sky: (0.62, 0.66, 0.72),
            rain: 0.0,
            wetness: 0.0,
        },
        View {
            name: "bystra_town_rain",
            eye: at(680.0, 5.0, 470.0),
            target: at(760.0, 3.0, 510.0),
            lighting: SceneLighting::bystra_rain(),
            sky: (0.42, 0.46, 0.50),
            rain: 1.0,
            wetness: 1.0,
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
    renderer.set_water(&ctx, &water_vertices, &water_indices);
    let (dressing_v, dressing_i) = grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
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
        renderer.rain_intensity = view.rain;
        renderer.wetness = view.wetness;
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
