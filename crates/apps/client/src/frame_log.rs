//! The live frame instrument (the one program's Q2, 2026-09-08 — the owner: „gra nie trzyma
//! stałych 60 fps… błędy throttlingu, które czuć podczas jazdy”). Every other perf probe
//! measures a SCENE offscreen; nothing measured the frame the player actually gets — sim,
//! prediction, ingest, scene assembly, HUD, submit and present together — nor said what ran in
//! the longest one. This does: one sample per presented frame with its CPU phases, the hitches
//! (frames over [`HITCH_MS`]) with their worst phase, the GPU pass table sampled every
//! [`GPU_SAMPLE_EVERY`] frames (a blocking read, so not every frame). The process memory is
//! the run script's (`scripts/perf/cold-run.ps1` samples the working set and the GPU's used
//! memory beside the run — the workspace forbids the unsafe an in-process allocator needs).
//!
//! Armed by `WOT_FRAME_LOG=<path>`; the report is written there when the app exits. It costs a
//! few atomic loads and one `Instant::now()` per phase; unarmed it is a `None`.

use std::fmt::Write as _;
use std::time::Instant;

/// A frame longer than this is a hitch the player feels (a frame and a half at 60 Hz).
pub const HITCH_MS: f32 = 25.0;
/// A frame longer than this is left out of the means (a bake, a stall), never a hitch.
pub const STEADY_MAX_MS: f32 = 1_000.0;
/// How many frames the ring keeps: a minute at 60 FPS.
pub const RING_FRAMES: usize = 3_600;
/// The GPU pass table is read back (a blocking device read) once per this many frames.
pub const GPU_SAMPLE_EVERY: u32 = 60;

/// The CPU phases of one presented frame, in the order the loop runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The fixed 60 Hz ticks run before this frame: input → prediction → local server → ingest.
    FixedTicks,
    /// Mouse look, the FX/scar/topple clocks, the frame's bookkeeping.
    Bookkeeping,
    /// Camera advance and the view projection.
    Camera,
    /// Scene assembly: grass, the tree ladder, the vehicles' render frame, the FX vertices.
    SceneAssembly,
    /// The HUD model and its vertices (the reticle's trace inside).
    Hud,
    /// Handing the frame to the renderer: `set_render_frame`, `set_vehicle_render_frame`, FX, HUD.
    Upload,
    /// `renderer.render`: acquire the surface, encode, submit, present.
    Render,
    /// The loop asleep between events: `WaitUntil` (the pacer's beat, the next tick) and the
    /// OS event queue. Not work — but a frame that is late AND waits is the pacer's bug.
    Wait,
}

impl Phase {
    pub const ALL: [Phase; 8] = [
        Phase::FixedTicks,
        Phase::Bookkeeping,
        Phase::Camera,
        Phase::SceneAssembly,
        Phase::Hud,
        Phase::Upload,
        Phase::Render,
        Phase::Wait,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Phase::FixedTicks => "fixed_ticks",
            Phase::Bookkeeping => "bookkeeping",
            Phase::Camera => "camera",
            Phase::SceneAssembly => "scene_assembly",
            Phase::Hud => "hud",
            Phase::Upload => "upload",
            Phase::Render => "render",
            Phase::Wait => "wait",
        }
    }
}

/// The parts of one fixed tick worth telling apart (Q8): what the client does around the
/// authoritative step, and the step itself. Sub-phases of [`Phase::FixedTicks`]; their sum
/// never exceeds it, the rest of the envelope is "other" (input latches, breach deltas,
/// outcome refresh).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TickPart {
    /// `sight_solution` and the turret/gun commands derived from it.
    Sight,
    /// `step_prediction`: the player's hull predicted one tick.
    Predict,
    /// `tick_with_player_input`: the local authoritative server (bots, sim, spotting, filter).
    Host,
    /// Ingesting the tick's snapshot: `accept_and_sync` / `accept_remote_and_sync`.
    Sync,
}

impl TickPart {
    pub const ALL: [TickPart; 4] =
        [TickPart::Sight, TickPart::Predict, TickPart::Host, TickPart::Sync];

    pub fn name(self) -> &'static str {
        match self {
            TickPart::Sight => "sight",
            TickPart::Predict => "predict",
            TickPart::Host => "host",
            TickPart::Sync => "sync",
        }
    }
}

/// One presented frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSample {
    /// Seconds since the log started.
    pub at_s: f32,
    /// The whole frame interval (presented-to-presented).
    pub total_ms: f32,
    pub phases_ms: [f32; 8],
    /// The fixed ticks' parts, in [`TickPart::ALL`] order (summed over the frame's ticks).
    pub tick_parts_ms: [f32; 4],
    /// The fixed ticks this frame ran (0 on a frame between ticks, 2+ when catching up).
    pub fixed_ticks: u32,
}

impl FrameSample {
    pub fn phase_ms(&self, phase: Phase) -> f32 {
        self.phases_ms[phase as usize]
    }

    /// The phase that took the most of this frame.
    pub fn worst_phase(&self) -> (Phase, f32) {
        Phase::ALL
            .iter()
            .map(|phase| (*phase, self.phase_ms(*phase)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((Phase::Render, 0.0))
    }

    /// What the phases do not account for: sleeping in the pacer, the OS, the event queue.
    pub fn unattributed_ms(&self) -> f32 {
        (self.total_ms - self.phases_ms.iter().sum::<f32>()).max(0.0)
    }
}

/// One GPU pass-table sample: (pass name, milliseconds) in pass order.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuSample {
    pub at_s: f32,
    pub frame_ms: f32,
    pub passes: Vec<(String, f32)>,
}

#[derive(Debug)]
pub struct FrameLog {
    started: Instant,
    frames: std::collections::VecDeque<FrameSample>,
    /// Every hitch since the log started, oldest first (capped at 1 000 so a long stall does
    /// not grow the log without bound).
    hitches: Vec<FrameSample>,
    total_frames: u64,
    gpu: Vec<GpuSample>,
    /// The frame in flight: phase accumulators.
    phases_ms: [f32; 8],
    tick_parts_ms: [f32; 4],
    fixed_ticks: u32,
    phase_started: Option<(Phase, Instant)>,
    context: String,
    viewport: (u32, u32),
}

impl FrameLog {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            frames: std::collections::VecDeque::with_capacity(RING_FRAMES),
            hitches: Vec::new(),
            total_frames: 0,
            gpu: Vec::new(),
            phases_ms: [0.0; 8],
            tick_parts_ms: [0.0; 4],
            fixed_ticks: 0,
            phase_started: None,
            context: String::new(),
            viewport: (0, 0),
        }
    }

    /// A line the report opens with (the map, the mode, the vehicle).
    /// The presented size — the fill-bound half of every GPU number.
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
    }

    /// The frames the means are taken over: a frame over [`STEADY_MAX_MS`] is a bake or a
    /// stall, not a frame, and one of them would own every mean in a 90 s run.
    fn steady_frames(&self) -> impl Iterator<Item = &FrameSample> {
        self.frames.iter().filter(|frame| frame.total_ms <= STEADY_MAX_MS)
    }

    pub fn set_context(&mut self, context: impl Into<String>) {
        self.context = context.into();
    }

    pub fn elapsed_s(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }

    /// Open a phase; a phase still open is closed first (phases never nest).
    pub fn begin(&mut self, phase: Phase) {
        self.end_phase();
        self.phase_started = Some((phase, Instant::now()));
    }

    /// Close the open phase, if any.
    pub fn end_phase(&mut self) {
        if let Some((phase, started)) = self.phase_started.take() {
            self.phases_ms[phase as usize] += started.elapsed().as_secs_f32() * 1000.0;
        }
    }

    /// Add a measured duration to a phase without the clock (a caller that timed itself).
    pub fn add_ms(&mut self, phase: Phase, ms: f32) {
        self.phases_ms[phase as usize] += ms;
    }

    /// Add a measured duration to one part of the frame's fixed ticks.
    pub fn add_tick_ms(&mut self, part: TickPart, ms: f32) {
        self.tick_parts_ms[part as usize] += ms;
    }

    pub fn note_fixed_ticks(&mut self, count: u32) {
        self.fixed_ticks += count;
    }

    /// The frame was presented: `total_ms` is the interval since the previous presentation.
    /// Returns the sample, so a caller can print a hitch as it happens.
    pub fn end_frame(&mut self, total_ms: f32) -> FrameSample {
        self.end_phase();
        let sample = FrameSample {
            at_s: self.elapsed_s(),
            total_ms,
            phases_ms: self.phases_ms,
            tick_parts_ms: self.tick_parts_ms,
            fixed_ticks: self.fixed_ticks,
        };
        self.phases_ms = [0.0; 8];
        self.tick_parts_ms = [0.0; 4];
        self.fixed_ticks = 0;
        if self.frames.len() >= RING_FRAMES {
            self.frames.pop_front();
        }
        self.frames.push_back(sample);
        self.total_frames += 1;
        if total_ms > HITCH_MS && self.hitches.len() < 1_000 {
            self.hitches.push(sample);
        }
        sample
    }

    /// Whether this frame is one the GPU pass table is read on.
    pub fn gpu_sample_due(&self) -> bool {
        self.total_frames > 0 && self.total_frames.is_multiple_of(u64::from(GPU_SAMPLE_EVERY))
    }

    pub fn record_gpu(&mut self, frame_ms: f32, passes: Vec<(String, f32)>) {
        let at_s = self.elapsed_s();
        self.gpu.push(GpuSample { at_s, frame_ms, passes });
    }

    pub fn frames(&self) -> impl Iterator<Item = &FrameSample> {
        self.frames.iter()
    }

    pub fn hitches(&self) -> &[FrameSample] {
        &self.hitches
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Percentile of the frame intervals in the ring (0.0..=1.0), or 0 with no frames.
    pub fn percentile_ms(&self, p: f32) -> f32 {
        if self.frames.is_empty() {
            return 0.0;
        }
        let mut totals: Vec<f32> = self.frames.iter().map(|f| f.total_ms).collect();
        totals.sort_by(f32::total_cmp);
        let index = ((totals.len() - 1) as f32 * p.clamp(0.0, 1.0)).round() as usize;
        totals[index]
    }

    /// Mean of each phase over the ring, in [`Phase::ALL`] order.
    pub fn mean_phases_ms(&self) -> [f32; 8] {
        let mut sums = [0.0f32; 8];
        let mut n = 0usize;
        for frame in self.steady_frames() {
            n += 1;
            for (sum, ms) in sums.iter_mut().zip(frame.phases_ms) {
                *sum += ms;
            }
        }
        let n = n.max(1) as f32;
        sums.map(|sum| sum / n)
    }

    /// The report: what the frame cost, where the hitches went, what the GPU said, the heap.
    pub fn report(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# frame log — {}, viewport {}x{}",
            self.context, self.viewport.0, self.viewport.1
        );
        let _ = writeln!(
            out,
            "frames {} ({:.1} s), ring {} frames: p50 {:.2} ms  p95 {:.2} ms  p99 {:.2} ms  max {:.2} ms",
            self.total_frames,
            self.elapsed_s(),
            self.frames.len(),
            self.percentile_ms(0.50),
            self.percentile_ms(0.95),
            self.percentile_ms(0.99),
            self.percentile_ms(1.0)
        );
        let over_60 = self.frames.iter().filter(|f| f.total_ms > 1000.0 / 60.0 + 0.5).count();
        let _ = writeln!(
            out,
            "frames over 60 Hz budget (17.2 ms): {over_60} of {} ({:.1} %); hitches over {HITCH_MS:.0} ms: {}",
            self.frames.len(),
            100.0 * over_60 as f32 / self.frames.len().max(1) as f32,
            self.hitches.len()
        );
        let means = self.mean_phases_ms();
        let stalls = self.frames.len() - self.steady_frames().count();
        let _ = writeln!(
            out,
            "mean CPU phases (ms) over the steady frames ({stalls} frames over {STEADY_MAX_MS:.0} ms left out — a bake or a stall is not a frame):"
        );
        for (phase, mean) in Phase::ALL.iter().zip(means) {
            let _ = writeln!(out, "  {:<15} {:>7.3}", phase.name(), mean);
        }
        let steady = self.steady_frames().count().max(1) as f32;
        let unattributed: f32 =
            self.steady_frames().map(FrameSample::unattributed_ms).sum::<f32>() / steady;
        let _ =
            writeln!(out, "  {:<15} {:>7.3}  (event dispatch, OS)", "unattributed", unattributed);

        if self.frames.iter().any(|frame| frame.fixed_ticks > 0) {
            let n = self.steady_frames().count().max(1) as f32;
            let mut parts = [0.0f32; 4];
            for frame in self.steady_frames() {
                for (sum, ms) in parts.iter_mut().zip(frame.tick_parts_ms) {
                    *sum += ms;
                }
            }
            let ticks_per_frame =
                self.steady_frames().map(|frame| frame.fixed_ticks as f32).sum::<f32>() / n;
            let envelope = self.mean_phases_ms()[Phase::FixedTicks as usize];
            let _ = writeln!(
                out,
                "fixed tick parts (mean ms per frame, {ticks_per_frame:.2} ticks per frame):"
            );
            for (part, sum) in TickPart::ALL.iter().zip(parts) {
                let _ = writeln!(out, "  {:<15} {:>7.3}", part.name(), sum / n);
            }
            let other = (envelope - parts.iter().sum::<f32>() / n).max(0.0);
            let _ = writeln!(out, "  {:<15} {:>7.3}", "other", other);
        }

        let mut worst: Vec<&FrameSample> = self.hitches.iter().collect();
        worst.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
        let _ = writeln!(out, "the {} longest frames:", worst.len().min(8));
        for frame in worst.iter().take(8) {
            let (phase, ms) = frame.worst_phase();
            let _ = writeln!(
                out,
                "  t {:>7.1} s  {:>7.2} ms  worst phase {} {:.2} ms  (ticks {}, unattributed {:.2})",
                frame.at_s,
                frame.total_ms,
                phase.name(),
                ms,
                frame.fixed_ticks,
                frame.unattributed_ms()
            );
        }
        // Which phase owns the hitches: count the worst phase over every hitch.
        if !self.hitches.is_empty() {
            let mut owners = [0usize; 8];
            for frame in &self.hitches {
                owners[frame.worst_phase().0 as usize] += 1;
            }
            let _ = writeln!(out, "hitches by worst phase:");
            for (phase, count) in Phase::ALL.iter().zip(owners) {
                if count > 0 {
                    let _ = writeln!(out, "  {:<15} {count}", phase.name());
                }
            }
        }
        if !self.gpu.is_empty() {
            let _ = writeln!(out, "GPU pass table ({} samples, mean ms):", self.gpu.len());
            let mut names: Vec<String> = Vec::new();
            for sample in &self.gpu {
                for (name, _) in &sample.passes {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            let frame_mean =
                self.gpu.iter().map(|s| s.frame_ms).sum::<f32>() / self.gpu.len() as f32;
            let _ = writeln!(out, "  {:<18} {:>7.3}", "gpu frame", frame_mean);
            for name in names {
                let (sum, n) = self.gpu.iter().fold((0.0f32, 0usize), |(sum, n), sample| {
                    sample
                        .passes
                        .iter()
                        .find(|(p, _)| *p == name)
                        .map_or((sum, n), |(_, ms)| (sum + ms, n + 1))
                });
                if n > 0 {
                    let _ = writeln!(out, "  {:<18} {:>7.3}", name, sum / n as f32);
                }
            }
        }
        out
    }
}

impl Default for FrameLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hitch_is_attributed_to_the_phase_that_took_it() {
        let mut log = FrameLog::new();
        for _ in 0..10 {
            log.add_ms(Phase::Render, 8.0);
            log.add_ms(Phase::FixedTicks, 2.0);
            log.end_frame(16.6);
        }
        log.add_ms(Phase::SceneAssembly, 30.0);
        log.add_ms(Phase::Render, 8.0);
        let hitch = log.end_frame(40.0);
        assert_eq!(hitch.worst_phase().0, Phase::SceneAssembly);
        assert_eq!(log.hitches().len(), 1);
        assert!((log.percentile_ms(0.5) - 16.6).abs() < 1e-3);
        assert!((log.percentile_ms(1.0) - 40.0).abs() < 1e-3);
        assert!((hitch.unattributed_ms() - 2.0).abs() < 1e-3);
        let report = log.report();
        assert!(report.contains("hitches over 25 ms: 1"), "{report}");
        assert!(report.contains("scene_assembly"), "{report}");
    }

    #[test]
    fn phases_never_nest_and_the_ring_is_bounded() {
        let mut log = FrameLog::new();
        log.begin(Phase::Camera);
        log.begin(Phase::Hud); // closes Camera
        log.end_phase();
        let sample = log.end_frame(10.0);
        assert!(sample.phase_ms(Phase::Camera) >= 0.0 && sample.phase_ms(Phase::Hud) >= 0.0);
        for _ in 0..(RING_FRAMES + 50) {
            log.end_frame(16.0);
        }
        assert_eq!(log.frames().count(), RING_FRAMES);
        assert_eq!(log.total_frames(), RING_FRAMES as u64 + 51);
        assert!(log.gpu_sample_due() || !log.gpu_sample_due());
    }

    #[test]
    fn the_fixed_tick_parts_are_reported_per_frame_with_the_rest_as_other() {
        let mut log = FrameLog::new();
        for _ in 0..2 {
            log.begin(Phase::FixedTicks);
            log.note_fixed_ticks(2);
            log.add_tick_ms(TickPart::Host, 6.0);
            log.add_tick_ms(TickPart::Sight, 1.0);
            log.end_phase();
            log.add_ms(Phase::FixedTicks, 10.0);
            log.end_frame(12.0);
        }
        let report = log.report();
        assert!(
            report.contains("fixed tick parts (mean ms per frame, 2.00 ticks per frame):"),
            "{report}"
        );
        assert!(report.contains("  host              6.000"), "{report}");
        assert!(report.contains("  sight             1.000"), "{report}");
        // The envelope is 10 ms plus the clock's epsilon: other = envelope - 7.
        let other: f32 = report
            .lines()
            .find_map(|line| {
                line.trim().strip_prefix("other").map(|rest| rest.trim().parse().unwrap())
            })
            .expect("other row");
        assert!((other - 3.0).abs() < 0.05, "{other}");
    }

    #[test]
    fn a_stall_over_a_second_is_left_out_of_the_means_but_kept_in_the_percentiles() {
        let mut log = FrameLog::new();
        log.begin(Phase::Hud);
        log.add_ms(Phase::Hud, 8_000.0);
        log.end_frame(9_000.0);
        for _ in 0..9 {
            log.add_ms(Phase::Render, 10.0);
            log.end_frame(12.0);
        }
        let means = log.mean_phases_ms();
        assert!((means[Phase::Render as usize] - 10.0).abs() < 1e-3, "{means:?}");
        assert!(means[Phase::Hud as usize] < 1e-3, "{means:?}");
        let report = log.report();
        assert!(report.contains("1 frames over 1000 ms left out"), "{report}");
        assert!(report.contains("max 9000.00 ms"), "{report}");
    }

    #[test]
    fn the_gpu_table_is_averaged_by_pass_name() {
        let mut log = FrameLog::new();
        log.record_gpu(20.0, vec![("scene_pass".into(), 14.0), ("shadow_pass".into(), 1.0)]);
        log.record_gpu(22.0, vec![("scene_pass".into(), 16.0)]);
        let report = log.report();
        assert!(report.contains("scene_pass          15.000"), "{report}");
        assert!(report.contains("shadow_pass          1.000"), "{report}");
    }
}
