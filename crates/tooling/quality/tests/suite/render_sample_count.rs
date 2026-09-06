//! Architecture gate: the frame's sample count is resolved in ONE place, so an instrument and
//! the frame it claims to measure cannot drift apart.
//!
//! They did drift. `OffscreenTarget::new` took `DEFAULT_MSAA_SAMPLES` straight (4x) while
//! `WindowRenderer::new` ran the same constant through `resolve_msaa_samples`, which folds every
//! adapter to 1x under the one-look policy. Two resolutions of one fact, forty lines apart, and
//! nothing put them on the same page. The result was not a cosmetic mismatch:
//!
//! - every frame-time number this project has ever quoted — 16.83 ms p50, 20.48 ms p95, the
//!   16.116 ms Ostrogorsk flora reading — was captured at 4x MSAA, which pays `sample_count x`
//!   fill on every blended pass, on a GPU the renderer's own comment calls bandwidth-bound first;
//! - every committed golden PNG is a multisampled picture, so the frames this project reviews its
//!   own art direction on are cleaner than the frame a player is shown, exactly where it matters
//!   most — thin, distant geometry.
//!
//! The rule is what stops a third resolution appearing. `resolve_msaa_samples` is the policy;
//! callers ask for a PURPOSE instead — `shipped_sample_count` when the frame is meant to be the
//! game, `review_sample_count` when it is meant to be looked at — and those two live next to each
//! other where the difference between them is impossible to miss.

use std::fs;

use quality::workspace::{crate_src_dir, repo_relative, rust_files, workspace_root};

/// The module that owns the policy; the only file allowed to apply it.
const OWNER: &str = "msaa.rs";

#[test]
fn the_frame_sample_count_is_resolved_in_one_place() {
    let root = workspace_root();
    let src = crate_src_dir(&root, "renderer_wgpu");
    let mut offenders = Vec::new();

    for path in rust_files(&src) {
        if path.file_name().and_then(|name| name.to_str()) == Some(OWNER) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else { continue };
        for (index, line) in source.lines().enumerate() {
            if line.contains("resolve_msaa_samples(") {
                offenders.push(format!("{}:{}", repo_relative(&path, &root), index + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the MSAA policy is applied outside {OWNER}, which is how the offscreen instrument and \
         the window came to render different pictures.\n  {}\n\nAsk for a purpose instead: \
         `shipped_sample_count` if this frame is meant to BE the game, `review_sample_count` if \
         it is meant to be LOOKED AT.",
        offenders.join("\n  ")
    );
}
