//! Architecture gate: a render pass is opened in exactly one place, so it cannot be opened
//! unmeasured.
//!
//! An instrument wired into ten call sites decays. The eleventh pass gets added by someone with no
//! reason to know a profiler exists, it opens its own `RenderPassDescriptor` with
//! `timestamp_writes: None`, and from then on the frame's per-pass timings quietly sum to less
//! than the frame — with nothing anywhere reporting a gap. That is the same shape as the two
//! instrument failures this project has already paid for: a frame capture that drew no vehicles,
//! and a capture that rendered at a sample count the game does not ship.
//!
//! So the descriptor is not the call site's to fill in. `PassRecorder::begin` takes a `PassId` and
//! derives the label and the timestamp writes from it; a new pass is instrumented by construction
//! or it does not compile. This rule is what keeps that true.

use std::fs;

use quality::workspace::{crate_src_dir, repo_relative, rust_files, workspace_root};

/// The module that owns pass creation.
const RECORDER: &str = "pass_recorder.rs";

/// Files allowed to open a pass outside the recorder, and why. Each entry is a debt or a
/// deliberate exception — never a convenience.
const ALLOWLIST: &[(&str, &str)] = &[(
    "offscreen.rs",
    "`clear_color` is a GPU smoke helper that fills a target with one colour; it is not part of a \
     rendered frame, has no PassId, and belongs to no budget. It would become a frame pass only \
     if something started drawing through it.",
)];

#[test]
fn a_render_pass_is_only_opened_by_the_recorder() {
    let root = workspace_root();
    let src = crate_src_dir(&root, "renderer_wgpu");
    let mut offenders = Vec::new();

    for path in rust_files(&src) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if name == RECORDER || ALLOWLIST.iter().any(|(file, _)| *file == name) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else { continue };
        for (index, line) in source.lines().enumerate() {
            if line.contains("begin_render_pass(") {
                offenders.push(format!("{}:{}", repo_relative(&path, &root), index + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a render pass is opened outside {RECORDER}:\n  {}\n\nOpen it through \
         `PassRecorder::begin(encoder, PassId::…, colour, depth)` instead — the label and the \
         timestamp writes come from the PassId, so the pass is measured by construction. A pass \
         that genuinely is not part of a frame goes in this test's ALLOWLIST with its reason.",
        offenders.join("\n  ")
    );
}

/// The allowlist may only name files that exist and that actually open a pass — otherwise it
/// rots into a list of permissions nobody needs, which is how an exception outlives its reason.
#[test]
fn the_allowlist_describes_call_sites_that_actually_exist() {
    let root = workspace_root();
    let src = crate_src_dir(&root, "renderer_wgpu");
    let mut stale = Vec::new();

    for (file, _) in ALLOWLIST {
        let found = rust_files(&src).into_iter().find(|path| {
            path.file_name().and_then(|n| n.to_str()) == Some(*file)
                && fs::read_to_string(path)
                    .map(|text| text.contains("begin_render_pass("))
                    .unwrap_or(false)
        });
        if found.is_none() {
            stale.push(*file);
        }
    }

    assert!(
        stale.is_empty(),
        "these files are excused from the recorder rule but no longer open a pass — delete the \
         entries:\n  {}",
        stale.join("\n  ")
    );
}

/// No module but the recorder may so much as NAME `wgpu::RenderPass`.
///
/// This is the counting half of the same rule, and it is deliberately stated as "cannot hold the
/// type" rather than "must not call draw". `CountedPass` owns the pass privately and exposes only
/// counting draws, so an uncounted draw is not something a call site can write — it would first
/// have to get its hands on the raw pass. Forbidding the type forbids that, and it is a rule a
/// text scan can actually check, unlike "did you call the counting draw or the other one".
#[test]
fn no_draw_bypasses_the_counter() {
    let root = workspace_root();
    let src = crate_src_dir(&root, "renderer_wgpu");
    let mut offenders = Vec::new();

    for path in rust_files(&src) {
        if path.file_name().and_then(|n| n.to_str()) == Some(RECORDER) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else { continue };
        for (index, line) in source.lines().enumerate() {
            // The descriptor types are fine — they are how a pass is described, not held.
            if line.contains("wgpu::RenderPass<")
                || line.contains("wgpu::RenderPass)")
                || line.contains("wgpu::RenderPass ")
            {
                offenders.push(format!("{}:{}", repo_relative(&path, &root), index + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a raw `wgpu::RenderPass` is held outside {RECORDER}:\n  {}\n\nTake \
         `&mut CountedPass<'_, '_>` instead. A raw pass can be drawn into without counting, and a \
         frame whose draw total quietly misses one is worse than no counter at all.",
        offenders.join("\n  ")
    );
}

/// The triangle arithmetic divides vertex and index counts by three, which is only true because
/// every pipeline in this renderer is a triangle list. A strip or a fan would not fail anything —
/// it would silently report a third of the triangles it drew, and a budget would be set against
/// the wrong number.
#[test]
fn every_pipeline_is_a_triangle_list() {
    let root = workspace_root();
    let src = crate_src_dir(&root, "renderer_wgpu");
    let mut offenders = Vec::new();

    for path in rust_files(&src) {
        let Ok(source) = fs::read_to_string(&path) else { continue };
        for (index, line) in source.lines().enumerate() {
            if line.contains("PrimitiveTopology::")
                && !line.contains("PrimitiveTopology::TriangleList")
            {
                offenders.push(format!("{}:{}", repo_relative(&path, &root), index + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a pipeline uses a topology the triangle counter cannot read:\n  {}\n\nThe counter \
         divides by three. Teach it this topology in `CountedPass::draw*` before shipping the \
         pipeline, or the frame will under-report its geometry.",
        offenders.join("\n  ")
    );
}
