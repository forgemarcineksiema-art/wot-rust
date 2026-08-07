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
