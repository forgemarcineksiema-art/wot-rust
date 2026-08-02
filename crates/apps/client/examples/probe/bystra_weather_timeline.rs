use std::fs::File;
use std::io::BufWriter;

use client::{
    GRASS_MESH_HANDLE, bake_terrain_ground_maps, battlefield_ground_and_statics_meshes,
    battlefield_water_mesh, grass_card_dressing_mesh, grass_frame_objects, grass_tuft_mesh,
    terrain_material_set_for,
};
use game_core::{MatchWeather, WeatherVariant};
use renderer_api::{Camera, CameraProjectionPolicy, RenderFrame, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use scene_build::weather_timeline::WeatherTimeline;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    const TIMES: [u32; 5] = [0, 150, 300, 450, 600];

    let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);
    let ((ground_vertices, ground_indices), (vertices, indices)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let materials = terrain_material_set_for(terrain::MapId::BystraValley);
    let (water_vertices, water_indices) = battlefield_water_mesh(&battlefield);
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(5.0);
    let at = |x: f32, up: f32, z: f32| [x, ground(x, z) + up, z];
    let views = [
        ("driver", at(680.0, 4.0, 470.0), at(760.0, 2.5, 510.0)),
        ("steep", at(560.0, 110.0, 350.0), at(650.0, 0.0, 510.0)),
        ("far", at(250.0, 18.0, 500.0), at(740.0, 4.0, 500.0)),
    ];
    let timeline = WeatherTimeline::new(
        terrain::MapId::BystraValley,
        MatchWeather::new(WeatherVariant::RainSqualls, 0xB7_57_AA),
    );

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, WIDTH, HEIGHT)?;
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
    renderer.register_mesh(&ctx, GRASS_MESH_HANDLE, &grass_tuft_mesh());

    for (name, eye, look) in views {
        let grass = grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &ground_maps,
            &materials,
            glam::Vec3::from_array(eye),
        );
        renderer.set_render_frame(&ctx, &RenderFrame { objects: grass, ..RenderFrame::default() });
        renderer.shadow_focus = Some(look);
        for time_s in TIMES {
            let weather = timeline.sample(time_s as f32);
            renderer.scene_time_s = time_s as f32;
            renderer.scene_lighting = weather.lighting;
            renderer.set_outdoor_sky(weather.sky.0, weather.sky.1, weather.sky.2);
            renderer.rain_intensity = weather.rain_intensity;
            renderer.wetness = weather.surface_wetness;
            renderer.puddle_fill = weather.puddle_fill;
            renderer.cloud_offset = weather.cloud_offset;
            renderer.rain_phase_s = weather.rain_phase_s;

            let camera = Camera { eye, target: look, vertical_fov_degrees: 55.0 };
            let projection = CameraProjectionPolicy::webgpu_default();
            let view_proj = view_projection_matrix(
                &camera,
                WIDTH as f32 / HEIGHT as f32,
                projection.near_plane_m(),
                projection.far_plane_m(),
            );
            renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
            write_png(
                &format!("target/bystra_weather_{name}_{time_s:03}.png"),
                WIDTH,
                HEIGHT,
                &target.read_rgba8(&ctx)?,
            )?;
        }
    }
    Ok(())
}

fn write_png(
    path: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    println!("wrote {path}");
    Ok(())
}
