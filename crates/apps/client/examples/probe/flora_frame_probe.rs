//! Runtime gate (Świat 2.0 — procedural-only): hot 1080p frame throughput for Ostrogorsk's
//! full battlefield-oak scatter (the instanced LOD ladder) against the same view with only
//! those trees removed.
//!
//! `cargo run -p client --release --example probe -- flora_frame_probe`

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

const WIDTH: u32 = 1_920;
const HEIGHT: u32 = 1_080;
const WARMUP_FRAMES: usize = 24;
const SAMPLE_FRAMES: usize = 120;
const SAMPLE_ROUNDS: usize = 3;
const SIXTY_FPS_FRAME_MS: f64 = 1_000.0 / 60.0;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let full = map_forge::battlefield(terrain::MapId::Ostrogorsk);
    let mut without_trees = full.clone();
    without_trees.scenery.retain(|instance| instance.kind != terrain::SceneryKind::Oak);
    let tree_count = full.scenery.len() - without_trees.scenery.len();
    let born = terrain::initial_cover_phase_bytes(&full.static_cover);
    let ((ground_vertices, ground_indices), (full_vertices, full_indices)) =
        battlefield_ground_and_statics_meshes(&full, &born);
    let (_, (baseline_vertices, baseline_indices)) =
        battlefield_ground_and_statics_meshes(&without_trees, &born);
    let ground_maps = bake_terrain_ground_maps(&full);

    let ctx = GpuContext::headless()?;
    let info = ctx.adapter.get_info();
    println!(
        "adapter: {} ({:?}, {:?}, driver {})",
        info.name, info.backend, info.device_type, info.driver
    );
    println!(
        "scene: Ostrogorsk, {WIDTH}x{HEIGHT}, {tree_count} battlefield oaks, \
         full {} vertices vs baseline {}",
        full_vertices.len(),
        baseline_vertices.len()
    );
    let target = OffscreenTarget::new(&ctx, WIDTH, HEIGHT)?;
    let mut baseline = SceneRenderer::for_offscreen(&ctx, &baseline_vertices, &baseline_indices)?;
    let mut flora = SceneRenderer::for_offscreen(&ctx, &full_vertices, &full_indices)?;
    configure_renderer(&ctx, &mut baseline, &ground_vertices, &ground_indices, &ground_maps);
    configure_renderer(&ctx, &mut flora, &ground_vertices, &ground_indices, &ground_maps);

    let ground = |x: f32, z: f32| full.heightmap.sample_height(x, z).unwrap_or(0.0);
    let camera = Camera {
        eye: [460.0, ground(460.0, 230.0) + 3.0, 230.0],
        target: [460.0, ground(460.0, 500.0) + 2.5, 500.0],
        vertical_fov_degrees: 55.0,
    };
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        WIDTH as f32 / HEIGHT as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );
    baseline.shadow_focus = Some(camera.target);
    flora.shadow_focus = Some(camera.target);
    // Battlefield oaks draw from the instanced LOD ladder, not the statics bake, so the flora
    // side has to submit them the way the battle frame does — otherwise this probe measures an
    // empty map and reports a delta of zero. The baseline keeps submitting nothing: that IS
    // the A/B.
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        flora.register_mesh(&ctx, handle, &mesh);
    }
    let mut lod_state = scene_build::tree_lod::TreeLodState::default();
    let tree_frame = renderer_api::RenderFrame {
        objects: scene_build::tree_lod::tree_frame_objects(
            &full.scenery,
            &full.static_cover,
            &born,
            glam::Vec3::from_array(camera.eye),
            &mut lod_state,
        ),
        ..renderer_api::RenderFrame::default()
    };
    flora.set_render_frame(&ctx, &tree_frame);
    warm(&ctx, &target, &baseline, view_proj, camera.eye)?;
    warm(&ctx, &target, &flora, view_proj, camera.eye)?;

    let mut baseline_samples = Vec::with_capacity(SAMPLE_ROUNDS);
    let mut flora_samples = Vec::with_capacity(SAMPLE_ROUNDS);
    for round in 0..SAMPLE_ROUNDS {
        if round.is_multiple_of(2) {
            baseline_samples.push(measure(&ctx, &target, &baseline, view_proj, camera.eye)?);
            flora_samples.push(measure(&ctx, &target, &flora, view_proj, camera.eye)?);
        } else {
            flora_samples.push(measure(&ctx, &target, &flora, view_proj, camera.eye)?);
            baseline_samples.push(measure(&ctx, &target, &baseline, view_proj, camera.eye)?);
        }
    }
    baseline_samples.sort_by(f64::total_cmp);
    flora_samples.sort_by(f64::total_cmp);
    let baseline_ms = baseline_samples[SAMPLE_ROUNDS / 2];
    let flora_ms = flora_samples[SAMPLE_ROUNDS / 2];
    let overhead_ms = flora_ms - baseline_ms;
    println!("baseline median: {baseline_ms:.3} ms/frame  {baseline_samples:?}");
    println!("full flora median: {flora_ms:.3} ms/frame  {flora_samples:?}");
    println!(
        "battlefield-oak delta: {overhead_ms:+.3} ms/frame; 60 FPS gate ({SIXTY_FPS_FRAME_MS:.3} ms): {}",
        if flora_ms <= SIXTY_FPS_FRAME_MS { "PASS" } else { "FAIL" }
    );
    Ok(())
}

fn configure_renderer(
    ctx: &GpuContext,
    renderer: &mut SceneRenderer,
    ground_vertices: &[renderer_api::SceneVertex],
    ground_indices: &[u32],
    ground_maps: &renderer_api::TerrainGroundMaps,
) {
    renderer.set_battlefield_ground(
        ctx,
        ground_vertices,
        ground_indices,
        ground_maps,
        &terrain_material_set_for(terrain::MapId::Ostrogorsk),
    );
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;
}

fn warm(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    renderer: &SceneRenderer,
    view_proj: [[f32; 4]; 4],
    eye: [f32; 3],
) -> Result<(), renderer_api::RenderError> {
    for _ in 0..WARMUP_FRAMES {
        renderer.render(ctx, target.render_target(), view_proj, eye)?;
    }
    target.read_rgba8(ctx)?;
    Ok(())
}

fn measure(
    ctx: &GpuContext,
    target: &OffscreenTarget,
    renderer: &SceneRenderer,
    view_proj: [[f32; 4]; 4],
    eye: [f32; 3],
) -> Result<f64, renderer_api::RenderError> {
    let start = std::time::Instant::now();
    for _ in 0..SAMPLE_FRAMES {
        renderer.render(ctx, target.render_target(), view_proj, eye)?;
    }
    target.read_rgba8(ctx)?;
    Ok(start.elapsed().as_secs_f64() * 1_000.0 / SAMPLE_FRAMES as f64)
}
