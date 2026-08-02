//! Release-mode timing capture: the one-off bakes (battle-scene hitch sizes), the CPU costs the
//! render loop pays every frame (grass conjuring), and — since the "one look" policy is stated in
//! FRAMES — the frame itself.
//!
//! Run: `cargo run -p client --release --example probe -- perf_capture`
//!
//! **What the frame number is and is not.** It renders a real battle scene OFFSCREEN, so it counts
//! the GPU and CPU work of producing an image and NOT swapchain acquire, present or vsync pacing.
//! And it runs on whatever machine you are sitting at, which is not the MX330 the policy names. So
//! it is a RELATIVE instrument: run it before a change and after, on one machine, and the delta is
//! real. Reading it as a pass/fail against 60 FPS would be reading it wrong.
//!
//! Before this existed the project measured every INPUT to frame cost — vehicle triangles, HUD
//! vertices, culling, LOD, snapshot size — and never the frame. A policy whose central promise is
//! a frame rate had no frame number anywhere.

use std::time::Instant;

pub(crate) fn run() {
    let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);

    let t = Instant::now();
    let ((gv, gi), (sv, si)) = client::battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let statics_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "scene bake (ground+statics): {statics_ms:.1} ms  (ground {} v / {} i, statics {} v / {} i)",
        gv.len(),
        gi.len(),
        sv.len(),
        si.len()
    );

    let t = Instant::now();
    let maps = client::bake_terrain_ground_maps(&battlefield);
    println!("ground maps bake (splat+macro): {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);

    let t = Instant::now();
    let (wv, _wi) = client::battlefield_water_mesh(&battlefield);
    println!("water mesh: {:.1} ms ({} v)", t.elapsed().as_secs_f64() * 1000.0, wv.len());

    // The cover-collapse rebuild: what a building falling mid-battle costs on the render thread.
    let t = Instant::now();
    let states = vec![1u8; battlefield.static_cover.len()];
    let _ = client::battlefield_statics_mesh(&battlefield, &states);
    println!("statics rebuild (cover collapse): {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);

    // The PARTIAL rebuild (urban-map program PR-04): one collapsed building re-bakes only the
    // buckets its footprint touches, plus the assembly concat. This is the worker's real cost
    // per collapse on a dense map.
    let intact = vec![0u8; battlefield.static_cover.len()];
    let mut buckets = client::battlefield_statics_buckets(&battlefield, &intact, &[]);
    let collapsed = battlefield
        .static_cover
        .iter()
        .position(|cover| cover.kind == terrain::StaticCoverKind::FarmBuilding)
        .unwrap_or(0);
    let mut one_down = intact.clone();
    one_down[collapsed] = 1;
    let t = Instant::now();
    let dirty: Vec<usize> = client::statics_buckets_touched_by_cover(
        &battlefield,
        &battlefield.static_cover[collapsed],
    )
    .collect();
    for &bucket in &dirty {
        buckets[bucket] =
            client::battlefield_statics_bucket_mesh(&battlefield, &one_down, &[], bucket);
    }
    let (av, _ai) = client::assemble_statics_mesh(&buckets);
    println!(
        "statics rebuild (single collapse, {} dirty bucket(s) + assembly): {:.2} ms ({} v)",
        dirty.len(),
        t.elapsed().as_secs_f64() * 1000.0,
        av.len()
    );

    // The URBAN map (urban-map program PR-15): the dense-core numbers - the one-look FPS
    // sign-off is measured, not promised. FL-5 deliberately runs this with every accepted
    // imported-flora instance present, so the capture cannot silently benchmark the old trees.
    let city = map_forge::battlefield(terrain::MapId::Ostrogorsk);
    let imported_flora = city
        .scenery
        .iter()
        .filter(|instance| {
            matches!(
                instance.kind,
                terrain::SceneryKind::FloraTree | terrain::SceneryKind::FloraPine
            )
        })
        .count();
    let born = terrain::initial_cover_phase_bytes(&city.static_cover);
    let t = Instant::now();
    let mut city_buckets = client::battlefield_statics_buckets(&city, &born, &[]);
    let (cv, ci) = client::assemble_statics_mesh(&city_buckets);
    println!(
        "ostrogorsk statics bake ({} boxes, {imported_flora} imported flora): {:.1} ms ({} v / {} i)",
        city.static_cover.len(),
        t.elapsed().as_secs_f64() * 1000.0,
        cv.len(),
        ci.len()
    );
    let all_rubble = vec![1u8; city.static_cover.len()];
    let t = Instant::now();
    let _ = client::battlefield_statics_mesh(&city, &all_rubble);
    println!(
        "ostrogorsk statics rebuild (all-rubble): {:.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let city_collapsed = city
        .static_cover
        .iter()
        .position(|cover| cover.kind == terrain::StaticCoverKind::CityBuilding)
        .unwrap_or(0);
    let mut one_down_city = born.clone();
    one_down_city[city_collapsed] = 1;
    let t = Instant::now();
    let dirty: Vec<usize> =
        client::statics_buckets_touched_by_cover(&city, &city.static_cover[city_collapsed])
            .collect();
    for &bucket in &dirty {
        city_buckets[bucket] =
            client::battlefield_statics_bucket_mesh(&city, &one_down_city, &[], bucket);
    }
    let (av2, _) = client::assemble_statics_mesh(&city_buckets);
    println!(
        "ostrogorsk statics rebuild (single collapse, {} dirty bucket(s) + assembly): {:.2} ms ({} v)",
        dirty.len(),
        t.elapsed().as_secs_f64() * 1000.0,
        av2.len()
    );

    // Grass: the real per-frame cost, averaged hot.
    let materials = client::terrain_material_set_for(terrain::MapId::BystraValley);
    let eye = glam::Vec3::new(500.0, 8.0, 470.0);
    let mut count = 0;
    let t = Instant::now();
    const FRAMES: u32 = 300;
    for frame in 0..FRAMES {
        let wobble = glam::Vec3::new((frame as f32 * 0.1).sin() * 3.0, 0.0, frame as f32 * 0.05);
        let objects = client::grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &maps,
            &materials,
            eye + wobble,
        );
        count = objects.len();
    }
    let per_frame_us = t.elapsed().as_secs_f64() * 1.0e6 / f64::from(FRAMES);
    println!("grass conjure: {per_frame_us:.0} us/frame ({count} instances)");

    frame_time_capture();

    // A landmark bake (any style growth shows up here).
    let t = Instant::now();
    let church = world_forge::building::bake_building(
        world_forge::building::BuildingStyle::Church,
        0,
        world_forge::building::StructureForm::Intact,
    );
    println!(
        "church bake: {:.2} ms ({} tris)",
        t.elapsed().as_secs_f64() * 1000.0,
        church.triangle_count()
    );
}

/// Render a real battle scene offscreen and report the frame-time distribution.
///
/// Percentiles, not a mean: a frame budget is missed by the SLOW frames, and an average hides
/// exactly the spikes the one-look policy calls a bug.
///
/// The scene is assembled through the SAME calls `look_harness` uses for review frames — ground,
/// statics, water, grass-card dressing, the flora atlas, the grass population — so this measures
/// the picture the game actually draws rather than a stripped-down stand-in that would flatter it.
fn frame_time_capture() {
    const WARMUP: usize = 20;
    const FRAMES: usize = 180;
    let (width, height) = (1920u32, 1080u32);
    let map = terrain::MapId::BystraValley;

    let battlefield = map_forge::battlefield(map);
    let materials = client::terrain_material_set_for(map);
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        client::battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = client::bake_terrain_ground_maps(&battlefield);
    let (water_v, water_i) = client::battlefield_water_mesh(&battlefield);
    let (dressing_v, dressing_i) =
        client::grass_card_dressing_mesh(&battlefield, &ground_maps, &materials);

    let Ok(ctx) = renderer_wgpu::GpuContext::headless() else {
        println!("frame time: no headless GPU adapter — skipped");
        return;
    };
    let Ok(target) = renderer_wgpu::OffscreenTarget::new(&ctx, width, height) else {
        println!("frame time: offscreen target unavailable — skipped");
        return;
    };
    let Ok(mut renderer) =
        renderer_wgpu::SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)
    else {
        println!("frame time: scene renderer unavailable — skipped");
        return;
    };
    renderer.set_battlefield_ground(&ctx, &ground_v, &ground_i, &ground_maps, &materials);
    renderer.set_water(&ctx, &water_v, &water_i);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
    renderer.set_foliage_atlas(&ctx, &scene_build::flora_pack::flora_catalog().atlas_mips);
    renderer.register_mesh(&ctx, client::GRASS_MESH_HANDLE, &client::grass_tuft_mesh());

    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
    let mut samples_ms: Vec<f64> = Vec::with_capacity(FRAMES);

    for frame in 0..(WARMUP + FRAMES) {
        // Walk the eye across the valley: a static camera lets anything that caches per-position
        // report a frame cost the moving game never pays.
        let travel = frame as f32 * 0.35;
        let eye = glam::Vec3::new(430.0 + travel * 0.4, 26.0, 380.0 + travel);
        let camera = renderer_api::Camera {
            eye: eye.into(),
            target: (eye + glam::Vec3::new(0.35, -0.25, 1.0)).into(),
            vertical_fov_degrees: 60.0,
        };
        let view_proj = renderer_api::view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        let grass = client::grass_frame_objects(
            &battlefield.heightmap,
            battlefield.water,
            &ground_maps,
            &materials,
            eye,
        );

        let t = Instant::now();
        renderer.set_render_frame(
            &ctx,
            &renderer_api::RenderFrame { objects: grass, ..renderer_api::RenderFrame::default() },
        );
        if renderer.render(&ctx, target.render_target(), view_proj, camera.eye).is_err() {
            println!("frame time: a render failed — skipped");
            return;
        }
        // The GPU is asynchronous. Without a fence this times command SUBMISSION and reports a
        // fiction; the readback is the cheapest fence available here — and it is NOT free, so its
        // cost is measured separately below and reported alongside.
        if target.read_rgba8(&ctx).is_err() {
            println!("frame time: readback failed — skipped");
            return;
        }
        if frame >= WARMUP {
            samples_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }

    // Calibrate the fence: the same readback against an already-rendered target, with no scene
    // work in front of it. Whatever this costs is instrument overhead the real frame never pays.
    let mut fence_ms: Vec<f64> = Vec::with_capacity(30);
    for _ in 0..30 {
        let t = Instant::now();
        if target.read_rgba8(&ctx).is_err() {
            break;
        }
        fence_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    fence_ms.sort_by(f64::total_cmp);
    let fence_p50 = fence_ms.get(fence_ms.len() / 2).copied().unwrap_or(0.0);

    samples_ms.sort_by(f64::total_cmp);
    let at =
        |p: f64| samples_ms[((samples_ms.len() as f64 * p) as usize).min(samples_ms.len() - 1)];
    println!(
        "frame time @{width}x{height} ({} samples): p50 {:.2} ms  p95 {:.2} ms  p99 {:.2} ms  max {:.2} ms",
        samples_ms.len(),
        at(0.50),
        at(0.95),
        at(0.99),
        samples_ms[samples_ms.len() - 1]
    );
    println!(
        "  readback fence alone: {fence_p50:.2} ms  ->  scene work ~{:.2} ms p50",
        (at(0.50) - fence_p50).max(0.0)
    );
    println!("  (offscreen: excludes present/vsync, and this is not the MX330 the policy names.");
    println!("   Read it BEFORE and AFTER a change on one machine; it is not a pass mark.)");
}
