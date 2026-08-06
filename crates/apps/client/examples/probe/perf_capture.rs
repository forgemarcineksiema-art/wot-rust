//! Release-mode timing capture: the one-off bakes (battle-scene hitch sizes), the CPU costs the
//! render loop pays every frame (grass conjuring), and — since the "one look" policy is stated in
//! FRAMES — the frame itself.
//!
//! Run: `cargo run -p client --release --example probe -- perf_capture`
//!
//! **What the frame number is and is not.** It renders a real battle scene OFFSCREEN, so it counts
//! the GPU and CPU work of producing an image and NOT swapchain acquire, present or vsync pacing —
//! which makes every number here a LOWER BOUND on what the real frame costs.
//!
//! It runs on whatever machine you are sitting at. When that machine IS the min spec (the author's
//! dev box is the MX330 the policy names, as `detail_cost_probe` also records), the absolute
//! number means something: subtract the reported readback fence and compare the remainder against
//! 16.67 ms. On any other machine it is a RELATIVE instrument — run it before a change and after,
//! on one machine, and only the delta is real.
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
            &battlefield.static_cover,
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

/// A 7v7 parked along the camera path, at the spread a battle actually presents.
///
/// The frame instrument measured ground, water, dressing and grass and NOT ONE TANK — which made
/// the largest single body of geometry on a vehicle (a T-54's running gear is 38.6k triangles
/// across 204 instances, more than twice its whole static bake) invisible to the only number the
/// "one look" policy is stated in. The vehicles are spread in depth on purpose: the gear switches
/// detail tier by distance, so a battle's real cost is a MIX of tiers, and a lineup parked at one
/// range would measure a tier the game never draws alone.
fn battle_lineup(
    battlefield: &terrain::BattlefieldMap,
    catalog: &mut client::VehicleMeshCatalog,
    eye: Option<[f32; 3]>,
) -> Vec<renderer_api::RenderObject> {
    use game_core::VehicleKind;

    // An engagement, not a parade: the fourteen sit 27..180 m out from the eye path (which walks
    // +Z from z=380 to z≈443), so about four of them fall inside the 60 m gear-detail threshold
    // at the closest point and the rest stay on the distance tier. A lineup parked at ONE range
    // would measure a tier the game never draws alone; passing `eye` applies the battle's own
    // rule per tank, which the first version of this instrument did not — it built every tank at
    // the near tier and so measured gear the game does not draw at that distance.
    let roster = VehicleKind::PLAYABLE;
    let mut objects = Vec::new();
    for slot in 0..14 {
        let team = slot / 7;
        let file = slot % 7;
        let kind = roster[slot % roster.len()];
        let x = 442.0 + (file as f32 - 3.0) * 12.0 + if team == 0 { -7.0 } else { 7.0 };
        let z = 470.0 + slot as f32 * 9.0;
        let ground = battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
        let snapshot = net::TankSnapshot {
            tank_id: game_core::TankId(slot as u64 + 1),
            team: game_core::TeamId(team as u16 + 1),
            vehicle: kind,
            position: [x, ground, z],
            yaw_rad: if team == 0 { 0.4 } else { 3.5 },
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.1,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.05,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: kind.spec().gun.dispersion_mrad,
            module_hit_points: kind.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            rack_fire_remaining_s: None,
        };
        let tint = if team == 0 { [0.34, 0.38, 0.30] } else { [0.40, 0.34, 0.28] };
        objects.append(&mut client::tank_render_objects_from_eye(catalog, &snapshot, tint, eye));
    }
    objects
}

/// Render a real battle scene offscreen and report the frame-time distribution.
///
/// Percentiles, not a mean: a frame budget is missed by the SLOW frames, and an average hides
/// exactly the spikes the one-look policy calls a bug.
///
/// The scene is assembled through the SAME calls `look_harness` uses for review frames — ground,
/// statics, water, grass-card dressing, the flora atlas, the grass population, and (since the
/// fleet-parity programme) the vehicles — so this measures the picture the game actually draws
/// rather than a stripped-down stand-in that would flatter it.
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
    let flora_catalog = scene_build::flora_pack::flora_catalog();
    renderer.set_foliage_atlas(&ctx, &flora_catalog.atlas_mips, flora_catalog.normal_mips.as_ref());
    for (handle, mesh) in client::grass_species_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }

    // The fleet enters the frame instrument. Built once (the lineup is static at rest phase, so
    // per-frame cost is the DRAW, which is what we are measuring) and its meshes registered like
    // any other.
    // Pre-warm: creating a vehicle's catalog entry registers BOTH gear tiers, so one build puts
    // every mesh the rotation can ask for on the GPU before any frame is timed.
    let mut catalog = client::VehicleMeshCatalog::default();
    let warm = battle_lineup(&battlefield, &mut catalog, None);
    for (handle, mesh) in catalog.take_pending_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    println!("battle lineup: 14 vehicles, {} render objects at the near tier", warm.len());

    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();

    // The A/B instrument (Jedna Trawa P0). Sequential probe processes measure the thermal ramp
    // of this laptop, not the scene (the Teren program learned this the hard way): run 2 of 3
    // reported REMOVING work as costing +2.6 ms. So the configs rotate INSIDE one process in
    // short blocks — every config visits every thermal state, every block walks the identical
    // camera path, and only the per-config aggregate is reported.
    // The sniper entry step (18°) rides along as its own config: D3 stretches the far
    // collapse and the chunk cutoff under magnification, and the honest question — does a
    // magnified meadow cost more than the wide view? — is only answerable by measuring the
    // scope in the same rotation as the battle view.
    // The last field is the fleet: "full scene" and "full + 7v7" are the SAME frame apart from
    // 14 vehicles, so their delta is what putting the fleet on screen costs — the number the
    // running-gear budget has never been checked against.
    let configs: [(&str, bool, bool, f32, bool); 5] = [
        ("full scene", true, true, 60.0, false),
        ("no card meadow", false, true, 60.0, false),
        ("no near ring", true, false, 60.0, false),
        ("scope 18deg", true, true, 18.0, false),
        ("full + 7v7", true, true, 60.0, true),
    ];
    const CYCLES: usize = 4;
    const BLOCK_WARMUP: usize = 8;
    let block_frames = FRAMES / CYCLES;
    let mut samples: [Vec<f64>; 5] = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut dressing_bound = true;

    for _ in 0..WARMUP {
        let camera = renderer_api::Camera {
            eye: [430.0, 26.0, 380.0],
            target: [430.35, 25.75, 381.0],
            vertical_fov_degrees: 60.0,
        };
        let view_proj = renderer_api::view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        if renderer.render(&ctx, target.render_target(), view_proj, camera.eye).is_err() {
            println!("frame time: a render failed — skipped");
            return;
        }
    }
    let _ = target.read_rgba8(&ctx);

    for _cycle in 0..CYCLES {
        for (config, &(_, with_dressing, with_grass, fov, with_tanks)) in configs.iter().enumerate()
        {
            // Buffer swaps happen OUTSIDE the timed frames; empty slices clear the slot.
            if with_dressing != dressing_bound {
                if with_dressing {
                    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
                } else {
                    renderer.set_dressing(&ctx, &[], &[]);
                }
                dressing_bound = with_dressing;
            }
            for block_frame in 0..(BLOCK_WARMUP + block_frames) {
                // Every block walks the SAME eye path: configs are compared on identical
                // content, and nothing that caches per-position flatters a moving camera.
                let travel = block_frame.saturating_sub(BLOCK_WARMUP) as f32 * 0.35 * 4.0;
                let eye = glam::Vec3::new(430.0 + travel * 0.4, 26.0, 380.0 + travel);
                let camera = renderer_api::Camera {
                    eye: eye.into(),
                    target: (eye + glam::Vec3::new(0.35, -0.25, 1.0)).into(),
                    vertical_fov_degrees: fov,
                };
                let view_proj = renderer_api::view_projection_matrix(
                    &camera,
                    width as f32 / height as f32,
                    projection.near_plane_m(),
                    projection.far_plane_m(),
                );
                let grass = if with_grass {
                    client::grass_frame_objects(
                        &battlefield.heightmap,
                        battlefield.water,
                        &battlefield.static_cover,
                        &ground_maps,
                        &materials,
                        eye,
                    )
                } else {
                    Vec::new()
                };

                // Rebuilt every frame with the CURRENT eye, exactly as the battle path does: the
                // detail tier a tank draws at is a function of where the camera is this frame.
                let mut grass = grass;
                if with_tanks {
                    grass.append(&mut battle_lineup(&battlefield, &mut catalog, Some(eye.into())));
                    for (handle, mesh) in catalog.take_pending_meshes() {
                        renderer.register_mesh(&ctx, handle, &mesh);
                    }
                }

                let t = Instant::now();
                renderer.set_render_frame(
                    &ctx,
                    &renderer_api::RenderFrame {
                        objects: grass,
                        ..renderer_api::RenderFrame::default()
                    },
                );
                if renderer.render(&ctx, target.render_target(), view_proj, camera.eye).is_err() {
                    println!("frame time: a render failed — skipped");
                    return;
                }
                // The GPU is asynchronous. Without a fence this times command SUBMISSION and
                // reports a fiction; the readback is the cheapest fence available here — and it
                // is NOT free, so its cost is measured separately below and reported alongside.
                if target.read_rgba8(&ctx).is_err() {
                    println!("frame time: readback failed — skipped");
                    return;
                }
                if block_frame >= BLOCK_WARMUP {
                    samples[config].push(t.elapsed().as_secs_f64() * 1000.0);
                }
            }
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

    let mut full_p50 = 0.0;
    for (config, (name, _, _, _, _)) in configs.iter().enumerate() {
        let series = &mut samples[config];
        series.sort_by(f64::total_cmp);
        let at = |p: f64| series[((series.len() as f64 * p) as usize).min(series.len() - 1)];
        if config == 0 {
            full_p50 = at(0.50);
        }
        println!(
            "frame time @{width}x{height} [{name}] ({} samples, {CYCLES} interleaved cycles): p50 {:.2} ms  p95 {:.2} ms  p99 {:.2} ms  max {:.2} ms  (Δ vs full {:+.2} ms p50)",
            series.len(),
            at(0.50),
            at(0.95),
            at(0.99),
            series[series.len() - 1],
            at(0.50) - full_p50,
        );
    }
    println!(
        "  readback fence alone: {fence_p50:.2} ms  ->  full-scene work ~{:.2} ms p50",
        (full_p50 - fence_p50).max(0.0)
    );
    println!(
        "  (offscreen: excludes present/vsync, so every number here is a FLOOR. Subtract the \
         fence, then compare"
    );
    println!(
        "   against 16.67 ms for 60 FPS — meaningful only when this box IS the min spec; \
         elsewhere read deltas.)"
    );
}
