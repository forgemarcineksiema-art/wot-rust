//! The meadow at a driver's eye height (Jedna Trawa): three frames down the SAME sightline,
//! so the near tufts, the far tufts and the ground that carries the meadow can be judged
//! against each other instead of one at a time.
//!
//! This exists because a review camera looking across a valley cannot answer the question
//! that actually matters — "does the far grass read as GRASS, or as cones standing in a
//! field?" — and that question was answered wrong once already by looking at the wrong frame.
//!
//! Run: `cargo run -p client --release --example probe -- grass_costumes`

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, battlefield_water_mesh,
    grass_card_dressing_mesh, grass_frame_objects, grass_species_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, RenderFrame, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1600u32, 900u32);
    let map = terrain::MapId::ProkhorovkaHill252_2;
    let battlefield = map_forge::battlefield(map);
    let materials = terrain_material_set_for(map);
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let (water_v, water_i) = battlefield_water_mesh(&battlefield);
    let (dressing_v, dressing_i) = grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    renderer.set_battlefield_ground(&ctx, &ground_v, &ground_i, &ground_maps, &materials);
    renderer.set_water(&ctx, &water_v, &water_i);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
    renderer.set_foliage_atlas(&ctx, &scene_build::flora_pack::flora_catalog().atlas_mips);
    for (handle, mesh) in grass_species_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    renderer.scene_time_s = 12.0;

    // Open meadow on the hill's east slope, looking along it — the same ground the live
    // frame that opened this program was shot on.
    let anchor = glam::Vec3::new(520.0, 0.0, 470.0);
    let forward = glam::Vec3::new(0.82, 0.0, 0.57).normalize();
    // Commander's eye height: what the player actually looks from, not a review helicopter.
    let eye_h = 2.4;

    // Three stations down one sightline. The MIDDLE one is the interesting frame: it stands
    // just past the costume hand-off, where the far tufts are the biggest they ever get on
    // screen — if they read as cones anywhere, it is here.
    let stations: [(&str, f32); 3] = [("near", 0.0), ("handoff", 34.0), ("far", 90.0)];
    for (name, back) in stations {
        let ground = battlefield.heightmap.sample_height(anchor.x, anchor.z).unwrap_or(0.0);
        let eye = glam::Vec3::new(
            anchor.x - forward.x * back,
            ground + eye_h,
            anchor.z - forward.z * back,
        );
        let target_point = eye + forward * 40.0 - glam::Vec3::Y * 4.0;
        let camera =
            Camera { eye: eye.into(), target: target_point.into(), vertical_fov_degrees: 55.0 };
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        let grass = grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &battlefield.static_cover,
            &ground_maps,
            &materials,
            eye,
        );
        renderer.set_render_frame(&ctx, &RenderFrame { objects: grass, ..Default::default() });
        renderer.shadow_focus = Some(target_point.into());
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/grass_{name}.png");
        let file = std::fs::File::create(&path)?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
