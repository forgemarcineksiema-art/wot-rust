//! Review renders of Ostrogorsk (urban-map program PR-15) — the look-review artifacts:
//!   `target/ostrogorsk_canyon.png`  — a street canyon at tank-eye level, down market lane.
//!   `target/ostrogorsk_square.png`  — the church square from the boulevard approach.
//!   `target/ostrogorsk_berm.png`    — the rail berm and the level crossing from the fields.
//!   `target/ostrogorsk_avenue.png`  — the imported broadleaf rows from inside the boulevard.
//! `cargo run -p client --example probe -- ostrogorsk_views`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1280u32, 720u32);
    let battlefield = map_forge::battlefield(terrain::MapId::Ostrogorsk);
    let born = terrain::initial_cover_phase_bytes(&battlefield.static_cover);
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &born);
    let ground_maps = bake_terrain_ground_maps(&battlefield);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    renderer.set_battlefield_ground(
        &ctx,
        &ground_v,
        &ground_i,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::Ostrogorsk),
    );
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;

    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
    let views = [
        // Down the market-lane canyon at tank-eye height: tenement walls both sides.
        (
            "canyon",
            [150.0, ground(150.0, 446.0) + 2.4, 446.0],
            [400.0, ground(400.0, 446.0) + 2.0, 446.0],
        ),
        // The church square from the boulevard approach.
        (
            "square",
            [330.0, ground(330.0, 480.0) + 5.0, 480.0],
            [252.0, ground(252.0, 500.0) + 6.0, 500.0],
        ),
        // The berm and the level crossing from the east fields, elevators at the back.
        (
            "berm",
            [920.0, ground(920.0, 430.0) + 6.0, 430.0],
            [830.0, ground(830.0, 500.0) + 5.0, 500.0],
        ),
        // Down the boulevard's imported broadleaf rows: this frame is the FL-5 look gate and
        // deliberately starts before the first pair so the authored spacing reads at tank height.
        (
            "avenue",
            [460.0, ground(460.0, 230.0) + 3.0, 230.0],
            [460.0, ground(460.0, 500.0) + 2.5, 500.0],
        ),
    ];
    for (label, eye, look) in views {
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
        let path = format!("target/ostrogorsk_{label}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
    }
    Ok(())
}
