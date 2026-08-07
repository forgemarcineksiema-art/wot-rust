//! What a battle costs as it AGES — the axis every other perf probe is blind to.
//!
//! `perf_capture` and its siblings all measure a SCENE: a fixed camera path over a pristine
//! field with an empty crater ledger. That instrument answers "is this content too heavy", and
//! it answered it. It cannot answer the other complaint — the game holds 60 FPS and then drops,
//! suddenly and hard, in the LATE phase of a battle. Nothing about the scene changed there. What
//! changed is how much battle has already happened.
//!
//! Everything below hangs off one fact: the replicated crater ledger holds `sim::MAX_CRATERS`
//! records and, once full, recycles its oldest. A long match does not approach that cap, it SITS
//! at it — so from that point on every further ground impact moves the ledger, and everything
//! keyed on the ledger re-fires on every shot for the rest of the match.
//!
//! Three costs hang off that trigger, and this probe times all three against crater count:
//!
//!   * **grass** — `render.rs` repopulates the near-field cache SYNCHRONOUSLY inside the render
//!     call whenever the ledger fingerprint moves, and the repopulation filters every tuft
//!     against every crater;
//!   * **the two bakes** — ground and card meadow. Both run on a worker (deliberately: a comment
//!     in `render.rs` records that the memcpy was moved off the render thread on purpose), but
//!     the meshes they produce are UPLOADED from the render thread in the harvest frame, so the
//!     frame still pays their size;
//!   * **the FX ledger** — terrain scars are built once and copied, with a fade applied, into a
//!     fresh per-frame vertex buffer. That buffer is built whether or not it fits the GPU one.
//!
//! Run: `cargo run -p client --release --example probe -- battle_age_cost`

use std::time::Instant;

/// The 1000 m battle map — the one a 7v7 actually chews up.
const MAP: terrain::MapId = terrain::MapId::ProkhorovkaHill252_2;

/// `sim::MAX_CRATERS`, named here rather than imported so this probe does not pull the whole
/// server crate in for one integer. The rungs below walk up to it because a long battle sits on
/// it rather than approaching it.
const LEDGER_CAP: usize = 256;

/// `renderer_wgpu`'s `FX_VERTEX_CAPACITY`. It is a private const there, and — unlike
/// `vehicle_instance_budget()` and `armor_damage_aperture_budget()` — it is neither exported nor
/// locked by any test. That asymmetry is half of what this probe is here to show.
const FX_BUFFER_BYTES: usize = 1 << 19;

pub(crate) fn run() {
    let pristine = map_forge::battlefield(MAP);
    let maps = client::bake_terrain_ground_maps(&pristine);
    let materials = client::terrain_material_set_for(MAP);
    let extent = pristine.heightmap.extent_m();
    let (eye_x, eye_z) = (extent[0] * 0.5, extent[1] * 0.5);
    let eye = glam::Vec3::new(
        eye_x,
        pristine.heightmap.sample_height(eye_x, eye_z).unwrap_or(0.0) + 2.0,
        eye_z,
    );

    println!("battle age cost — {MAP:?}, {:.0} x {:.0} m", extent[0], extent[1]);
    println!(
        "  (release build; every number is the MEDIAN of repeated runs, one timing is noise)\n"
    );

    println!("A. THE CRATER CHAIN — what one further ground impact makes the client redo.");
    println!("   grass is synchronous in the render call; the two bakes run on a worker but the");
    println!("   frame that harvests them uploads their meshes from the render thread.\n");
    println!(
        "  {:>7}  {:>10}  {:>11}  {:>11}  {:>13}  {:>13}",
        "craters", "grass ms", "ground bake", "meadow bake", "ground upload", "meadow upload"
    );

    for &count in &[0usize, 32, 128, LEDGER_CAP] {
        let mut field = pristine.clone();
        field.heightmap.set_craters(&ledger(count, extent));

        let grass_ms = timed(9, || {
            client::grass_frame_objects(
                &field.heightmap,
                field.water,
                &field.static_cover,
                &maps,
                &materials,
                eye,
            )
        });
        let ground_ms = timed(3, || client::battlefield_ground_mesh(&field));
        let meadow_ms = timed(3, || client::grass_card_dressing_mesh(&field, &maps, &materials));

        // What the harvest frame hands the GPU: both meshes, WHOLE, every time — the harvest
        // re-uploads the entire ground and the entire card meadow, not the cells that changed.
        let (ground_v, ground_i) = client::battlefield_ground_mesh(&field);
        let (meadow_v, meadow_i) = client::grass_card_dressing_mesh(&field, &maps, &materials);
        let ground_mib =
            (size_of_val(&*ground_v) + size_of_val(&*ground_i)) as f64 / (1024.0 * 1024.0);
        let meadow_mib =
            (size_of_val(&*meadow_v) + size_of_val(&*meadow_i)) as f64 / (1024.0 * 1024.0);

        println!(
            "  {count:>7}  {grass_ms:>10.2}  {ground_ms:>9.1} ms  {meadow_ms:>9.1} ms  \
             {ground_mib:>9.1} MiB  {meadow_mib:>9.1} MiB",
        );
    }

    println!(
        "\n  Read the last row as the steady state of a long match, and the first as its opening."
    );
    println!(
        "  The two bakes are a worker's problem. The two UPLOADS are the render thread's, and it"
    );
    println!("  pays them whole on the frame the bake lands.");

    // How much of that upload the new crater actually justifies. The harvest re-uploads both
    // meshes WHOLE; this is what a partial upload would have had to carry instead, and it is the
    // number the shape of any fix hangs on.
    let mut before = pristine.clone();
    before.heightmap.set_craters(&ledger(LEDGER_CAP - 1, extent));
    let mut after = pristine.clone();
    after.heightmap.set_craters(&ledger(LEDGER_CAP, extent));
    report_delta(
        "ground",
        client::battlefield_ground_mesh(&before),
        client::battlefield_ground_mesh(&after),
    );
    report_delta(
        "meadow",
        client::grass_card_dressing_mesh(&before, &maps, &materials),
        client::grass_card_dressing_mesh(&after, &maps, &materials),
    );

    println!("\nB. THE FX LEDGER — what the FX layer asks for, against what the buffer holds.");
    let vertex_bytes = size_of::<renderer_api::FxVertex>();
    let capacity = FX_BUFFER_BYTES / vertex_bytes;
    println!(
        "  FxVertex is {vertex_bytes} B; the FX buffer is {} KiB, so it holds {capacity} vertices.",
        FX_BUFFER_BYTES / 1024,
    );

    let mut scars = client::TerrainScars::default();
    let mut recorded = 0usize;
    // Twice the cap, so the pool is saturated the way a shelled field saturates it.
    for index in 0..(LEDGER_CAP * 2) {
        scars.record(&impact(index, extent), &pristine.heightmap);
        recorded += 1;
    }
    // The per-frame build, exactly as `fx_frame_vertices` does it: a fresh unsized `Vec`, filled
    // by appending every live mark with its current fade applied.
    let build_ms = timed(9, || {
        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        vertices
    });
    let mut vertices = Vec::new();
    scars.append_quads(&mut vertices);
    println!(
        "  {recorded} impacts recorded -> {} vertices per frame ({:.0} KiB), built in {build_ms:.2} ms",
        vertices.len(),
        (vertices.len() * vertex_bytes) as f64 / 1024.0,
    );
    if vertices.len() > capacity {
        println!(
            "  OVER BUDGET by {:.1}x — and `set_fx` answers an oversized upload with a bare",
            vertices.len() as f64 / capacity as f64,
        );
        println!("  `return`, so the WHOLE FX layer stops drawing. Terrain scars alone do this.");
    }
    // How much battle it takes to get there, in marks.
    let mut probe_scars = client::TerrainScars::default();
    let mut fill_at = None;
    for index in 0..(LEDGER_CAP * 2) {
        probe_scars.record(&impact(index, extent), &pristine.heightmap);
        let mut probe_vertices = Vec::new();
        probe_scars.append_quads(&mut probe_vertices);
        if probe_vertices.len() > capacity {
            fill_at = Some(index + 1);
            break;
        }
    }
    match fill_at {
        Some(marks) => {
            println!("  The buffer is full at {marks} ground impacts — with nothing else in it: no",)
        }
        None => println!("  Terrain scars alone never fill it; the ceiling is elsewhere."),
    }
    if fill_at.is_some() {
        println!("  particles, no ruts, no tracers, no hull decals, all of which share it.");
    }

    println!(
        "\nC. THE PER-FRAME FLEET BUILD — running gear, rebuilt for all 14 tanks every frame."
    );
    match vehicle_geometry::RunningGearKinematics::for_vehicle(game_core::VehicleKind::T54_1951) {
        Some(kin) => {
            let gear_ms = timed(50, || {
                let mut placements = 0usize;
                for tank in 0..14 {
                    let phase = tank as f32 * 0.37;
                    placements += vehicle_geometry::running_gear_placements_dynamic(
                        &kin,
                        phase,
                        phase,
                        vehicle_geometry::GearDynamics::default(),
                    )
                    .len();
                }
                placements
            });
            let per_tank = vehicle_geometry::running_gear_placements_dynamic(
                &kin,
                0.0,
                0.0,
                vehicle_geometry::GearDynamics::default(),
            )
            .len();
            println!(
                "  {per_tank} placements per tank, {} for a 7v7: {gear_ms:.3} ms per frame,",
                per_tank * 14,
            );
            println!("  in 14 freshly allocated unsized `Vec`s that are dropped the same frame.");
        }
        None => println!("  T-54 has no running-gear kinematics; nothing to measure."),
    }
}

/// How much of a whole-mesh re-upload one further crater actually justifies: how many vertices
/// differ at all, and the span between the first and the last of them — a buffer write is a
/// RANGE, so no partial upload can ever be tighter than that span.
fn report_delta(
    label: &str,
    before: (Vec<renderer_api::SceneVertex>, Vec<u32>),
    after: (Vec<renderer_api::SceneVertex>, Vec<u32>),
) {
    let (old_v, _) = before;
    let (new_v, _) = after;
    let vertex_bytes = size_of::<renderer_api::SceneVertex>();
    let whole_mib = (new_v.len() * vertex_bytes) as f64 / (1024.0 * 1024.0);
    if old_v.len() != new_v.len() {
        println!(
            "  {label}: one more crater moves the vertex COUNT ({} -> {}), so everything after the",
            old_v.len(),
            new_v.len(),
        );
        println!("    first change shifts — a partial upload needs a stable layout first.");
        return;
    }
    let mut first = None;
    let mut last = 0usize;
    let mut changed = 0usize;
    for (index, (old, new)) in old_v.iter().zip(new_v.iter()).enumerate() {
        if old != new {
            first.get_or_insert(index);
            last = index;
            changed += 1;
        }
    }
    match first {
        None => println!(
            "  {label}: one more crater changes NOTHING — the whole {whole_mib:.1} MiB re-upload is waste."
        ),
        Some(first) => {
            let span = last - first + 1;
            println!(
                "  {label}: one more crater changes {changed} of {} vertices ({:.4}%), inside a span",
                new_v.len(),
                100.0 * changed as f64 / new_v.len() as f64,
            );
            println!(
                "    of {span} vertices = {:.2} MiB, against a {whole_mib:.1} MiB whole-mesh upload.",
                (span * vertex_bytes) as f64 / (1024.0 * 1024.0),
            );
        }
    }
}

/// A plausible ledger of `count` high-explosive craters scattered through the contested middle of
/// the map — battles chew the centre, not the spawn corners. Sized by the same derivation the sim
/// uses, and deterministic, so the ladder compares crater COUNT and nothing else.
fn ledger(count: usize, extent: [f32; 2]) -> Vec<terrain::CraterRecord> {
    let radius = terrain::he_crater_radius_m(122.0);
    let depth = terrain::he_crater_depth_m(122.0);
    let mut seed = 0x5eed_2026_u64;
    (0..count)
        .map(|_| {
            let x = extent[0] * (0.25 + game_core::math::next_hash_unit(&mut seed) * 0.5);
            let z = extent[1] * (0.25 + game_core::math::next_hash_unit(&mut seed) * 0.5);
            terrain::CraterRecord::from_world(
                x,
                z,
                radius,
                depth,
                terrain::CRATER_KIND_HIGH_EXPLOSIVE,
            )
        })
        .collect()
}

/// One high-explosive round dying on the ground, at the same spread as [`ledger`] — the scars and
/// the craters must land on the same field, because in a battle they are the same events.
fn impact(index: usize, extent: [f32; 2]) -> game_core::ShellImpact {
    let mut seed = 0x5eed_2026_u64 ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let x = extent[0] * (0.25 + game_core::math::next_hash_unit(&mut seed) * 0.5);
    let z = extent[1] * (0.25 + game_core::math::next_hash_unit(&mut seed) * 0.5);
    game_core::ShellImpact {
        position: glam::Vec3::new(x, 0.0, z),
        surface: game_core::ImpactSurface::Terrain,
        shell_type: game_core::ShellType::HighExplosive,
        direction: glam::Vec3::new(0.6, -0.3, 0.74).normalize(),
        caliber_mm: 122.0,
        ..Default::default()
    }
}

/// Median of `runs` timings, in milliseconds. A single timing on a laptop measures the laptop.
fn timed<T>(runs: usize, mut body: impl FnMut() -> T) -> f64 {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = Instant::now();
        let out = body();
        let elapsed = start.elapsed();
        std::hint::black_box(out);
        samples.push(elapsed.as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}
