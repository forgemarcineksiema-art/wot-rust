use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientLoopPhase {
    WinitEvent,
    InputSystem,
    FixedTickAccumulator,
    RequestRedraw,
    RenderOnRedraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientLoopEvent {
    Input,
    AboutToWait { elapsed: Duration },
    RedrawRequested,
    Resized { width: u32, height: u32 },
    CloseRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientLoopAction {
    CaptureInput,
    RunFixedTicks(u32),
    RequestRedraw,
    RenderFrame,
    Resize { width: u32, height: u32 },
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedTickAccumulator {
    tick_duration: Duration,
    accumulated: Duration,
}

impl FixedTickAccumulator {
    pub fn from_hz(hz: u32) -> Self {
        assert!(hz > 0);
        Self {
            tick_duration: Duration::from_secs_f64(1.0 / hz as f64),
            accumulated: Duration::ZERO,
        }
    }

    /// Maximum fixed ticks run for a single frame. After a long stall (debugger,
    /// OS suspend) we drop the backlog instead of catching up hundreds of ticks at
    /// once, which would spiral the loop (each catch-up frame falling further behind).
    pub const MAX_CATCHUP_TICKS: u32 = 8;

    pub fn accumulate(&mut self, elapsed: Duration) -> u32 {
        self.accumulated += elapsed;
        let mut ticks = 0;
        while self.accumulated >= self.tick_duration {
            self.accumulated -= self.tick_duration;
            ticks += 1;
            if ticks >= Self::MAX_CATCHUP_TICKS {
                // Drop the remaining backlog so a long stall cannot spiral the loop.
                self.accumulated = Duration::ZERO;
                break;
            }
        }
        ticks
    }

    pub fn tick_duration(&self) -> Duration {
        self.tick_duration
    }

    pub fn remainder(&self) -> Duration {
        self.accumulated
    }
}

/// Frame pacing (Płynność 2.0 / F1): the render loop's own metronome. Before this, every
/// `about_to_wait` unconditionally requested a redraw under `ControlFlow::Poll` — with the
/// non-blocking Mailbox present mode the client rendered as fast as the GPU allowed, at a
/// random phase against the display's refresh: a full core burned for textbook micro-judder.
/// The pacer admits ONE redraw per display interval; the loop sleeps (`WaitUntil`) between
/// beats instead of spinning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePacer {
    frame_interval: Duration,
    accumulated: Duration,
}

impl FramePacer {
    pub fn from_hz(hz: f64) -> Self {
        let hz = if hz.is_finite() && hz > 1.0 { hz } else { 60.0 };
        Self { frame_interval: Duration::from_secs_f64(1.0 / hz), accumulated: Duration::ZERO }
    }

    /// Feed elapsed wall time; true when a presentation beat is due. A stall never queues a
    /// burst: the debt is capped at one interval, so the loop presents once and moves on.
    pub fn frame_due(&mut self, elapsed: Duration) -> bool {
        self.accumulated = (self.accumulated + elapsed).min(self.frame_interval * 2);
        if self.accumulated >= self.frame_interval {
            self.accumulated -= self.frame_interval;
            true
        } else {
            false
        }
    }

    /// How long the loop may sleep before the next beat is due.
    pub fn until_due(&self) -> Duration {
        self.frame_interval.saturating_sub(self.accumulated)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinitLoopDriver {
    accumulator: FixedTickAccumulator,
    pacer: FramePacer,
}

impl WinitLoopDriver {
    pub fn new(tick_hz: u32) -> Self {
        Self {
            accumulator: FixedTickAccumulator::from_hz(tick_hz),
            pacer: FramePacer::from_hz(60.0),
        }
    }

    /// Match the pacer to the actual display once the window knows its monitor. Refresh rates
    /// above 120 Hz are capped: the sim ticks at 60 and the presentation interpolates — beyond
    /// ~120 presented frames/s there is nothing new to show for the power burned.
    pub fn set_present_hz(&mut self, hz: f64) {
        self.pacer = FramePacer::from_hz(hz.min(120.0));
    }

    /// How long `about_to_wait` may sleep (`ControlFlow::WaitUntil`) before either the next
    /// fixed tick or the next presentation beat needs the loop again.
    pub fn suggested_wait(&self) -> Duration {
        let until_tick =
            self.accumulator.tick_duration().saturating_sub(self.accumulator.remainder());
        until_tick.min(self.pacer.until_due())
    }

    pub const fn event_driven_phases() -> [ClientLoopPhase; 5] {
        [
            ClientLoopPhase::WinitEvent,
            ClientLoopPhase::InputSystem,
            ClientLoopPhase::FixedTickAccumulator,
            ClientLoopPhase::RequestRedraw,
            ClientLoopPhase::RenderOnRedraw,
        ]
    }

    pub const fn uses_manual_event_polling() -> bool {
        false
    }

    /// Sub-tick interpolation factor in `[0, 1]`: how far the leftover accumulator has
    /// advanced into the next fixed tick. Rendering blends the previous tick's pose toward
    /// the current one by this fraction so a 60 Hz sim presents smoothly under vsync.
    pub fn render_alpha(&self) -> f32 {
        let tick = self.accumulator.tick_duration().as_secs_f32();
        if tick <= 0.0 {
            return 0.0;
        }
        (self.accumulator.remainder().as_secs_f32() / tick).clamp(0.0, 1.0)
    }

    pub fn handle_event(&mut self, event: ClientLoopEvent) -> Vec<ClientLoopAction> {
        match event {
            ClientLoopEvent::Input => vec![ClientLoopAction::CaptureInput],
            ClientLoopEvent::AboutToWait { elapsed } => {
                let ticks = self.accumulator.accumulate(elapsed);
                let mut actions = Vec::new();
                if ticks > 0 {
                    actions.push(ClientLoopAction::RunFixedTicks(ticks));
                }
                // Paced, not unconditional: one redraw per display beat (see [`FramePacer`]).
                if self.pacer.frame_due(elapsed) {
                    actions.push(ClientLoopAction::RequestRedraw);
                }
                actions
            }
            ClientLoopEvent::RedrawRequested => vec![ClientLoopAction::RenderFrame],
            ClientLoopEvent::Resized { width, height } => {
                vec![ClientLoopAction::Resize { width, height }]
            }
            ClientLoopEvent::CloseRequested => vec![ClientLoopAction::Exit],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redraws(actions: &[ClientLoopAction]) -> usize {
        actions.iter().filter(|a| **a == ClientLoopAction::RequestRedraw).count()
    }

    /// F1's contract: the loop presents ON THE BEAT, not as fast as it spins. Four quick
    /// wake-ups inside one display interval yield exactly one redraw between them.
    #[test]
    fn the_pacer_admits_one_redraw_per_display_interval() {
        let mut driver = WinitLoopDriver::new(60);
        driver.set_present_hz(60.0);
        let mut total = 0;
        for _ in 0..4 {
            let actions = driver
                .handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(4) });
            total += redraws(&actions);
        }
        assert_eq!(total, 0, "16 ms of 4 ms wake-ups sit inside one 16.6 ms beat");
        let actions =
            driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(4) });
        assert_eq!(redraws(&actions), 1, "crossing the beat presents exactly once");
    }

    /// A long stall never queues a burst of presentations: one beat of debt, one redraw.
    #[test]
    fn a_stall_costs_one_redraw_not_a_burst() {
        let mut driver = WinitLoopDriver::new(60);
        driver.set_present_hz(60.0);
        let actions = driver
            .handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(500) });
        assert_eq!(redraws(&actions), 1, "a 500 ms stall presents once, not thirty times");
        // And the debt cap means the very next tiny wake-up owes at most one more beat.
        let actions =
            driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(1) });
        assert!(redraws(&actions) <= 1);
    }

    /// The suggested sleep never overshoots the earlier of the two deadlines (tick vs beat),
    /// so neither the sim nor the presentation can starve while the loop waits.
    #[test]
    fn the_suggested_wait_respects_both_deadlines() {
        let mut driver = WinitLoopDriver::new(60);
        driver.set_present_hz(60.0);
        driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(10) });
        let wait = driver.suggested_wait();
        assert!(wait <= Duration::from_secs_f64(1.0 / 60.0), "never sleeps past a beat: {wait:?}");
        assert!(wait > Duration::ZERO, "steady state always sleeps a little");
    }

    /// Displays above 120 Hz cap: the sim ticks at 60 and interpolates — beyond ~120 there is
    /// nothing new to present for the power burned. Broken refresh reports fall back sanely.
    #[test]
    fn present_rate_is_capped_and_survives_nonsense() {
        let mut driver = WinitLoopDriver::new(60);
        driver.set_present_hz(240.0);
        assert!(driver.suggested_wait() >= Duration::from_secs_f64(1.0 / 121.0));
        driver.set_present_hz(0.0);
        assert!(driver.suggested_wait() <= Duration::from_secs_f64(1.0 / 60.0 + 0.001));
    }
}
