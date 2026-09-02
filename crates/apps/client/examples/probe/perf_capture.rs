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
    // sign-off is measured, not promised. The battlefield oaks draw through the instanced LOD
    // ladder, so they are NOT in the statics bake — the tree cost lives in flora_frame_probe.
    let city = map_forge::battlefield(terrain::MapId::Ostrogorsk);
    let battlefield_oaks =
        city.scenery.iter().filter(|instance| instance.kind == terrain::SceneryKind::Oak).count();
    let born = terrain::initial_cover_phase_bytes(&city.static_cover);
    let t = Instant::now();
    let mut city_buckets = client::battlefield_statics_buckets(&city, &born, &[]);
    let (cv, ci) = client::assemble_statics_mesh(&city_buckets);
    println!(
        "ostrogorsk statics bake ({} boxes, {battlefield_oaks} oaks off-bake): {:.1} ms ({} v / {} i)",
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
            battlefield.water_view(),
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
    garage_frame_time_capture();

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
/// How this config decides each tank's running-gear tier.
///
/// The shipping builder derives the tier from ONE input -- the distance from the eye -- and
/// nothing else reads that argument. So the brackets are expressed by lying about where the eye
/// is, rather than by a second tier mechanism the probe would own: `None` is the builder's
/// documented "no camera" case (authored construction, i.e. near), and a distant sentinel forces
/// the far tier on every tank. The probe therefore has no tier rule of its own to drift from the
/// game's.
#[derive(Clone, Copy)]
enum FleetGear {
    /// The battle's own rule, applied per tank from this frame's real eye.
    FromEye,
    ForcedNear,
    ForcedFar,
}

impl FleetGear {
    fn eye(self, actual: [f32; 3]) -> Option<[f32; 3]> {
        match self {
            FleetGear::FromEye => Some(actual),
            FleetGear::ForcedNear => None,
            FleetGear::ForcedFar => Some([0.0, 0.0, -10_000.0]),
        }
    }
}

/// One row of the rotation: a scene the renderer already knows how to be, plus a name.
///
/// A struct rather than a tuple because the row grew to seven fields, and a reader counting
/// `true, true, 60.0` positions to work out which flag is which is a reader about to misread a
/// measurement.
#[derive(Clone, Copy)]
struct Config {
    name: &'static str,
    dressing: bool,
    grass: bool,
    fov: f32,
    fleet: Option<FleetGear>,
    shadows: bool,
    ssao: bool,
    /// Inny Poziom S1: six shot lights live in the frame's local slots — the barrage worst
    /// case, since every lit pass pays the per-fragment pool loop for a live slot whether or
    /// not the pool reaches the pixel.
    shot_lights: bool,
}

impl Config {
    /// The full battle scene. Every other row is this one with something taken away, so the
    /// difference between them is the thing taken away and nothing else.
    fn named(name: &'static str) -> Self {
        Self {
            name,
            dressing: true,
            grass: true,
            fov: 60.0,
            fleet: None,
            shadows: true,
            ssao: true,
            shot_lights: false,
        }
    }
}

/// The 7v7, built through the SAME call the battle makes.
///
/// It used to be built through `tank_render_objects_tiered` into the SCENE frame -- a second
/// vehicle path the game does not use. That drew the fleet with the scene shader instead of
/// `vehicle.wgsl`, without vehicle materials, and INTO THE FAR SHADOW CASCADE the battle
/// deliberately excludes it from. Every fleet cost this probe ever reported described a fleet
/// drawn as scenery.
fn battle_lineup(
    battlefield: &terrain::BattlefieldMap,
    catalog: &mut client::VehicleAssetCatalog,
    eye: Option<[f32; 3]>,
) -> client::VehicleRenderFrame {
    use game_core::VehicleKind;

    // An engagement, not a parade: the fourteen sit 27..180 m out from the eye path (which walks
    // +Z from z=380 to z~443), so about four of them fall inside the 60 m gear-detail threshold
    // at the closest point and the rest stay on the distance tier. A lineup parked at ONE range
    // would measure a tier the game never draws alone.
    let roster = VehicleKind::PLAYABLE;
    let mut tanks = Vec::with_capacity(14);
    for slot in 0..14u32 {
        let team = slot / 7;
        let file = slot % 7;
        let kind = roster[slot as usize % roster.len()];
        let x = 442.0 + (file as f32 - 3.0) * 12.0 + if team == 0 { -7.0 } else { 7.0 };
        let z = 470.0 + slot as f32 * 9.0;
        let ground = battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
        tanks.push(engine::PresentationTank {
            id: game_core::TankId(u64::from(slot) + 1),
            team: game_core::TeamId(team as u16 + 1),
            vehicle: kind,
            translation: [x, ground, z],
            hull_yaw_rad: if team == 0 { 0.4 } else { 3.5 },
            turret_yaw_rad: 0.1,
            gun_pitch_rad: 0.05,
            hit_points: 1000,
            destroyed_modules_mask: 0,
            spotted_by_teams_mask: 0,
            module_hit_points: kind.spec().module_health.hit_points_by_slot(),
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        });
    }
    client::split_pbr_vehicle_render_frame_on_terrain(
        catalog,
        tanks,
        game_core::TankId(1),
        1.0,
        Some(&battlefield.heightmap),
        0,
        eye,
    )
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

    // Armed for per-pass timing. Every config in the rotation pays the same query writes, so the
    // DELTAS between them are untouched; the absolutes carry whatever the instrument costs, which
    // is the honest trade for knowing where the time goes.
    let Ok(ctx) =
        renderer_wgpu::GpuContext::headless_with_options(renderer_wgpu::GpuContextOptions {
            pass_timing: true,
        })
    else {
        println!("frame time: no headless GPU adapter — skipped");
        return;
    };
    // The SHIPPED sample count, not the review one. Until this line existed the capture built
    // its scene at 4x MSAA while the window ships 1x on every adapter, so every frame-time
    // number this project has quoted described a picture nobody plays — more expensive in fill
    // and cleaner on distant geometry than the real thing.
    let Ok(target) = renderer_wgpu::OffscreenTarget::new(&ctx, width, height) else {
        println!("frame time: offscreen target unavailable — skipped");
        return;
    };
    let Ok(mut renderer) =
        renderer_wgpu::SceneRenderer::for_offscreen_as_shipped(&ctx, &statics_v, &statics_i)
    else {
        println!("frame time: scene renderer unavailable — skipped");
        return;
    };
    // Which GPU produced these numbers, in the numbers themselves. The "one look" policy is
    // stated in terms of one machine — the MX330 — and until this line every frame time this
    // project quoted was attributed to that machine by belief, not by the instrument. This box
    // carries two adapters (an Intel iGPU and the MX330); `PowerPreference::HighPerformance`
    // should pick the discrete one, but "should" is not a measurement.
    let adapter = ctx.adapter.get_info();
    println!(
        "frame time: {}x{} offscreen, {}x MSAA (the shipped count — review images use 4x), \
         adapter: {} ({:?}, {:?}, driver {})",
        width,
        height,
        renderer.sample_count(),
        adapter.name,
        adapter.backend,
        adapter.device_type,
        adapter.driver,
    );
    let profiler = renderer_wgpu::FrameProfiler::new(&ctx.device, &ctx.queue, true);
    if let Some(reason) = profiler.unavailable_reason() {
        println!("per-pass timing unavailable: {reason}");
    }
    let timed = profiler.active().is_some();
    renderer.set_pass_profiler(profiler);
    renderer.set_battlefield_ground(&ctx, &ground_v, &ground_i, &ground_maps, &materials);
    renderer.set_water(&ctx, &water_v, &water_i);
    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
    // The leaf atlas, exactly as the battle binds it — see `bind_battle_foliage_atlas`.
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    for (handle, mesh) in client::grass_species_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }

    // The fleet enters the frame instrument. Built once (the lineup is static at rest phase, so
    // per-frame cost is the DRAW, which is what we are measuring) and its meshes registered like
    // any other.
    // Pre-warm: creating a vehicle's catalog entry registers BOTH gear tiers, so one build puts
    // every mesh the rotation can ask for on the GPU before any frame is timed.
    let mut catalog = client::VehicleAssetCatalog::default();
    // Both tiers are built once, so every mesh the rotation can ask for is on the GPU before any
    // frame is timed; a first-use bake inside a timed block would be measured as frame cost.
    let warm = battle_lineup(&battlefield, &mut catalog, None);
    let _ = battle_lineup(&battlefield, &mut catalog, Some([0.0, 0.0, -10_000.0]));
    for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
        renderer.register_vehicle_mesh(&ctx, handle, &mesh);
    }
    for (handle, maps) in catalog.take_pending_vehicle_materials() {
        renderer.register_vehicle_material(&ctx, handle, &maps);
    }
    // Objects are NOT draws: the renderer batches by (mesh, material), so a tank's 192 shoe
    // links collapse into one instanced draw. Both numbers are reported below, per config, and
    // the draw count now comes from the FRAMES THEMSELVES rather than from a second, parallel
    // implementation of batching that nothing else used and nothing kept honest.
    let lineup_objects = warm.objects.len();

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
    // `None` = no vehicles. `Some(tier)` = the fleet at that tier. The two FORCED rows bracket
    // what the distance tier can ever buy, and they sit in the SAME rotation as everything else
    // because a tier A/B run as separate processes measures this laptop's thermal ramp instead
    // (the baseline moved 19.2 -> 23.8 ms across four sequential runs while nothing changed).
    // The last two rows exist because the per-pass table said the scene pass is three quarters
    // of the frame. Knowing a pass costs 12 ms is not the same as knowing WHY: shadows and SSAO
    // each have their own passes AND a per-pixel cost inside the scene shader, and only turning
    // them off separates the two. Both toggles are the renderer's own capability fallbacks, so
    // this measures a configuration the renderer already knows how to be.
    let configs = [
        Config::named("full scene"),
        Config { dressing: false, ..Config::named("no card meadow") },
        Config { grass: false, ..Config::named("no near ring") },
        Config { fov: 18.0, ..Config::named("scope 18deg") },
        Config { fleet: Some(FleetGear::FromEye), ..Config::named("full + 7v7") },
        Config {
            fleet: Some(FleetGear::FromEye),
            shot_lights: true,
            ..Config::named("full + 7v7 + 6 shot lights")
        },
        Config { fleet: Some(FleetGear::ForcedNear), ..Config::named("7v7 gear NEAR forced") },
        Config { fleet: Some(FleetGear::ForcedFar), ..Config::named("7v7 gear FAR forced") },
        Config { shadows: false, ..Config::named("no shadows") },
        Config { ssao: false, ..Config::named("no ssao") },
    ];
    // The per-config arrays below are fixed-size, so a row added to the table without this
    // number moving would silently drop off the end of the report.
    const CONFIGS: usize = 10;
    assert_eq!(configs.len(), CONFIGS, "the config table and its sample arrays disagree");
    const CYCLES: usize = 4;
    const BLOCK_WARMUP: usize = 8;
    let block_frames = FRAMES / CYCLES;
    let mut samples: [Vec<f64>; CONFIGS] = std::array::from_fn(|_| Vec::new());
    // What each config actually submitted, taken off the last timed frame of its last block.
    // Every timed frame of a config walks the same path and draws the same content, so one
    // frame's counts describe the config.
    let mut counts: [renderer_wgpu::FrameCounts; CONFIGS] = Default::default();
    // The CPU half of the frame, split three ways. The GPU table says where the GPU's time goes;
    // these say where the ~2 ms it does NOT account for goes.
    let mut cpu_scene: [Vec<f64>; CONFIGS] = std::array::from_fn(|_| Vec::new());
    let mut cpu_vehicle: [Vec<f64>; CONFIGS] = std::array::from_fn(|_| Vec::new());
    let mut cpu_encode: [Vec<f64>; CONFIGS] = std::array::from_fn(|_| Vec::new());
    // Per-pass GPU time, kept per config AND per rotation cycle. The ledger refuses to report if
    // any config missed a cycle — an incomplete rotation is two thermal states wearing one run's
    // authority, which is the exact reading the rotation exists to prevent.
    let mut stats = client::RotationStats::new(configs.len(), CYCLES, renderer_wgpu::PassId::COUNT);
    let base_lighting = renderer.scene_lighting;
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

    for cycle in 0..CYCLES {
        for (config, cfg) in configs.iter().enumerate() {
            let Config { dressing: with_dressing, grass: with_grass, fov, fleet: tanks, .. } = *cfg;
            // Buffer swaps and quality toggles happen OUTSIDE the timed frames; empty slices
            // clear the slot.
            if with_dressing != dressing_bound {
                if with_dressing {
                    renderer.set_dressing(&ctx, &dressing_v, &dressing_i);
                } else {
                    renderer.set_dressing(&ctx, &[], &[]);
                }
                dressing_bound = with_dressing;
            }
            renderer.set_shadows_enabled(cfg.shadows);
            renderer.set_ssao_enabled(cfg.ssao);
            // The shot's light (S1): six D-10 flashes strung along the eye path, or the
            // profile's own all-off slots. Set outside the timed frames like every toggle.
            let mut lighting = base_lighting;
            if cfg.shot_lights {
                for (index, slot) in lighting.local_lights.iter_mut().enumerate() {
                    let along = index as f32;
                    *slot = renderer_api::LocalLight::muzzle_flash(
                        [432.0 + along * 5.0, 8.0, 396.0 + along * 14.0],
                        1.0,
                    );
                }
            }
            renderer.scene_lighting = lighting;
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
                        battlefield.water_view(),
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
                let fleet = tanks.map_or_else(
                    || client::VehicleRenderFrame { objects: Vec::new(), armor_damage: Vec::new() },
                    |gear| battle_lineup(&battlefield, &mut catalog, gear.eye(eye.into())),
                );
                for (handle, mesh) in catalog.take_pending_vehicle_meshes() {
                    renderer.register_vehicle_mesh(&ctx, handle, &mesh);
                }
                for (handle, maps) in catalog.take_pending_vehicle_materials() {
                    renderer.register_vehicle_material(&ctx, handle, &maps);
                }

                // The timed window splits into three CPU phases and a fence. The whole frame minus
                // the fence runs ~2 ms longer than the GPU says the frame took, so about that much
                // is CPU — and until these three timers existed, nothing said WHICH of the three
                // it was. `render` returns after submit, which is asynchronous, so its wall time
                // is encode cost rather than GPU work.
                let t = Instant::now();
                renderer.set_render_frame(
                    &ctx,
                    &renderer_api::RenderFrame {
                        objects: grass,
                        ..renderer_api::RenderFrame::default()
                    },
                );
                let t_scene = t.elapsed();
                // Always set it, even when empty: otherwise the previous config's fleet would
                // linger into a config that is supposed to have none.
                renderer.set_vehicle_render_frame(
                    &ctx,
                    &renderer_api::RenderFrame {
                        objects: fleet.objects,
                        armor_damage: fleet.armor_damage,
                        ..renderer_api::RenderFrame::default()
                    },
                );
                let t_vehicle = t.elapsed();
                if renderer.render(&ctx, target.render_target(), view_proj, camera.eye).is_err() {
                    println!("frame time: a render failed — skipped");
                    return;
                }
                let t_encode = t.elapsed();
                // The GPU is asynchronous. Without a fence this times command SUBMISSION and
                // reports a fiction; the readback is the cheapest fence available here — and it
                // is NOT free, so its cost is measured separately below and reported alongside.
                if target.read_rgba8(&ctx).is_err() {
                    println!("frame time: readback failed — skipped");
                    return;
                }
                if block_frame >= BLOCK_WARMUP {
                    samples[config].push(t.elapsed().as_secs_f64() * 1000.0);
                    cpu_scene[config].push(t_scene.as_secs_f64() * 1000.0);
                    cpu_vehicle[config].push((t_vehicle - t_scene).as_secs_f64() * 1000.0);
                    cpu_encode[config].push((t_encode - t_vehicle).as_secs_f64() * 1000.0);
                    counts[config] = renderer.last_frame_counts();
                    // Read AFTER the wall-clock sample: the readback maps a buffer and would
                    // otherwise be timed as frame cost, which is how an instrument starts
                    // measuring itself.
                    if let Some(timings) = renderer.read_pass_timings(&ctx) {
                        let per_pass: Vec<Option<f32>> = renderer_wgpu::PassId::ALL
                            .iter()
                            .map(|id| timings.pass_ms(*id))
                            .collect();
                        stats.record(config, cycle, timings.frame_ms(), &per_pass);
                    }
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
    for (config, cfg) in configs.iter().enumerate() {
        let name = cfg.name;
        let series = &mut samples[config];
        series.sort_by(f64::total_cmp);
        let at = |p: f64| series[((series.len() as f64 * p) as usize).min(series.len() - 1)];
        if config == 0 {
            full_p50 = at(0.50);
        }
        let submitted = counts[config].total();
        println!(
            "frame time @{width}x{height} [{name}] ({} samples, {CYCLES} interleaved cycles): p50 {:.2} ms  p95 {:.2} ms  p99 {:.2} ms  max {:.2} ms  (Δ vs full {:+.2} ms p50)",
            series.len(),
            at(0.50),
            at(0.95),
            at(0.99),
            series[series.len() - 1],
            at(0.50) - full_p50,
        );
        println!(
            "    submitted: {} draws, {} triangles, {} instances",
            submitted.draws, submitted.triangles, submitted.instances,
        );
        let cpu = |label: &str, series: &mut Vec<f64>| {
            series.sort_by(f64::total_cmp);
            let at = |p: f64| series[((series.len() as f64 * p) as usize).min(series.len() - 1)];
            println!("    cpu {label:<16} p50 {:>6.3} ms  p95 {:>6.3} ms", at(0.50), at(0.95));
        };
        cpu("set_render_frame", &mut cpu_scene[config]);
        cpu("set_vehicle_frame", &mut cpu_vehicle[config]);
        cpu("encode+submit", &mut cpu_encode[config]);
        for id in renderer_wgpu::PassId::ALL {
            let pass = counts[config].pass(*id);
            if pass.draws > 0 {
                println!(
                    "      {:<18} {:>4} draws  {:>9} tris  {:>6} inst",
                    id.label(),
                    pass.draws,
                    pass.triangles,
                    pass.instances,
                );
            }
        }
    }
    println!(
        "  the 7v7 lineup is {lineup_objects} render objects; the counts above are what the renderer actually submitted after batching by (mesh, material)."
    );

    // Per-pass GPU time. Printed only if the rotation completed: the whole defence against this
    // laptop's thermal ramp is that every config visited every cycle, and an aggregate over a
    // half-finished rotation compares a cold config against a hot one while looking rigorous.
    if !timed {
        println!(
            "
per-pass GPU time: unavailable on this adapter"
        );
    } else if let Some(complaint) = stats.imbalance() {
        println!(
            "
per-pass GPU time: {complaint}"
        );
    } else {
        println!(
            "
per-pass GPU time (ms, timestamp queries; every config equally armed):"
        );
        for (config, cfg) in configs.iter().enumerate() {
            let name = cfg.name;
            let frame = stats.frame(config);
            println!(
                "  [{name}] frame p50 {:.3}  p95 {:.3}  p99 {:.3}  max {:.3}  ({} samples)",
                frame.p50, frame.p95, frame.p99, frame.max, frame.samples,
            );
            // The SHARE is the number that survives a hot box. Absolutes drift with the
            // laptop's thermal state by a factor of two; the split between passes does not,
            // which makes it the part of this table worth acting on.
            let share = |ms: f32| if frame.p50 > 0.0 { 100.0 * ms / frame.p50 } else { 0.0 };
            for (index, id) in renderer_wgpu::PassId::ALL.iter().enumerate() {
                let Some(pass) = stats.pass(config, index) else { continue };
                println!(
                    "      {:<18} p50 {:>7.3}  p95 {:>7.3}  p99 {:>7.3}   {:>5.1}% of frame",
                    id.label(),
                    pass.p50,
                    pass.p95,
                    pass.p99,
                    share(pass.p50),
                );
            }
            // Always, even at zero: a table that only mentions the gap when it is large teaches
            // the reader that the passes are the whole frame.
            let gap = stats.unattributed(config);
            println!(
                "      {:<18} p50 {:>7.3}  p95 {:>7.3}  p99 {:>7.3}   {:>5.1}% <- outside every pass",
                "unattributed",
                gap.p50,
                gap.p95,
                gap.p99,
                share(gap.p50),
            );
        }
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

/// The GARAGE frame (Hala 3.0 F1): the hangar measured with the battle instrument's own
/// discipline — offscreen at the shipped sample count, configs rotated inside one process so
/// every configuration visits every thermal state, percentiles rather than means.
///
/// The camera walks a FULL ORBIT each block (the boom clamped by `max_orbit_boom`, exactly as
/// the live camera clamps it), so every wall angle, the gate's daylight block and the shed
/// glazing all pass through the frame — a parked hero shot from one angle would measure the
/// garage's cheapest view and call it the garage.
///
/// The rotation prices the deferred purchase decisions by A/B:
/// - "no interior grain" is C1's per-scene detail-normal flag turned off — its delta IS the
///   flag's price, the number the C1 purchase was made on credit against;
/// - "no shadows"/"no ssao" split the room's per-pixel lighting cost the way the battle
///   rows do.
///
/// Hala v4 P1 widened it into an ATTRIBUTION instrument: the scene comes from the same
/// `apply_shipped_garage_scene` the goldens use (the capture measured a stale room for four
/// days once); ablation rigs isolate the vehicle's overdraw, the furnishings' fill, the
/// shaft quads and the penumbra kernel; a @4x block prices the F2 candidate; per-pass
/// tables print for every config; and a 720p coda confirms (or refutes) fill-bound.
fn garage_frame_time_capture() {
    use game_core::{TankId, TeamId, VehicleKind};

    const WARMUP: usize = 12;
    const FRAMES: usize = 120;
    const CYCLES: usize = 4;
    const BLOCK_WARMUP: usize = 6;
    let (width, height) = (1920u32, 1080u32);

    // What the game uploads post-E3: the shell without the gate curtain in the statics slot,
    // fan blades + parked slats in the dynamic slot. Beside it, the P1 ablation variants:
    // the same hall unfurnished (gallery + props skipped) and a bare slab of its footprint.
    let (hangar_v, hangar_i) = scene_build::hangar::hangar_scene_mesh_without_gate();
    let (bare_v, bare_i, bare_casters) = scene_build::hangar::hangar_probe_mesh_unfurnished();
    let (slab_v, slab_i) = scene_build::hangar::hangar_probe_mesh_floor_slab();

    let Ok(ctx) =
        renderer_wgpu::GpuContext::headless_with_options(renderer_wgpu::GpuContextOptions {
            pass_timing: true,
        })
    else {
        println!("garage frame time: no headless GPU adapter — skipped");
        return;
    };
    let Ok(target) = renderer_wgpu::OffscreenTarget::new(&ctx, width, height) else {
        println!("garage frame time: offscreen target unavailable — skipped");
        return;
    };
    // Four renderers, ONE rotation: the statics buffer is constructor-bound, so each mesh
    // variant (and the 4x candidate) is its own renderer, and the blocks interleave across
    // all of them so every rig visits every thermal state.
    let built = (
        renderer_wgpu::SceneRenderer::for_offscreen_as_shipped(&ctx, &hangar_v, &hangar_i),
        renderer_wgpu::SceneRenderer::for_offscreen_as_shipped(&ctx, &bare_v, &bare_i),
        renderer_wgpu::SceneRenderer::for_offscreen_as_shipped(&ctx, &slab_v, &slab_i),
        renderer_wgpu::SceneRenderer::for_offscreen_with_sample_count(
            &ctx, 4, &hangar_v, &hangar_i,
        ),
    );
    let (Ok(mut shipped), Ok(mut unfurnished), Ok(mut slab), Ok(mut four_x)) = built else {
        println!("garage frame time: a scene renderer was unavailable — skipped");
        return;
    };
    let adapter = ctx.adapter.get_info();
    println!(
        "garage frame time: {}x{} offscreen, {}x MSAA shipped (+{}x candidate), adapter: {} ({:?})",
        width,
        height,
        shipped.sample_count(),
        four_x.sample_count(),
        adapter.name,
        adapter.backend,
    );
    for renderer in [&mut shipped, &mut unfurnished, &mut slab, &mut four_x] {
        renderer.set_pass_profiler(renderer_wgpu::FrameProfiler::new(
            &ctx.device,
            &ctx.queue,
            true,
        ));
    }
    let timed = shipped.pass_profiler().active().is_some();

    // The SHIPPED garage scene, from the SAME function the golden harness applies — this
    // capture measured a stale room for four days once (#544/#545/#554 changed the shipped
    // garage; the instrument kept the old setup), so the assembly is shared, not copied.
    client::apply_shipped_garage_scene(&ctx, &mut shipped);
    client::apply_shipped_garage_scene(&ctx, &mut four_x);
    client::apply_shipped_garage_scene(&ctx, &mut unfurnished);
    // ...except the caster set: the shipped indices index the shipped vertex buffer, and the
    // unfurnished hall trims its own sun with the same filter instead.
    unfurnished.set_terrain_shadow_indices(&ctx, Some(&bare_casters));
    // The slab rig is the "vehicle only" reference: the shipped rig and switches with none of
    // the hall's own draws — no shafts, no fan/gate/mechanic, casters = the slab itself.
    slab.scene_time_s = 12.0; // the frozen review second, as the shared assembly sets it
    slab.scene_lighting =
        scene_build::hangar::hangar_lighting(scene_build::hangar::HangarLight::Day);
    let (bg_r, bg_g, bg_b) = scene_build::hangar::INTERIOR_BACKGROUND;
    slab.set_interior_background(bg_r, bg_g, bg_b);
    slab.shadow_focus = Some(scene_build::hangar::hangar_shadow_focus());
    slab.shadow_focus_radius_m = Some(scene_build::hangar::hangar_shadow_radius_m());
    slab.shadow_cascades = Some(1);
    slab.set_shadow_softness(scene_build::hangar::HANGAR_SHADOW_SOFTNESS);
    slab.set_bloom_mips(scene_build::hangar::hangar_bloom_mips());
    slab.set_hero_probe(Some(scene_build::hangar::hangar_hero_probe()));
    slab.set_interior_detail_normal(true);
    slab.set_environment_cube(&ctx, Some(&scene_build::hangar::hangar_reflection_cube().mips));

    // The parked hero, in the live rest pose.
    let vehicle = VehicleKind::T54_1951;
    let spec = vehicle.spec();
    let snapshot = net::TankSnapshot {
        tank_id: TankId(0),
        team: TeamId(1),
        vehicle,
        position: [0.0, client::TURNTABLE_TOP_M, 0.0],
        yaw_rad: scene_build::hangar::HERO_PARK_YAW,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: spec.hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 0.0,
        module_hit_points: spec.module_health.hit_points_by_slot(),
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
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
    };
    let mut catalog = client::VehicleAssetCatalog::default();
    if let Err(error) = catalog.load_forge_artifact_tree("target/forge") {
        eprintln!("note: no Forge artifacts loaded ({error}); hero uses the neutral material");
    }
    let objects = client::tank_vehicle_render_objects_at_rest(
        &mut catalog,
        &snapshot,
        [0.72, 0.76, 0.62],
        &client::VehicleVariation::from_snapshot(&snapshot),
        0.0,
        0.0,
    );
    let hero_frame = client::render_frame_from_objects(objects);
    let empty_frame = client::render_frame_from_objects(Vec::new());
    let vehicle_meshes = catalog.take_pending_vehicle_meshes();
    let vehicle_materials = catalog.take_pending_vehicle_materials();
    for renderer in [&mut shipped, &mut unfurnished, &mut slab, &mut four_x] {
        for (handle, mesh) in &vehicle_meshes {
            renderer.register_vehicle_mesh(&ctx, *handle, mesh);
        }
        for (handle, maps) in &vehicle_materials {
            renderer.register_vehicle_material(&ctx, *handle, maps);
        }
        renderer.set_vehicle_render_frame(&ctx, &hero_frame);
    }

    #[derive(Clone, Copy)]
    enum Rig {
        Shipped,
        Unfurnished,
        Slab,
        FourX,
    }
    #[derive(Clone, Copy)]
    struct GarageConfig {
        name: &'static str,
        rig: Rig,
        grain: bool,
        shadows: bool,
        ssao: bool,
        /// Penumbra radius for the block — the shipped kernel unless a config isolates it.
        softness: f32,
        fx: bool,
        vehicle: bool,
    }
    const SOFT: f32 = scene_build::hangar::HANGAR_SHADOW_SOFTNESS;
    let shipped_cfg = GarageConfig {
        name: "garage full",
        rig: Rig::Shipped,
        grain: true,
        shadows: true,
        ssao: true,
        softness: SOFT,
        fx: true,
        vehicle: true,
    };
    // The attribution table (Hala v4 P1). Deltas vs [garage full] price, in order: the C1
    // grain flag, the whole shadow system, the whole AO system, the blended shaft quads, the
    // 8-tap penumbra alone, the hero's overdraw, the props' fill, the room minus the vehicle,
    // and the 4x candidate — the second half of the F2 go/no-go number.
    let configs = [
        shipped_cfg,
        GarageConfig { name: "no interior grain (C1)", grain: false, ..shipped_cfg },
        GarageConfig { name: "no shadows", shadows: false, ..shipped_cfg },
        GarageConfig { name: "no ssao", ssao: false, ..shipped_cfg },
        GarageConfig { name: "no fx (shaft blades)", fx: false, ..shipped_cfg },
        GarageConfig { name: "battle shadow kernel", softness: 0.0, ..shipped_cfg },
        GarageConfig { name: "hall only (no vehicle)", vehicle: false, ..shipped_cfg },
        GarageConfig { name: "unfurnished hall", rig: Rig::Unfurnished, ..shipped_cfg },
        GarageConfig { name: "vehicle on bare slab", rig: Rig::Slab, fx: false, ..shipped_cfg },
        GarageConfig { name: "garage @4x (F2 candidate)", rig: Rig::FourX, ..shipped_cfg },
    ];
    const CONFIGS: usize = 10;
    assert_eq!(configs.len(), CONFIGS, "the garage config table and its arrays disagree");
    let shaft_fx = client::hangar_shaft_fx_vertices();

    let projection = renderer_api::CameraProjectionPolicy::webgpu_default();
    let pivot = scene_build::hangar::hangar_camera_pivot();
    let camera_at = |t: f32| {
        // One full revolution per block, at the hero pitch, boom clamped by the same
        // arithmetic the live camera clamps with — every wall angle gets measured.
        let yaw = scene_build::hangar::HERO_ORBIT_YAW + t * std::f32::consts::TAU;
        let pitch = scene_build::hangar::HERO_ORBIT_PITCH;
        let boom = scene_build::hangar::HERO_ORBIT_DISTANCE
            .min(scene_build::hangar::max_orbit_boom(yaw, pitch));
        let eye = pivot + scene_build::hangar::orbit_direction(yaw, pitch) * boom;
        renderer_api::Camera {
            eye: eye.to_array(),
            target: pivot.to_array(),
            vertical_fov_degrees: scene_build::hangar::HERO_FOV_DEGREES,
        }
    };

    let block_frames = FRAMES / CYCLES;
    let mut samples: [Vec<f64>; CONFIGS] = std::array::from_fn(|_| Vec::new());
    let mut counts: [renderer_wgpu::FrameCounts; CONFIGS] = Default::default();
    let mut stats = client::RotationStats::new(configs.len(), CYCLES, renderer_wgpu::PassId::COUNT);

    for frame in 0..WARMUP {
        let camera = camera_at(frame as f32 / WARMUP as f32);
        let view_proj = renderer_api::view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        if shipped.render(&ctx, target.render_target(), view_proj, camera.eye).is_err() {
            println!("garage frame time: a render failed — skipped");
            return;
        }
    }
    let _ = target.read_rgba8(&ctx);

    for cycle in 0..CYCLES {
        for (config, cfg) in configs.iter().enumerate() {
            let renderer: &mut renderer_wgpu::SceneRenderer = match cfg.rig {
                Rig::Shipped => &mut shipped,
                Rig::Unfurnished => &mut unfurnished,
                Rig::Slab => &mut slab,
                Rig::FourX => &mut four_x,
            };
            renderer.set_interior_detail_normal(cfg.grain);
            renderer.set_shadows_enabled(cfg.shadows);
            renderer.set_ssao_enabled(cfg.ssao);
            renderer.set_shadow_softness(cfg.softness);
            renderer.set_fx(&ctx, if cfg.fx { &shaft_fx } else { &[] });
            renderer.set_vehicle_render_frame(
                &ctx,
                if cfg.vehicle { &hero_frame } else { &empty_frame },
            );
            for block_frame in 0..(BLOCK_WARMUP + block_frames) {
                let t_orbit = block_frame.saturating_sub(BLOCK_WARMUP) as f32 / block_frames as f32;
                let camera = camera_at(t_orbit);
                let view_proj = renderer_api::view_projection_matrix(
                    &camera,
                    width as f32 / height as f32,
                    projection.near_plane_m(),
                    projection.far_plane_m(),
                );
                let t = Instant::now();
                if renderer.render(&ctx, target.render_target(), view_proj, camera.eye).is_err() {
                    println!("garage frame time: a render failed — skipped");
                    return;
                }
                if target.read_rgba8(&ctx).is_err() {
                    println!("garage frame time: readback failed — skipped");
                    return;
                }
                if block_frame >= BLOCK_WARMUP {
                    samples[config].push(t.elapsed().as_secs_f64() * 1000.0);
                    counts[config] = renderer.last_frame_counts();
                    if let Some(timings) = renderer.read_pass_timings(&ctx) {
                        let per_pass: Vec<Option<f32>> = renderer_wgpu::PassId::ALL
                            .iter()
                            .map(|id| timings.pass_ms(*id))
                            .collect();
                        stats.record(config, cycle, timings.frame_ms(), &per_pass);
                    }
                }
            }
        }
    }

    // The fence's own price, measured the way the battle instrument measures it.
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
    for (config, cfg) in configs.iter().enumerate() {
        let series = &mut samples[config];
        series.sort_by(f64::total_cmp);
        let at = |p: f64| series[((series.len() as f64 * p) as usize).min(series.len() - 1)];
        if config == 0 {
            full_p50 = at(0.50);
        }
        let submitted = counts[config].total();
        println!(
            "garage frame @{width}x{height} [{}] ({} samples, {CYCLES} cycles): p50 {:.2} ms  p95 {:.2} ms  max {:.2} ms  (delta vs full {:+.2} ms p50)",
            cfg.name,
            series.len(),
            at(0.50),
            at(0.95),
            series[series.len() - 1],
            at(0.50) - full_p50,
        );
        println!(
            "    submitted: {} draws, {} triangles, {} instances",
            submitted.draws, submitted.triangles, submitted.instances,
        );
    }
    println!(
        "  garage budget: p50 {full_p50:.2} ms minus fence {fence_p50:.2} ms = {:.2} ms against \
         16.67 ms (one-look: meaningful only when this box IS the MX330; elsewhere read deltas)",
        full_p50 - fence_p50,
    );
    println!(
        "  F2 gate: the @4x block's delta vs [garage full] IS the measured 4x surcharge; \
         F2-GO asks (post-diet) @4x p50 minus fence <= 16.67 ms"
    );
    if let Some(problem) = stats.imbalance() {
        println!("  {problem}");
    } else if timed {
        println!("  garage per-pass GPU time (ms), per config:");
        for (config, cfg) in configs.iter().enumerate() {
            let frame = stats.frame(config);
            let residual = stats.unattributed(config);
            println!(
                "    [{}] GPU frame p50 {:.3}  p95 {:.3}  (outside passes p50 {:.3})",
                cfg.name, frame.p50, frame.p95, residual.p50
            );
            for (index, id) in renderer_wgpu::PassId::ALL.iter().enumerate() {
                let Some(pass) = stats.pass(config, index) else { continue };
                println!("      {:<18} p50 {:>7.3}  p95 {:>7.3}", id.label(), pass.p50, pass.p95);
            }
        }
    }

    // Fill-bound confirmation: the shipped config once more at 720p, compared on the pass
    // instrument's GPU frame (not wall clock, so the fence stays out of the ratio). This
    // block is sequential rather than rotated — it argues a RATIO against the rotation's
    // config 0, and only the ratio: near 2.25x for 2.25x the pixels reads fill/bandwidth-
    // bound, near 1.0x reads raster/fixed-cost.
    let Ok(target720) = renderer_wgpu::OffscreenTarget::new(&ctx, 1280, 720) else {
        println!("  720p target unavailable — fill-bound check skipped");
        return;
    };
    let cfg = shipped_cfg;
    shipped.set_interior_detail_normal(cfg.grain);
    shipped.set_shadows_enabled(cfg.shadows);
    shipped.set_ssao_enabled(cfg.ssao);
    shipped.set_shadow_softness(cfg.softness);
    shipped.set_fx(&ctx, &shaft_fx);
    shipped.set_vehicle_render_frame(&ctx, &hero_frame);
    let mut gpu_720_ms: Vec<f32> = Vec::new();
    for block_frame in 0..(BLOCK_WARMUP + block_frames) {
        let t_orbit = block_frame.saturating_sub(BLOCK_WARMUP) as f32 / block_frames as f32;
        let camera = camera_at(t_orbit);
        let view_proj = renderer_api::view_projection_matrix(
            &camera,
            1280.0 / 720.0,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        if shipped.render(&ctx, target720.render_target(), view_proj, camera.eye).is_err()
            || target720.read_rgba8(&ctx).is_err()
        {
            println!("  720p render failed — fill-bound check skipped");
            return;
        }
        if block_frame >= BLOCK_WARMUP
            && let Some(timings) = shipped.read_pass_timings(&ctx)
        {
            gpu_720_ms.push(timings.frame_ms());
        }
    }
    gpu_720_ms.sort_by(f32::total_cmp);
    if timed && !gpu_720_ms.is_empty() {
        let p50_720 = gpu_720_ms[gpu_720_ms.len() / 2];
        let p50_1080 = stats.frame(0).p50;
        println!(
            "  fill-bound check: GPU p50 {p50_1080:.2} ms @1080p vs {p50_720:.2} ms @720p \
             = x{:.2} for x2.25 pixels",
            p50_1080 / p50_720.max(0.01),
        );
    }
}
