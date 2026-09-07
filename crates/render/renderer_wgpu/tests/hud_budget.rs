//! Locks the HUD's budget, in three parts. The upload budget: the buffer holds the full battle
//! HUD (reticle, bars, readouts, damage log, ammo panel and minimap need well over the old
//! 2048-vertex cap), and an oversized upload degrades by truncating to whole triangles instead of
//! silently blanking the frame. The shape: the HUD is one draw in its own pass (interface program
//! F1). And the cost: what that pass takes on the min spec, as a FLOOR that cannot regress and a
//! TARGET the design document set.
//!
//! Its own test binary, on purpose (2026-09-07): inside the `suite` binary the other GPU tests
//! run on their own threads alongside this one, and under the full gate the HUD pass measured a
//! steady 2.99 ms (p95 3.00) against 0.4 ms alone — the GPU was shared, not the pass costlier.
//! Cargo runs test binaries one after another, so here the pass has the GPU to itself, which is
//! the only condition under which a floor on its time means anything. And the device must have
//! DRAINED the previous binary's work: the measurement is the best of several paused windows
//! (see the loop), because straight after the suite the same pass reads ten times its cost.

use renderer_api::{Camera, HudVertex, view_projection_matrix};
use renderer_wgpu::{
    FrameProfiler, GpuContext, GpuContextOptions, OffscreenTarget, PassId, SceneRenderer,
};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping hud budget test: {error}");
            None
        }
    }
}

fn hud_vertices(count: usize) -> Vec<HudVertex> {
    (0..count).map(|_| HudVertex::new([0.0, 0.0], [1.0, 1.0, 1.0, 1.0])).collect()
}

fn render_once(ctx: &GpuContext, target: &OffscreenTarget, renderer: &mut SceneRenderer) {
    let camera =
        Camera { eye: [0.0, 0.0, 3.0], target: [0.0, 0.0, 0.0], vertical_fov_degrees: 55.0 };
    renderer
        .render(
            ctx,
            target.render_target(),
            view_projection_matrix(&camera, 1.0, 0.1, 20.0),
            camera.eye,
        )
        .expect("render");
}

#[test]
fn the_hud_buffer_holds_a_full_battle_hud() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");

    // 12000 vertices: a busy frame with the 36x36-cell minimap, drop-shadowed text, damage
    // log and ammo panel — the 8192-vertex budget cut the minimap's blips off mid-frame.
    let busy_frame = hud_vertices(12000);
    renderer.set_hud(&ctx, &busy_frame);
    assert_eq!(renderer.hud_vertex_count(), 12000, "a busy battle HUD must upload untruncated");
}

#[test]
fn an_oversized_hud_upload_truncates_to_whole_triangles_instead_of_blanking() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");

    let oversized = hud_vertices(20000);
    renderer.set_hud(&ctx, &oversized);

    let kept = renderer.hud_vertex_count();
    assert!(kept > 0, "an oversized upload must keep the leading vertices, not blank the HUD");
    assert!((kept as usize) < oversized.len(), "an oversized upload must be truncated");
    assert_eq!(kept % 3, 0, "truncation must land on a whole triangle");
}

/// The HUD is its own pass (interface program F1): one draw, in `hud_pass`, carrying every
/// uploaded triangle — and the FXAA pass it used to ride inside stays the single fullscreen
/// triangle it always was. Counts, not timings, so this holds on any adapter.
#[test]
fn a_full_hud_is_one_draw_in_its_own_pass() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    renderer.set_hud(&ctx, &hud_vertices(12000));
    render_once(&ctx, &target, &mut renderer);

    let counts = renderer.last_frame_counts();
    let hud = counts.pass(PassId::Hud);
    assert_eq!(hud.draws, 1, "the whole HUD is one draw call");
    assert_eq!(hud.triangles, 4000, "twelve thousand vertices are four thousand triangles");
    assert_eq!(hud.instances, 1);
    let fxaa = counts.pass(PassId::Fxaa);
    assert_eq!(fxaa.draws, 1, "the FXAA pass no longer carries the HUD");
    assert_eq!(fxaa.triangles, 1, "the FXAA pass is its one fullscreen triangle and nothing else");
}

/// GDD §9 and §15.8 give the HUD 0.5 ms on the min spec. The interface program (F1) makes that a
/// FLOOR/TARGET pair like the look goldens: the FLOOR is what the MX330 measured the day the pass
/// first had a name, asserted so a costlier HUD cannot land unnoticed; the TARGET is the design's
/// number, printed as debt until the HUD wave earns it.
const HUD_PASS_TARGET_MS: f32 = 0.5;
/// The first MX330 record (2026-09-05, 1920x1080, the synthetic worst case below): p50 0.179 ms,
/// p95 0.201 ms. The floor is that reading with half again of slack, because the gate runs on
/// the same laptop that is compiling, timing and thermally throttling — a floor that flickers
/// with the room's temperature would be burned down within a week, and a HUD that doubles in
/// cost still fails it. Raised only on purpose, in the PR that made the HUD costlier, with the
/// new reading in its message.
///
/// Second record (2026-09-06, after F3's signed-distance glyphs: the fragment shader fetches the
/// field and its derivative for every fragment so the derivatives stay in uniform control flow):
/// p50 0.289 ms, p95 0.318 ms, cold. Floor 0.27 → 0.43 (the reading × 1.5); the TARGET holds.
/// The per-fragment fetch is H26's first shave.
const HUD_PASS_FLOOR_MS: f32 = 0.43;
/// The adapter name of the min spec; the number above is meaningful only there. Every other
/// machine prints its reading and asserts nothing.
const MIN_SPEC_ADAPTER: &str = "MX330";
/// The full buffer: 16 384 vertices as 2 730 quads on a 65 x 42 grid, each covering three
/// quarters of its cell, half of them sampling the atlas. About three quarters of the screen
/// filled once through the HUD's alpha blend — more than the shipped HUD covers, so the reading
/// is an upper bound on today's interface, and the shape the SDF plates of F2 will be measured
/// against on the same grid.
const WORST_CASE_VERTICES: usize = 16_384;

fn synthetic_worst_case_hud() -> Vec<HudVertex> {
    const COLUMNS: usize = 65;
    const ROWS: usize = 42;
    let quads = WORST_CASE_VERTICES / 6;
    let cell_w = 2.0 / COLUMNS as f32;
    let cell_h = 2.0 / ROWS as f32;
    let mut vertices = Vec::with_capacity(quads * 6);
    for quad in 0..quads {
        let column = (quad % COLUMNS) as f32;
        let row = (quad / COLUMNS % ROWS) as f32;
        let x0 = -1.0 + column * cell_w + cell_w * 0.067;
        let y0 = -1.0 + row * cell_h + cell_h * 0.067;
        let (x1, y1) = (x0 + cell_w * 0.866, y0 + cell_h * 0.866);
        let color = [0.2, 0.25, 0.3, 0.85];
        let corner = |x: f32, y: f32, u: f32, v: f32| {
            if quad % 2 == 0 {
                HudVertex::textured([x, y], [u, v], color)
            } else {
                HudVertex::new([x, y], color)
            }
        };
        vertices.extend([
            corner(x0, y0, 0.0, 1.0),
            corner(x1, y0, 1.0, 1.0),
            corner(x1, y1, 1.0, 0.0),
            corner(x0, y0, 0.0, 1.0),
            corner(x1, y1, 1.0, 0.0),
            corner(x0, y1, 0.0, 0.0),
        ]);
    }
    vertices.truncate(WORST_CASE_VERTICES);
    vertices
}

#[test]
fn the_full_hud_pass_stays_under_its_floor_on_the_min_spec() {
    let Ok(ctx) = GpuContext::headless_with_options(GpuContextOptions { pass_timing: true }) else {
        eprintln!("skipping HUD pass budget: no headless adapter");
        return;
    };
    let profiler = FrameProfiler::new(&ctx.device, &ctx.queue, true);
    if profiler.active().is_none() {
        eprintln!(
            "skipping HUD pass budget: {}",
            profiler.unavailable_reason().unwrap_or("timing unavailable")
        );
        return;
    }

    let target = OffscreenTarget::new(&ctx, 1920, 1080).expect("a 1080p target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    renderer.set_pass_profiler(profiler);
    let hud = synthetic_worst_case_hud();
    // Whole quads only: the largest multiple of six under the buffer's capacity.
    let expected = WORST_CASE_VERTICES / 6 * 6;
    assert_eq!(hud.len(), expected, "the worst case fills the buffer with whole quads");
    renderer.set_hud(&ctx, &hud);
    assert_eq!(renderer.hud_vertex_count() as usize, expected, "nothing truncated");

    // The pass's OWN cost is what the floor means, and the device does not give it up right
    // after another test binary has used it: measured 2026-09-07, the same pass read 3.6 ms
    // straight after the renderer suite (GPU at 74 °C, clocks up, the driver still draining
    // that binary's work) and 0.30 ms twenty seconds later. So: several windows with a pause
    // between them, and the window with the lowest median is the measurement — the one taken
    // when the device was free. Every window is printed, so a real regression (all windows
    // high) is as visible as the drain (the first window high, the rest low).
    const WARMUP: usize = 10;
    const SAMPLES: usize = 40;
    const WINDOWS: usize = 5;
    const PAUSE_BETWEEN_WINDOWS: std::time::Duration = std::time::Duration::from_millis(1500);
    let mut best: Option<(f32, f32)> = None;
    let mut windows = Vec::with_capacity(WINDOWS);
    for window in 0..WINDOWS {
        if window > 0 {
            std::thread::sleep(PAUSE_BETWEEN_WINDOWS);
        }
        let mut readings = Vec::with_capacity(SAMPLES);
        for frame in 0..(WARMUP + SAMPLES) {
            render_once(&ctx, &target, &mut renderer);
            // The readback is the fence: without it the timings would be read before the GPU
            // wrote them, and the first frame's numbers would be the previous frame's.
            target.read_rgba8(&ctx).expect("readback");
            let timings = renderer.read_pass_timings(&ctx).expect("an armed frame reports timings");
            let hud_ms = timings.pass_ms(PassId::Hud).expect("the HUD pass is encoded every frame");
            if frame >= WARMUP {
                readings.push(hud_ms);
            }
        }
        readings.sort_by(f32::total_cmp);
        let p50 = readings[readings.len() / 2];
        let p95 = readings[(readings.len() * 95 / 100).min(readings.len() - 1)];
        windows.push(p50);
        if best.is_none_or(|(b, _)| p50 < b) {
            best = Some((p50, p95));
        }
    }
    let (p50, p95) = best.expect("at least one window");
    println!(
        "HUD PASS windows (p50 ms): {}",
        windows.iter().map(|w| format!("{w:.3}")).collect::<Vec<_>>().join("  ")
    );

    let name = ctx.adapter.get_info().name;
    println!(
        "HUD PASS on {name}: p50 {p50:.3} ms  p95 {p95:.3} ms  ({WORST_CASE_VERTICES} vertices, \
         1920x1080; floor {HUD_PASS_FLOOR_MS} ms, target {HUD_PASS_TARGET_MS} ms)"
    );
    if p50 > HUD_PASS_TARGET_MS {
        println!("HUD DEBT: short of the target by {:.3} ms", p50 - HUD_PASS_TARGET_MS);
    }
    if name.contains(MIN_SPEC_ADAPTER) {
        assert!(
            p50 <= HUD_PASS_FLOOR_MS,
            "the HUD pass regressed on the min spec: p50 {p50:.3} ms against a floor of \
             {HUD_PASS_FLOOR_MS} ms — a costlier interface needs its number raised on purpose, \
             in the PR that made it costlier"
        );
    }
}
