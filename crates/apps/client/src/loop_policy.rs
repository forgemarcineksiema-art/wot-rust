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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinitLoopDriver {
    accumulator: FixedTickAccumulator,
}

impl WinitLoopDriver {
    pub fn new(tick_hz: u32) -> Self {
        Self { accumulator: FixedTickAccumulator::from_hz(tick_hz) }
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
                actions.push(ClientLoopAction::RequestRedraw);
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
