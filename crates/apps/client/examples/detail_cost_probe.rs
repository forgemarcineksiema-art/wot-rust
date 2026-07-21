//! Buy-back measurement probe (Żywy Step): renders the canonical midfield view offscreen in a
//! tight loop under two shader-detail masks and reports wall-clock per frame — the honest
//! apples-to-apples GPU cost of a feature on THIS machine (the min-spec is the dev box).
//! Run once per mask with the env set by the caller:
//! `WOT_GPU_DETAIL=<mask> cargo run --release -p client --example detail_cost_probe`

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, battlefield_water_mesh,
    prokhorovka_review_views, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mask = std::env::var("WOT_GPU_DETAIL").unwrap_or_else(|_| "unset".to_string());
    let (width, height) = (1920u32, 1080u32);

    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);
    let (water_v, water_i) = battlefield_water_mesh(&battlefield);
    let view = prokhorovka_review_views(&battlefield)
        .into_iter()
        .find(|view| view.name.contains("midfield"))
        .expect("midfield review view");

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let camera = Camera { eye: view.eye, target: view.target, vertical_fov_degrees: 55.0 };
    let projection = CameraProjectionPolicy::webgpu_default();
    let view_proj = view_projection_matrix(
        &camera,
        width as f32 / height as f32,
        projection.near_plane_m(),
        projection.far_plane_m(),
    );

    {
        // The mask resolves at renderer construction from WOT_GPU_DETAIL, set by the caller —
        // one process per mask is exactly the honest A/B: same scene, only the bits differ.
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
        renderer.set_battlefield_ground(
            &ctx,
            &ground_v,
            &ground_i,
            &ground_maps,
            &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
        );
        renderer.set_water(&ctx, &water_v, &water_i);
        {
            let (dressing_v, dressing_i) = client::grass_card_dressing_mesh(
                &battlefield,
                &ground_maps,
                &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
            );
            renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
        }
        renderer.scene_lighting = view.lighting;
        renderer.set_outdoor_sky(view.sky.0, view.sky.1, view.sky.2);
        renderer.shadow_focus = Some(view.target);
        // Warm up (pipeline caches, first-frame work), then measure a tight loop. The read
        // back at the end forces the GPU queue to drain so wall clock covers real GPU time.
        for _ in 0..20 {
            renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        }
        let _ = target.read_rgba8(&ctx)?;
        let frames = 200;
        let start = std::time::Instant::now();
        for _ in 0..frames {
            renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        }
        let _ = target.read_rgba8(&ctx)?;
        let elapsed = start.elapsed();
        println!(
            "mask {mask}: {:7.3} ms/frame ({} frames, {:?} total)",
            elapsed.as_secs_f64() * 1000.0 / frames as f64,
            frames,
            elapsed,
        );
    }
    Ok(())
}
