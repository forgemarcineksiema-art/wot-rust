use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, battlefield_water_mesh,
    terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{BattlefieldMap, MapId};

/// Render the map border from a tank's point of view — standing at the red line looking out,
/// and elevated along the line — on both authored maps. The review tool for the border
/// apron: the world past the line must read as more of the same land melting into the haze,
/// not a different game. One PNG per map x view under `target/`:
/// `cargo run -p client --example probe -- border_probe`
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let width = 1280u32;
    let height = 720u32;

    let maps: [(&str, BattlefieldMap, MapId); 2] = [
        (
            "prokhorovka",
            map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2),
            MapId::ProkhorovkaHill252_2,
        ),
        ("bystra", map_forge::battlefield(terrain::MapId::BystraValley), MapId::BystraValley),
    ];

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;

    for (name, battlefield, map_id) in maps {
        let ((ground_vertices, ground_indices), (statics_vertices, statics_indices)) =
            battlefield_ground_and_statics_meshes(&battlefield, &[]);
        let ground_maps = bake_terrain_ground_maps(&battlefield);
        let (water_vertices, water_indices) = battlefield_water_mesh(&battlefield);

        let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_vertices, &statics_indices)?;
        renderer.set_battlefield_ground(
            &ctx,
            &ground_vertices,
            &ground_indices,
            &ground_maps,
            &terrain_material_set_for(map_id),
        );
        renderer.set_water(&ctx, &water_vertices, &water_indices);
        renderer.scene_time_s = 180.0;
        renderer.scene_lighting = match map_id {
            MapId::BystraValley => SceneLighting::bystra_clear_afternoon(),
            _ => SceneLighting::battlefield_default(),
        };

        let [extent_x, extent_z] = battlefield.heightmap.extent_m();
        let ground = |x: f32, z: f32| {
            battlefield
                .heightmap
                .sample_height(x.clamp(0.0, extent_x), z.clamp(0.0, extent_z))
                .unwrap_or(0.0)
        };
        // Tank-eye at the red line looking OUT; a step back looking ALONG the line; and an
        // elevated look across the seam.
        let views: [(&str, [f32; 3], [f32; 3]); 3] = [
            (
                "out",
                [extent_x * 0.35, ground(extent_x * 0.35, extent_z - 5.0) + 2.4, extent_z - 5.0],
                [extent_x * 0.35, ground(extent_x * 0.35, extent_z - 5.0) + 1.0, extent_z + 120.0],
            ),
            (
                "along",
                [extent_x * 0.25, ground(extent_x * 0.25, extent_z - 25.0) + 2.4, extent_z - 25.0],
                [extent_x * 0.55, ground(extent_x * 0.55, extent_z) + 1.0, extent_z + 30.0],
            ),
            (
                "high",
                [extent_x * 0.5, ground(extent_x * 0.5, extent_z - 160.0) + 45.0, extent_z - 160.0],
                [extent_x * 0.5, 5.0, extent_z + 320.0],
            ),
        ];

        for (view_name, eye, at) in views {
            renderer.shadow_focus = Some([eye[0], 0.0, eye[2]]);
            let camera = Camera { eye, target: at, vertical_fov_degrees: 55.0 };
            let projection = CameraProjectionPolicy::webgpu_default();
            let view_proj = view_projection_matrix(
                &camera,
                width as f32 / height as f32,
                projection.near_plane_m(),
                projection.far_plane_m(),
            );
            renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;

            let pixels = target.read_rgba8(&ctx)?;
            let path = format!("target/border_{name}_{view_name}.png");
            let file = File::create(&path)?;
            let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(&pixels)?;
            println!("wrote {path}");
        }
    }
    Ok(())
}
