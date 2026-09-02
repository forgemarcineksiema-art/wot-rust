//! Runtime gate (Świat 2.0 — procedural-only): hot 1080p frame throughput for Ostrogorsk's
//! full battlefield-oak scatter (the instanced LOD ladder) against the same views with only
//! those trees removed. Two views, both gated: the LINEUP prices the scatter at range, the
//! UNDER-CROWN view noses the camera against an oak so the Near-rung canopy fills the frame —
//! the only stance that prices fill, which is what kills the MX330 (Drzewa 3.0 PR1). Each view
//! leaves `target/flora_frame_<view>.png` so the number always stands behind a reviewable frame.
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
    without_trees
        .scenery
        .retain(|instance| scene_build::tree_lod::ladder_species(instance.kind).is_none());
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
        "scene: Ostrogorsk, {WIDTH}x{HEIGHT}, {tree_count} ladder trees (every planted \
         species, F7), full {} vertices vs baseline {}",
        full_vertices.len(),
        baseline_vertices.len()
    );
    // A frame-time probe measures the shipped picture, not the review one: the window ships 1x
    // MSAA on every adapter, so building this scene at the review count would price fill the
    // player never pays.
    let target = OffscreenTarget::new(&ctx, WIDTH, HEIGHT)?;
    let mut baseline =
        SceneRenderer::for_offscreen_as_shipped(&ctx, &baseline_vertices, &baseline_indices)?;
    let mut flora = SceneRenderer::for_offscreen_as_shipped(&ctx, &full_vertices, &full_indices)?;
    println!("sample count: {}x MSAA (shipped; review images use 4x)", flora.sample_count());
    configure_renderer(&ctx, &mut baseline, &ground_vertices, &ground_indices, &ground_maps);
    configure_renderer(&ctx, &mut flora, &ground_vertices, &ground_indices, &ground_maps);

    // Battlefield oaks draw from the instanced LOD ladder, not the statics bake, so the flora
    // side has to submit them the way the battle frame does — otherwise this probe measures an
    // empty map and reports a delta of zero. The baseline keeps submitting nothing: that IS
    // the A/B.
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        flora.register_mesh(&ctx, handle, &mesh);
    }
    // The leaf atlas rides on BOTH renderers, exactly as the battle binds it — the baseline
    // must not win its A/B by skipping a bind the real frame always pays.
    let (foliage_color, foliage_normal) = scene_build::foliage_atlas_paint::foliage_atlas_chains();
    baseline.set_foliage_atlas(&ctx, &foliage_color, Some(&foliage_normal));
    flora.set_foliage_atlas(&ctx, &foliage_color, Some(&foliage_normal));

    let ground = |x: f32, z: f32| full.heightmap.sample_height(x, z).unwrap_or(0.0);
    // The under-crown worst case: nose against a battlefield oak, the Near-rung crown filling
    // the frame. The lineup view prices the scatter at range; only this view prices FILL —
    // without it, every canopy budget is a guess (Drzewa 3.0 PR1).
    let oak = full
        .scenery
        .iter()
        .find(|instance| instance.kind == terrain::SceneryKind::Oak)
        .expect("Ostrogorsk scatters battlefield oaks");
    let oak_base = glam::Vec3::from_array(oak.position);
    let crown_center = oak_base + glam::Vec3::Y * 13.0 * oak.scale;
    let approach = oak_base + glam::Vec3::new(7.0, 0.0, 0.0);
    let views = [
        (
            "lineup",
            Camera {
                eye: [460.0, ground(460.0, 230.0) + 3.0, 230.0],
                target: [460.0, ground(460.0, 500.0) + 2.5, 500.0],
                vertical_fov_degrees: 55.0,
            },
        ),
        (
            "under-crown",
            Camera {
                eye: [approach.x, ground(approach.x, approach.z) + 1.8, approach.z],
                target: crown_center.to_array(),
                vertical_fov_degrees: 55.0,
            },
        ),
    ];

    let projection = CameraProjectionPolicy::webgpu_default();
    let mut worst_ms = f64::MIN;
    for (name, camera) in views {
        let view_proj = view_projection_matrix(
            &camera,
            WIDTH as f32 / HEIGHT as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        baseline.shadow_focus = Some(camera.target);
        flora.shadow_focus = Some(camera.target);
        // The ladder picks its rung from THIS view's eye — under the crown that is the Near
        // bake, which is the whole point of the second view.
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
        warm(&ctx, &target, &mut baseline, view_proj, camera.eye)?;
        warm(&ctx, &target, &mut flora, view_proj, camera.eye)?;
        // One reviewable frame per view: the fill number is only trustworthy if an eye can
        // confirm the crown actually fills the frame (and not a wall the camera backed into).
        write_probe_png(name, &target.read_rgba8(&ctx)?)?;

        let mut baseline_samples = Vec::with_capacity(SAMPLE_ROUNDS);
        let mut flora_samples = Vec::with_capacity(SAMPLE_ROUNDS);
        for round in 0..SAMPLE_ROUNDS {
            if round.is_multiple_of(2) {
                baseline_samples.push(measure(
                    &ctx,
                    &target,
                    &mut baseline,
                    view_proj,
                    camera.eye,
                )?);
                flora_samples.push(measure(&ctx, &target, &mut flora, view_proj, camera.eye)?);
            } else {
                flora_samples.push(measure(&ctx, &target, &mut flora, view_proj, camera.eye)?);
                baseline_samples.push(measure(
                    &ctx,
                    &target,
                    &mut baseline,
                    view_proj,
                    camera.eye,
                )?);
            }
        }
        baseline_samples.sort_by(f64::total_cmp);
        flora_samples.sort_by(f64::total_cmp);
        let baseline_ms = baseline_samples[SAMPLE_ROUNDS / 2];
        let flora_ms = flora_samples[SAMPLE_ROUNDS / 2];
        let overhead_ms = flora_ms - baseline_ms;
        worst_ms = worst_ms.max(flora_ms);
        println!("[{name}] baseline median: {baseline_ms:.3} ms/frame  {baseline_samples:?}");
        println!("[{name}] full flora median: {flora_ms:.3} ms/frame  {flora_samples:?}");
        println!(
            "[{name}] ladder-tree delta: {overhead_ms:+.3} ms/frame; 60 FPS gate \
             ({SIXTY_FPS_FRAME_MS:.3} ms): {}",
            if flora_ms <= SIXTY_FPS_FRAME_MS { "PASS" } else { "FAIL" }
        );
    }
    println!(
        "worst view median: {worst_ms:.3} ms/frame; 60 FPS gate: {}",
        if worst_ms <= SIXTY_FPS_FRAME_MS { "PASS" } else { "FAIL" }
    );
    Ok(())
}

/// The review frame the numbers stand behind: `target/flora_frame_<view>.png`.
fn write_probe_png(view: &str, pixels: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let path = format!("target/flora_frame_{view}.png");
    let file = std::fs::File::create(&path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    println!("wrote {path}");
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
    renderer: &mut SceneRenderer,
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
    renderer: &mut SceneRenderer,
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
