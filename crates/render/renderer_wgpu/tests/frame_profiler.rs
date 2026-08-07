//! The per-pass timing instrument: its identity list, whether this machine can arm it, and
//! whether an armed frame actually comes back with numbers in it.
//!
//! What is locked here is the part that would be expensive to get wrong later: the list of passes
//! that budgets and measurements are keyed by, the fact that a shipped context still asks the
//! device for nothing, and the end-to-end proof that the GPU fills the query set a real frame
//! wrote — so the readback built on top of it is built on something seen working.

use renderer_api::view_projection_matrix;
use renderer_wgpu::{
    FrameProfiler, GpuContext, GpuContextOptions, OffscreenTarget, PassId, SceneRenderer,
};

/// The pass list, pinned in encoding order.
///
/// Budgets, recorded frame times and the profiler's timestamp slots are all keyed by `PassId`, so
/// a reorder does not fail loudly — it silently re-keys the register and reports every number
/// against the wrong pass. Appending is fine and expected; anything else has to come here first
/// and say why.
const EXPECTED: &[(&str, &str)] = &[
    ("ShadowNear", "shadow_pass"),
    ("ShadowFar", "shadow_pass_far"),
    ("SsaoPrepass", "ssao_prepass"),
    ("Ssao", "ssao_pass"),
    ("SsaoBlur", "ssao_blur_pass"),
    ("SceneOpaque", "scene_opaque_pass"),
    ("SceneWater", "scene_water_pass"),
    ("Scene", "scene_pass"),
    ("Bloom", "bloom_pass"),
    ("Post", "post_pass"),
    ("Fxaa", "fxaa_pass"),
];

#[test]
fn pass_ids_are_append_only_and_carry_their_wgpu_labels() {
    let actual: Vec<(String, &str)> =
        PassId::ALL.iter().map(|id| (format!("{id:?}"), id.label())).collect();
    let expected: Vec<(String, &str)> =
        EXPECTED.iter().map(|(name, label)| ((*name).to_string(), *label)).collect();

    assert_eq!(
        actual, expected,
        "the pass list changed. Appending is fine — reordering or renaming re-keys every budget \
         and every recorded measurement against the wrong pass."
    );
    assert_eq!(PassId::COUNT, EXPECTED.len());
    for (index, id) in PassId::ALL.iter().enumerate() {
        assert_eq!(id.index(), index, "{id:?} reports the wrong slot");
    }
}

/// The enum is only worth having if every variant names a pass something actually opens. A
/// `PassId` nobody encodes is a row in the budget table that will never be filled and a slot in
/// the register that will always read zero.
///
/// This is the second shape of this test. The first checked that every label appeared as a
/// literal at a `begin_render_pass` call site — a true statement until the recorder took the
/// descriptor away from the call sites, which is exactly the point of the recorder. What survives
/// the change is the question underneath: does anything actually ask for this pass?
///
/// `frame_graph.rs` is excluded because it DEFINES the variants; including it would let every one
/// of them satisfy this test with its own declaration, a gate that passes by construction.
#[test]
fn every_pass_id_is_asked_for_by_a_call_site() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = String::new();
    collect_rust_sources(&src, "frame_graph.rs", &mut sources);
    assert!(!sources.is_empty(), "found no renderer sources to scan");

    let mut unused = Vec::new();
    for id in PassId::ALL {
        let needle = format!("PassId::{id:?}");
        // `Scene` is a prefix of `SceneOpaque`, `Ssao` of `SsaoBlur` — a bare `contains` would
        // let the shorter variant ride on the longer one's call site.
        let referenced = sources.match_indices(&needle).any(|(at, _)| {
            sources[at + needle.len()..]
                .chars()
                .next()
                .is_none_or(|next| !next.is_alphanumeric() && next != '_')
        });
        if !referenced {
            unused.push(format!("{id:?}"));
        }
    }

    assert!(
        unused.is_empty(),
        "these PassId variants are never asked for by any call site:
  {}",
        unused.join(
            "
  "
        )
    );
}

fn collect_rust_sources(dir: &std::path::Path, skip_file: &str, out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, skip_file, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && path.file_name().and_then(|n| n.to_str()) != Some(skip_file)
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            out.push_str(&text);
            out.push('\n');
        }
    }
}

/// Whether this box can time a frame at all, answered by the instrument rather than by the
/// adapter's advertisement. A device built WITHOUT the feature must report `Unavailable` with a
/// reason, never a silently dead `Active`.
#[test]
fn the_profiler_reports_what_this_device_can_actually_do() {
    let Ok(plain) = GpuContext::headless() else {
        eprintln!("skipping profiler negotiation test: no headless adapter");
        return;
    };

    // A shipped context asked for nothing, so arming must refuse — with a reason.
    let refused = FrameProfiler::new(&plain.device, &plain.queue, true);
    assert!(refused.active().is_none(), "a device without the feature must not arm");
    assert!(
        refused.unavailable_reason().is_some(),
        "an unavailable instrument must say WHY — 'no numbers' and 'no numbers because this \
         device was not built for it' are different facts"
    );
    assert!(
        FrameProfiler::new(&plain.device, &plain.queue, false).unavailable_reason().is_none(),
        "nobody asked for timing, so there is nothing to explain"
    );

    // A context that stated the purpose arms, on any adapter that supports it.
    let Ok(timed) = GpuContext::headless_with_options(GpuContextOptions { pass_timing: true })
    else {
        eprintln!("skipping timed-context half: no headless adapter");
        return;
    };
    let profiler = FrameProfiler::new(&timed.device, &timed.queue, true);
    let info = timed.adapter.get_info();
    match profiler.active() {
        Some(active) => {
            // Printed, not just asserted: whether the min spec can time its own frame decides
            // whether the per-pass instrument is the real plan or the fallback one.
            eprintln!(
                "per-pass timing ARMED on {} ({:?}) — {} ns/tick, {} slots",
                info.name,
                info.backend,
                active.period_ns(),
                PassId::COUNT * 2,
            );
            assert!(
                active.period_ns() > 0.0,
                "{}: timestamps without a tick period are unreadable",
                info.name
            );
            let (begin, end) = active.slots(PassId::Fxaa);
            assert_eq!(end, begin + 1, "each pass owns a begin/end pair");
            assert!(end < PassId::COUNT as u32 * 2, "the last pass must fit inside the query set");
        }
        None => {
            // Legitimate on an adapter without the feature; the reason has to name it.
            eprintln!("per-pass timing UNAVAILABLE on {} ({:?})", info.name, info.backend);
            assert!(
                profiler.unavailable_reason().is_some(),
                "{}: refused to arm without saying why",
                info.name
            );
        }
    }
}

/// The armed instrument riding a REAL frame: the first time timestamps are written by the
/// renderer rather than by a test's own encoder. Proves three things at once — the descriptor
/// wgpu accepts, the resolve of a partial query set the frame actually wrote, and that the ticks
/// come back ordered and non-zero. Without this the next change would be building a readback for
/// numbers nobody had seen the GPU produce.
#[test]
fn an_armed_frame_writes_timestamps_the_gpu_actually_fills_in() {
    let Ok(ctx) = GpuContext::headless_with_options(GpuContextOptions { pass_timing: true }) else {
        eprintln!("skipping armed-frame test: no headless adapter");
        return;
    };
    let profiler = FrameProfiler::new(&ctx.device, &ctx.queue, true);
    if profiler.active().is_none() {
        eprintln!("skipping armed-frame test: {}", profiler.unavailable_reason().unwrap_or("?"));
        return;
    }

    let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    renderer.set_pass_profiler(profiler);

    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    renderer
        .render(
            &ctx,
            target.render_target(),
            view_projection_matrix(&camera, 1.0, 0.1, 20.0),
            camera.eye,
        )
        .expect("an armed frame must render");

    let active = renderer.pass_profiler().active().expect("still armed after the frame");
    let readback = active.readback_buffer();
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let mapped = readback.slice(..).get_mapped_range();
    let ticks: Vec<u64> = mapped
        .chunks_exact(8)
        .map(|b| u64::from_le_bytes(b.try_into().expect("8 bytes")))
        .collect();
    drop(mapped);
    readback.unmap();

    // The first pass of any frame is the near shadow map, so slots 0 and 1 are always written.
    assert!(ticks.len() >= 2, "the query set came back too small");
    assert!(ticks[0] > 0, "the first pass never wrote its start timestamp");
    assert!(ticks[1] > ticks[0], "a pass ended before it began: {} -> {}", ticks[0], ticks[1]);
    eprintln!(
        "first pass ({:?}) spanned {} ticks -> {:.3} ms",
        PassId::ShadowNear,
        ticks[1] - ticks[0],
        (ticks[1] - ticks[0]) as f64 * f64::from(active.period_ns()) / 1.0e6,
    );
}

/// The readback the aggregation is about to be built on: every pass the frame encoded comes back
/// with a positive duration, and the parts never exceed the whole.
///
/// The residual between them is the point of reporting `frame_ms` separately. A per-pass table
/// that silently sums to less than the frame invites the reader to believe the passes are the
/// whole story, and the gap — resolves, transitions, waiting on a previous submit — is exactly
/// where an unexplained millisecond would hide.
#[test]
fn a_timed_frame_reads_back_a_table_whose_parts_fit_inside_the_whole() {
    let Ok(ctx) = GpuContext::headless_with_options(GpuContextOptions { pass_timing: true }) else {
        eprintln!("skipping timing readback test: no headless adapter");
        return;
    };
    let profiler = FrameProfiler::new(&ctx.device, &ctx.queue, true);
    if profiler.active().is_none() {
        eprintln!(
            "skipping timing readback test: {}",
            profiler.unavailable_reason().unwrap_or("?")
        );
        return;
    }

    let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    renderer.set_pass_profiler(profiler);
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    renderer
        .render(
            &ctx,
            target.render_target(),
            view_projection_matrix(&camera, 1.0, 0.1, 20.0),
            camera.eye,
        )
        .expect("render");
    let _ = target.read_rgba8(&ctx);

    let timings = renderer.read_pass_timings(&ctx).expect("an armed frame must report timings");
    let mut named = 0;
    for id in PassId::ALL {
        if let Some(ms) = timings.pass_ms(*id) {
            assert!(ms >= 0.0, "{} reported a negative duration", id.label());
            eprintln!("{:<18} {:.4} ms", id.label(), ms);
            named += 1;
        }
    }
    assert!(named >= 3, "a frame that drew something encoded more than {named} passes");
    assert!(
        timings.sum_ms() <= timings.frame_ms() + 1.0e-3,
        "the passes ({:.4} ms) cannot outweigh the frame ({:.4} ms)",
        timings.sum_ms(),
        timings.frame_ms()
    );
    eprintln!(
        "frame {:.4} ms, passes {:.4} ms, unattributed {:.4} ms",
        timings.frame_ms(),
        timings.sum_ms(),
        timings.unattributed_ms()
    );
}
