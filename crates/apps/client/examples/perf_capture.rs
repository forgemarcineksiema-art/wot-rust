//! Release-mode CPU timing capture for the world-side hot costs (Płynność 2.0 / P0): the
//! one-off bakes (battle-scene hitch sizes) and the true per-frame costs the render loop pays
//! (grass conjuring). Run: `cargo run -p client --release --example perf_capture`

use std::time::Instant;

fn main() {
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
    // sign-off is measured, not promised.
    let city = map_forge::battlefield(terrain::MapId::Ostrogorsk);
    let born = terrain::initial_cover_phase_bytes(&city.static_cover);
    let t = Instant::now();
    let mut city_buckets = client::battlefield_statics_buckets(&city, &born, &[]);
    let (cv, ci) = client::assemble_statics_mesh(&city_buckets);
    println!(
        "ostrogorsk statics bake ({} boxes): {:.1} ms ({} v / {} i)",
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
