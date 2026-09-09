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
//! Armed by `WOT_FRAME_LOG=<path>`; bounded preallocated CPU samples are exported at exit.
//! No per-hitch log writes occur on the frame thread. `WOT_FRAME_GPU=0` disables the optional
//! blocking GPU readback; when enabled its CPU cost is a separate phase. Unarmed is a `None`.

use std::fmt::Write as _;
use std::time::Instant;

mod capture;

/// A frame longer than this is a hitch the player feels (a frame and a half at 60 Hz).
pub const HITCH_MS: f32 = 25.0;
/// Bounded diagnostic capture: over 30 minutes at 60 FPS or one 15-minute battle at 120.
/// Overflow is explicitly reported; no old frame is silently replaced by a newer one.
pub const CAPTURE_FRAMES: usize = 120_000;
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
    /// Blocking diagnostic GPU readback, separate from the game's render work.
    GpuReadback,
}

impl Phase {
    pub const ALL: [Phase; 9] = [
        Phase::FixedTicks,
        Phase::Bookkeeping,
        Phase::Camera,
        Phase::SceneAssembly,
        Phase::Hud,
        Phase::Upload,
        Phase::Render,
        Phase::Wait,
        Phase::GpuReadback,
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
            Phase::GpuReadback => "gpu_readback",
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

/// Inside [`TickPart::Host`]: where the AUTHORITATIVE tick's own time goes (Q11).
///
/// The host was the largest single CPU cost of a hitching frame and the log could only name
/// it — `host 520 ms` and nothing more — while a steady tick costs under half a millisecond.
/// `battle_host` has summed these sections since Q8, but only the `tick_sections` example ever
/// armed them; the frame the player actually gets never carried them. It does now, so a spike
/// says WHICH part of the tick grew. Local play only: the dedicated host runs its own ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum HostSection {
    /// `refresh_live_cover`: the sight cover rebuilt around damaged buildings and landed turrets.
    LiveCover,
    /// Every bot's brain: routes, target choice, aim.
    Bots,
    /// The simulation step: movement, contact, shells, damage, the sim's own spotting refresh.
    Sim,
    /// The crater ledger folded onto the heightmap, plus the outcome check.
    Craters,
    /// The spotting log's observer masks, on emitting ticks.
    SpottingLog,
    /// `Snapshot::from` and the pending events, on emitting ticks.
    Snapshot,
    /// The viewer's cut: `view_for` and its own observer masks.
    View,
    /// The rest of the tick envelope: event copies, the outcome word.
    Other,
}

impl HostSection {
    /// The order `battle_host::TickSections::sections` reports; the caller maps into it.
    pub const ALL: [HostSection; 8] = [
        HostSection::LiveCover,
        HostSection::Bots,
        HostSection::Sim,
        HostSection::Craters,
        HostSection::SpottingLog,
        HostSection::Snapshot,
        HostSection::View,
        HostSection::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HostSection::LiveCover => "live_cover",
            HostSection::Bots => "bots",
            HostSection::Sim => "sim",
            HostSection::Craters => "craters",
            HostSection::SpottingLog => "spotting_log",
            HostSection::Snapshot => "snapshot",
            HostSection::View => "view",
            HostSection::Other => "other",
        }
    }
}

/// One completed render attempt; this CPU clock does not measure physical display scanout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSample {
    /// Seconds since the log started.
    pub at_s: f32,
    /// The whole interval between completed render attempts, including diagnostic readback.
    pub total_ms: f32,
    pub phases_ms: [f32; 9],
    /// The fixed ticks' parts, in [`TickPart::ALL`] order (summed over the frame's ticks).
    pub tick_parts_ms: [f32; 4],
    /// Inside [`TickPart::Host`], in [`HostSection::ALL`] order (summed over the frame's ticks).
    /// All zero when nothing armed the host profile: remote play, or no frame log.
    pub host_sections_ms: [f32; 8],
    /// The observer masks this frame's ticks computed, and how many times a caller was handed
    /// them back instead of walking the lines of sight again (Q8's cache, seen per frame).
    pub host_masks: [u32; 2],
    /// The fixed ticks this frame ran (0 on a frame between ticks, 2+ when catching up).
    pub fixed_ticks: u32,
    pub workload: FrameWorkload,
}

/// Counts of current work, not estimates of memory or proof of the cause of a hitch.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FrameWorkload {
    pub craters: usize,
    pub particles: usize,
    pub vehicle_instances: usize,
    pub scenery_instances: usize,
    pub fx_vertices: usize,
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

    /// The costliest part of the authoritative tick this frame, or `None` when the host profile
    /// was not armed (remote play). This is the name a `host 520 ms` hitch was missing.
    pub fn worst_host_section(&self) -> Option<(HostSection, f32)> {
        HostSection::ALL
            .iter()
            .map(|section| (*section, self.host_sections_ms[*section as usize]))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, ms)| *ms > 0.0)
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
    completed: Instant,
    frames: std::collections::VecDeque<FrameSample>,
    /// Every hitch in the retained capture; bounded by CAPTURE_FRAMES.
    hitches: Vec<FrameSample>,
    total_frames: u64,
    gpu: Vec<GpuSample>,
    /// The frame in flight: phase accumulators.
    phases_ms: [f32; 9],
    tick_parts_ms: [f32; 4],
    host_sections_ms: [f32; 8],
    host_masks: [u32; 2],
    fixed_ticks: u32,
    phase_started: Option<(Phase, Instant)>,
    context: String,
    viewport: (u32, u32),
    workload: FrameWorkload,
    gpu_attempts: u64,
    gpu_enabled: bool,
}

impl FrameLog {
    pub fn new() -> Self {
        let started = Instant::now();
        Self {
            started,
            completed: started,
            frames: std::collections::VecDeque::with_capacity(CAPTURE_FRAMES),
            hitches: Vec::with_capacity(CAPTURE_FRAMES),
            total_frames: 0,
            gpu: Vec::new(),
            phases_ms: [0.0; 9],
            tick_parts_ms: [0.0; 4],
            host_sections_ms: [0.0; 8],
            host_masks: [0; 2],
            fixed_ticks: 0,
            phase_started: None,
            context: String::new(),
            viewport: (0, 0),
            workload: FrameWorkload::default(),
            gpu_attempts: 0,
            gpu_enabled: true,
        }
    }

    /// A line the report opens with (the map, the mode, the vehicle).
    /// The presented size — the fill-bound half of every GPU number.
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
    }

    /// All retained frames, including stalls. Classification is an analysis decision.
    fn steady_frames(&self) -> impl Iterator<Item = &FrameSample> {
        self.frames.iter()
    }

    pub fn set_workload(&mut self, workload: FrameWorkload) {
        self.workload = workload;
    }

    pub fn set_gpu_enabled(&mut self, enabled: bool) {
        self.gpu_enabled = enabled;
    }

    pub fn gpu_enabled(&self) -> bool {
        self.gpu_enabled
    }

    /// Complete the same interval whose CPU phases have just been accumulated.
    pub fn complete_frame(&mut self) -> FrameSample {
        self.end_phase();
        self.complete_frame_at(Instant::now())
    }

    fn complete_frame_at(&mut self, now: Instant) -> FrameSample {
        let ms = now.saturating_duration_since(self.completed).as_secs_f32() * 1000.0;
        self.completed = now;
        self.end_frame(ms)
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

    /// Fold one drained host profile into the frame in flight: the sections in
    /// [`HostSection::ALL`] order, then the tick's observer-mask computations and reuses.
    /// Called once per frame with the sums of every tick that frame ran.
    pub fn add_host_sections(&mut self, sections: [f32; 8], computed: u32, reused: u32) {
        for (sum, ms) in self.host_sections_ms.iter_mut().zip(sections) {
            *sum += ms;
        }
        self.host_masks[0] = self.host_masks[0].saturating_add(computed);
        self.host_masks[1] = self.host_masks[1].saturating_add(reused);
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
            host_sections_ms: self.host_sections_ms,
            host_masks: self.host_masks,
            fixed_ticks: self.fixed_ticks,
            workload: self.workload,
        };
        self.phases_ms = [0.0; 9];
        self.tick_parts_ms = [0.0; 4];
        self.host_sections_ms = [0.0; 8];
        self.host_masks = [0; 2];
        self.fixed_ticks = 0;
        self.total_frames += 1;
        if self.frames.len() < CAPTURE_FRAMES {
            self.frames.push_back(sample);
            if total_ms > HITCH_MS {
                self.hitches.push(sample);
            }
        }
        sample
    }

    /// Whether this frame is one the GPU pass table is read on.
    pub fn gpu_sample_due(&self) -> bool {
        self.gpu_enabled
            && self.frames.len() < CAPTURE_FRAMES
            && (self.total_frames + 1).is_multiple_of(u64::from(GPU_SAMPLE_EVERY))
    }

    pub fn record_gpu(&mut self, frame_ms: f32, passes: Vec<(String, f32)>) {
        let at_s = self.elapsed_s();
        self.gpu.push(GpuSample { at_s, frame_ms, passes });
    }

    pub fn record_gpu_attempt(&mut self, result: Option<(f32, Vec<(String, f32)>)>) {
        self.gpu_attempts += 1;
        if let Some((ms, passes)) = result {
            self.record_gpu(ms, passes);
        }
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

    /// Percentile of retained frame intervals (0.0..=1.0), or 0 with no frames.
    pub fn percentile_ms(&self, p: f32) -> f32 {
        if self.frames.is_empty() {
            return 0.0;
        }
        let mut totals: Vec<f32> = self.frames.iter().map(|f| f.total_ms).collect();
        totals.sort_by(f32::total_cmp);
        let index = ((totals.len() - 1) as f32 * p.clamp(0.0, 1.0)).round() as usize;
        totals[index]
    }

    /// Mean of each phase over the full retained capture, in [`Phase::ALL`] order.
    pub fn mean_phases_ms(&self) -> [f32; 9] {
        let mut sums = [0.0f32; 9];
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
            "frames {} ({:.1} s), retained {} frames: p50 {:.2} ms  p95 {:.2} ms  p99 {:.2} ms  max {:.2} ms",
            self.total_frames,
            self.frames.back().map_or(0.0, |frame| frame.at_s),
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
        let _ = writeln!(
            out,
            "capture dropped frames: {}; GPU enabled: {}, readback attempts: {}, returned samples: {}; first interval includes startup/transition; completion-to-completion is not display scanout",
            self.total_frames - self.frames.len() as u64,
            self.gpu_enabled,
            self.gpu_attempts,
            self.gpu.len()
        );
        let _ = writeln!(out, "mean CPU phases (ms) over ALL retained frames, including stalls:");
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

            // Q11: inside the host, where the authoritative tick's own time went. Zero
            // everywhere means the profile was never armed (remote play) — not a free tick.
            let mut sections = [0.0f32; 8];
            let mut masks = [0u64; 2];
            for frame in self.steady_frames() {
                for (sum, ms) in sections.iter_mut().zip(frame.host_sections_ms) {
                    *sum += ms;
                }
                masks[0] += u64::from(frame.host_masks[0]);
                masks[1] += u64::from(frame.host_masks[1]);
            }
            if sections.iter().any(|ms| *ms > 0.0) {
                let _ = writeln!(out, "host tick sections (mean ms per frame, inside host):");
                for (section, sum) in HostSection::ALL.iter().zip(sections) {
                    let _ = writeln!(out, "  {:<15} {:>7.3}", section.name(), sum / n);
                }
                let _ = writeln!(
                    out,
                    "  observer masks: {} computed, {} reused over the capture",
                    masks[0], masks[1]
                );
            }
        }

        let mut worst: Vec<&FrameSample> = self.hitches.iter().collect();
        worst.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
        let _ = writeln!(out, "the {} longest frames:", worst.len().min(8));
        for frame in worst.iter().take(8) {
            let (phase, ms) = frame.worst_phase();
            // When the fixed ticks own the frame, the useful word is which part of the
            // authoritative tick grew — `host 520 ms` alone named nothing (Q11).
            let host = match frame.worst_host_section() {
                Some((section, section_ms)) => {
                    format!(", host {} {section_ms:.2} ms", section.name())
                }
                None => String::new(),
            };
            let _ = writeln!(
                out,
                "  t {:>7.1} s  {:>7.2} ms  worst phase {} {:.2} ms  (ticks {}, unattributed {:.2}{host})",
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
            let mut owners = [0usize; 9];
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
        self.append_windows(&mut out);
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
    fn phases_never_nest_and_overflow_is_explicit_without_overwriting_the_start() {
        let mut log = FrameLog::new();
        log.begin(Phase::Camera);
        log.begin(Phase::Hud); // closes Camera
        log.end_phase();
        let sample = log.end_frame(10.0);
        assert!(sample.phase_ms(Phase::Camera) >= 0.0 && sample.phase_ms(Phase::Hud) >= 0.0);
        for _ in 0..(CAPTURE_FRAMES + 50) {
            log.end_frame(16.0);
        }
        assert_eq!(log.frames().count(), CAPTURE_FRAMES);
        assert_eq!(log.total_frames(), CAPTURE_FRAMES as u64 + 51);
        assert_eq!(log.frames().next().unwrap().total_ms, 10.0);
        assert!(log.report().contains("capture dropped frames: 51"));
        assert!(!log.gpu_sample_due());
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
    fn a_stall_over_a_second_remains_in_means_and_percentiles() {
        let mut log = FrameLog::new();
        log.begin(Phase::Hud);
        log.add_ms(Phase::Hud, 8_000.0);
        log.end_frame(9_000.0);
        for _ in 0..9 {
            log.add_ms(Phase::Render, 10.0);
            log.end_frame(12.0);
        }
        let means = log.mean_phases_ms();
        assert!((means[Phase::Render as usize] - 9.0).abs() < 1e-3, "{means:?}");
        assert!(means[Phase::Hud as usize] >= 800.0, "{means:?}");
        let report = log.report();
        assert!(report.contains("ALL retained frames, including stalls"), "{report}");
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

    /// Q11: `host 520 ms` named nothing, and a steady tick costs under half a millisecond —
    /// so the frame whose authoritative tick blew up must carry WHICH part of it grew, into
    /// the sample and into the report's list of the longest frames.
    #[test]
    fn a_host_spike_carries_the_name_of_the_section_that_grew() {
        let mut log = FrameLog::new();
        for index in 0..4u32 {
            let spike = index == 3;
            log.note_fixed_ticks(2);
            log.add_ms(Phase::FixedTicks, if spike { 520.0 } else { 1.0 });
            log.add_tick_ms(TickPart::Host, if spike { 519.0 } else { 0.8 });
            let sections = if spike {
                [480.0, 20.0, 15.0, 0.0, 2.0, 1.0, 1.0, 0.0]
            } else {
                [0.1, 0.3, 0.3, 0.0, 0.0, 0.05, 0.05, 0.0]
            };
            log.add_host_sections(sections, u32::from(spike), 1);
            log.end_frame(if spike { 540.0 } else { 16.0 });
        }
        let spike = log.frames().last().copied().expect("a frame");
        assert_eq!(spike.host_sections_ms[HostSection::LiveCover as usize], 480.0);
        let (section, ms) = spike.worst_host_section().expect("the profile was armed");
        assert_eq!(section, HostSection::LiveCover);
        assert!((ms - 480.0).abs() < 1e-3, "{ms}");
        assert_eq!(spike.host_masks, [1, 1], "one emitting tick, one reuse");

        let report = log.report();
        assert!(report.contains("host tick sections"), "{report}");
        assert!(report.contains(", host live_cover 480.00 ms)"), "{report}");
        assert!(report.contains("observer masks: 1 computed, 4 reused"), "{report}");
    }

    /// A remote battle's ticks run in the dedicated host's process. The sections are empty,
    /// and empty must read as "not measured here" — never as a tick that cost nothing.
    #[test]
    fn an_unarmed_host_profile_prints_no_section_block() {
        let mut log = FrameLog::new();
        for _ in 0..4 {
            log.note_fixed_ticks(1);
            log.add_ms(Phase::FixedTicks, 2.0);
            log.end_frame(16.0);
        }
        let report = log.report();
        assert!(report.contains("fixed tick parts"), "{report}");
        assert!(!report.contains("host tick sections"), "{report}");
        assert_eq!(log.frames().last().unwrap().worst_host_section(), None);
    }
}
