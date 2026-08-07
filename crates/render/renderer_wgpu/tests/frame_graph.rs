//! The frame as data, checked against itself.
//!
//! `FRAME_GRAPH` describes the frame the renderer encodes today — nothing consumes it yet, and
//! these locks are what make that description worth having. A table nobody checks is a comment
//! with syntax highlighting; this one has to survive being read back for every combination of the
//! three switches that gate its passes.
//!
//! Every check here is CPU-only. That matters: the frame graph is the safety net the rest of this
//! wave hangs from, and a net that only runs where there is a GPU would not be under the parts of
//! the gate that always run.

use std::collections::BTreeSet;

use renderer_wgpu::{FRAME_GRAPH, FrameResource, PassId, PassNode};

/// Every combination of the three switches that decide which passes exist: SSAO, bloom, and the
/// refraction grab. Eight frames, and the graph has to be coherent in all of them — including the
/// shipped one, where SSAO is on, bloom is off and refraction is off.
fn combinations() -> impl Iterator<Item = (bool, bool, bool)> {
    (0..8u8).map(|bits| (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0))
}

fn enabled(ssao: bool, bloom: bool, refraction: bool) -> Vec<&'static PassNode> {
    FRAME_GRAPH.iter().filter(|node| node.condition.holds(ssao, bloom, refraction)).collect()
}

/// The ordering property the whole graph exists to state: nothing reads what nothing has written.
///
/// Checked per combination rather than once, because the conditional passes are exactly where an
/// ordering mistake would hide — a frame with SSAO off must not leave the world sampling a buffer
/// no pass produced.
#[test]
fn every_resource_a_pass_reads_is_written_earlier() {
    for (ssao, bloom, refraction) in combinations() {
        let mut written: BTreeSet<FrameResource> = BTreeSet::new();
        for node in enabled(ssao, bloom, refraction) {
            for resource in node.reads {
                assert!(
                    written.contains(resource),
                    "{:?} reads {resource:?}, which nothing wrote yet \
                     (ssao={ssao}, bloom={bloom}, refraction={refraction})",
                    node.id
                );
            }
            written.extend(node.writes.iter().copied());
        }
    }
}

/// A frame produces exactly one picture, and it is the last thing it does. If two passes wrote the
/// output, the one that ran second would be the frame and the other would be wasted work nobody
/// could see.
#[test]
fn the_output_is_written_once_and_last() {
    for (ssao, bloom, refraction) in combinations() {
        let passes = enabled(ssao, bloom, refraction);
        let writers: Vec<PassId> = passes
            .iter()
            .filter(|node| node.writes.contains(&FrameResource::Output))
            .map(|node| node.id)
            .collect();
        assert_eq!(
            writers.len(),
            1,
            "the output must have exactly one author (ssao={ssao}, bloom={bloom}, \
             refraction={refraction}), found {writers:?}"
        );
        assert_eq!(
            passes.last().expect("a frame encodes something").id,
            writers[0],
            "the pass that writes the output must be the last one encoded"
        );
    }
}

/// An optional read is a claim that a placeholder is bound when the resource is absent. The claim
/// is only honest if the resource is a real product of some pass — otherwise the pass reads a
/// placeholder and nothing else, forever, and the graph is describing a dependency that does not
/// exist.
#[test]
fn an_optional_read_names_a_resource_something_can_actually_produce() {
    let produced: BTreeSet<FrameResource> =
        FRAME_GRAPH.iter().flat_map(|node| node.writes.iter().copied()).collect();
    for node in FRAME_GRAPH {
        for resource in node.optional_reads {
            assert!(
                produced.contains(resource),
                "{:?} optionally reads {resource:?}, which no pass writes — it would always be \
                 the placeholder",
                node.id
            );
        }
    }
}

/// The graph and the timestamp slots have to agree on what order a frame happens in, because the
/// profiler hands out slots in ENCODING order and reads them back by position. A graph listing
/// passes in a different order would name every measurement after the wrong pass.
#[test]
fn the_graph_lists_every_pass_once_in_encoding_order() {
    let graph: Vec<PassId> = FRAME_GRAPH.iter().map(|node| node.id).collect();
    assert_eq!(
        graph,
        PassId::ALL.to_vec(),
        "the frame graph and the pass list disagree about the order of a frame"
    );
}

/// A pass that neither reads nor writes anything is not part of a frame graph; it is a comment.
#[test]
fn every_pass_touches_at_least_one_resource() {
    for node in FRAME_GRAPH {
        assert!(
            !node.writes.is_empty(),
            "{:?} writes nothing — a pass that produces no resource cannot be depended on, and \
             nothing could tell whether it ran",
            node.id
        );
    }
}

/// The shipped frame, spelled out. Canonical runs SSAO on, bloom off (the tier sets `bloom_mips`
/// to zero) and refraction off (it needs multisampling, and the shipped count is 1x). This pins
/// which eight passes a player's frame is actually made of — a fact that has, until now, only
/// been discoverable by reading `render` top to bottom.
#[test]
fn the_shipped_frame_is_these_eight_passes() {
    let shipped: Vec<PassId> = enabled(true, false, false).iter().map(|node| node.id).collect();
    assert_eq!(
        shipped,
        vec![
            PassId::ShadowNear,
            PassId::ShadowFar,
            PassId::SsaoPrepass,
            PassId::Ssao,
            PassId::SsaoBlur,
            PassId::Scene,
            PassId::Post,
            PassId::Fxaa,
        ],
        "the shipped frame changed shape"
    );
}

/// The two scene paths are alternatives, never both and never neither: a frame draws its world
/// once. `Refraction` and `NoRefraction` are the only pair of conditions in the table that must
/// partition rather than merely differ.
#[test]
fn a_frame_draws_its_world_exactly_once() {
    for (ssao, bloom, refraction) in combinations() {
        let world = enabled(ssao, bloom, refraction)
            .iter()
            .filter(|node| node.writes.contains(&FrameResource::SceneDepth))
            .count();
        assert_eq!(
            world, 1,
            "the world is drawn {world} times (ssao={ssao}, bloom={bloom}, \
             refraction={refraction})"
        );
    }
}

/// The lock the whole table hangs from: what the renderer ENCODES must equal what the graph says
/// it encodes.
///
/// Every other test here checks the table against itself, which would pass just as happily if the
/// table described a different renderer. This one renders real frames and compares the passes that
/// actually opened against the passes the graph predicts — so the description cannot drift from
/// the thing described without something going red.
///
/// It needs a GPU but NOT `TIMESTAMP_QUERY`: which passes a frame encoded is a fact about the
/// frame, and the recorder keeps it whether or not anything is timing.
#[test]
fn the_encoded_passes_match_the_graph() {
    let Ok(ctx) = renderer_wgpu::GpuContext::headless() else {
        eprintln!("skipping graph/encoding agreement test: no headless adapter");
        return;
    };
    let target = renderer_wgpu::OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = renderer_api::view_projection_matrix(&camera, 1.0, 0.1, 20.0);

    let check = |renderer: &mut renderer_wgpu::SceneRenderer, what: &str| {
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        let predicted: Vec<PassId> = renderer.frame_switches().passes().collect();
        let encoded: Vec<PassId> =
            renderer.last_frame_pass_order().iter().map(|(_, id)| id).collect();
        assert_eq!(
            encoded,
            predicted,
            "{what}: the frame graph and the renderer disagree about which passes a frame is \
             made of (switches: {:?})",
            renderer.frame_switches()
        );
        assert!(!encoded.is_empty(), "{what}: a frame that encoded nothing is not a frame");
    };

    // The shipped tier: SSAO on, bloom off, refraction off.
    let mut shipped =
        renderer_wgpu::SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    check(&mut shipped, "canonical");

    // With SSAO switched off, three passes must vanish from BOTH sides at once.
    shipped.set_ssao_enabled(false);
    check(&mut shipped, "ssao off");
    assert!(
        !shipped.frame_switches().ssao,
        "turning SSAO off must be visible in the switches, or the check above proved nothing"
    );

    // And the dev-only rich tier, which is the only configuration that runs the bloom ladder.
    let mut rich = renderer_wgpu::SceneRenderer::for_offscreen_with_quality(
        &ctx,
        &[],
        &[],
        renderer_api::LightingQuality::rich(),
    )
    .expect("rich renderer");
    check(&mut rich, "rich");
}
