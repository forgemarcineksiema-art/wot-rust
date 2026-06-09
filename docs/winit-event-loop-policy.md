# winit Event Loop Policy

The desktop client is driven by `winit::event_loop::EventLoop::run_app()` and `ApplicationHandler`. The engine must not be shaped around a fake `poll_events() -> Iterator` loop.

## Flow

The client maps platform events into a small testable loop driver:

- `winit` event;
- input capture;
- fixed tick accumulator;
- redraw request;
- render on `RedrawRequested`.

The mental model can still be "game loop", but the implementation sits inside the platform event loop.

## Rules

- Do not write `while running { poll_events(); update(); render(); }`.
- Do not advance simulation directly inside `RedrawRequested`.
- `about_to_wait` computes elapsed time, feeds the fixed tick accumulator, runs whole simulation ticks, and requests redraw.
- `RedrawRequested` renders the current prepared frame.
- The elapsed time observed by `winit` is an accumulator input only; gameplay systems receive `FixedTimestep`.
- Window/input events are translated into client actions before touching simulation or renderer state.

`crates/client/src/loop_policy.rs` is intentionally independent from concrete `winit` types so this event-loop policy can be tested without opening a window.
