//! The per-pass timing instrument: its identity list, and whether this machine can arm it.
//!
//! Nothing here times a frame — that lands with the recorder. What is locked now is the part that
//! would be expensive to get wrong later: the list of passes that budgets and measurements are
//! keyed by, and the fact that a shipped context still asks the device for nothing.

use renderer_wgpu::{FrameProfiler, GpuContext, GpuContextOptions, PassId};

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

/// The enum is only worth having if it names passes that EXIST. Every label has to appear in the
/// renderer's own sources, or the profiler is measuring a frame it imagined.
///
/// `frame_graph.rs` is excluded from the scan on purpose: it is where the labels are DEFINED, so
/// including it would let every variant satisfy this test with its own declaration — a gate that
/// passes by construction, which is the failure mode this repo already documents on
/// `REGEN_WIRE_FIXTURES`. Verified by removing it and watching this test go red.
#[test]
fn every_pass_id_names_a_pass_the_renderer_actually_encodes() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = String::new();
    collect_rust_sources(&src, "frame_graph.rs", &mut sources);
    assert!(!sources.is_empty(), "found no renderer sources to scan");

    let mut missing = Vec::new();
    for id in PassId::ALL {
        // `ssao_pass` / `ssao_blur_pass` reach their descriptor through a loop variable rather
        // than a literal at the call site, so the literal is accepted anywhere in the renderer.
        if !sources.contains(&format!("\"{}\"", id.label())) {
            missing.push(format!("{id:?} -> {}", id.label()));
        }
    }

    assert!(
        missing.is_empty(),
        "these PassId variants name a pass no call site opens:\n  {}",
        missing.join("\n  ")
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
