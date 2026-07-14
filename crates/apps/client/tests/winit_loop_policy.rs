use std::time::Duration;

use client::{
    ClientLoopAction, ClientLoopEvent, ClientLoopPhase, FixedTickAccumulator, WinitLoopDriver,
};
use sim::DEFAULT_SIMULATION_TICK_HZ;

#[test]
fn client_loop_policy_is_winit_event_driven() {
    let phases = WinitLoopDriver::event_driven_phases();

    assert_eq!(
        phases,
        [
            ClientLoopPhase::WinitEvent,
            ClientLoopPhase::InputSystem,
            ClientLoopPhase::FixedTickAccumulator,
            ClientLoopPhase::RequestRedraw,
            ClientLoopPhase::RenderOnRedraw,
        ]
    );
    assert!(!WinitLoopDriver::uses_manual_event_polling());
}

#[test]
fn fixed_tick_accumulator_drains_whole_ticks_and_keeps_remainder() {
    let mut accumulator = FixedTickAccumulator::from_hz(60);

    let ticks = accumulator.accumulate(Duration::from_millis(34));

    assert_eq!(ticks, 2);
    assert!(accumulator.remainder() < accumulator.tick_duration());
}

#[test]
fn fixed_tick_accumulator_caps_catch_up_after_a_long_stall() {
    let mut accumulator = FixedTickAccumulator::from_hz(60);

    // A 5s stall would naively yield 300 ticks; it must be clamped and the backlog dropped.
    let ticks = accumulator.accumulate(Duration::from_secs(5));

    assert!(ticks <= FixedTickAccumulator::MAX_CATCHUP_TICKS);
    assert!(accumulator.remainder() < accumulator.tick_duration());
}

#[test]
fn about_to_wait_advances_fixed_ticks_and_requests_redraw() {
    let mut driver = WinitLoopDriver::new(60);

    let actions =
        driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(34) });

    assert_eq!(actions, [ClientLoopAction::RunFixedTicks(2), ClientLoopAction::RequestRedraw]);
}

#[test]
fn redraw_event_only_renders_prepared_frame() {
    let mut driver = WinitLoopDriver::new(60);

    let actions = driver.handle_event(ClientLoopEvent::RedrawRequested);

    assert_eq!(actions, [ClientLoopAction::RenderFrame]);
}

#[test]
fn render_alpha_reports_sub_tick_progress_for_interpolation() {
    let mut driver = WinitLoopDriver::new(60);

    // Fresh driver: no leftover time, so render on the tick boundary (alpha = 0).
    assert!(driver.render_alpha().abs() < 1.0e-6);

    // Half a 60 Hz tick (~8.33 ms) leaves the accumulator halfway into the next tick.
    driver.handle_event(ClientLoopEvent::AboutToWait {
        elapsed: Duration::from_secs_f64(0.5 / 60.0),
    });
    assert!((driver.render_alpha() - 0.5).abs() < 1.0e-3, "alpha = {}", driver.render_alpha());

    // Crossing a whole tick consumes it and wraps the remainder back below 1.0.
    driver.handle_event(ClientLoopEvent::AboutToWait {
        elapsed: Duration::from_secs_f64(0.5 / 60.0),
    });
    let alpha = driver.render_alpha();
    assert!((0.0..1.0).contains(&alpha), "alpha must stay in [0, 1): {alpha}");
}

#[test]
fn variable_render_cadence_accumulates_to_fixed_simulation_ticks() {
    let mut driver = WinitLoopDriver::new(DEFAULT_SIMULATION_TICK_HZ);

    // F1 pacing: the first two wake-ups sit inside one 60 Hz display beat (7+7 ms), so no
    // redraw yet; the third crosses BOTH a sim tick and the presentation beat.
    assert_eq!(
        driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(7) }),
        []
    );
    assert_eq!(
        driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(7) }),
        []
    );
    assert_eq!(
        driver.handle_event(ClientLoopEvent::AboutToWait { elapsed: Duration::from_millis(3) }),
        [ClientLoopAction::RunFixedTicks(1), ClientLoopAction::RequestRedraw]
    );

    assert_eq!(
        driver.handle_event(ClientLoopEvent::RedrawRequested),
        [ClientLoopAction::RenderFrame]
    );
}
