//! Windows scheduler-timer resolution (Płynność 2.0 / F5). The frame pacer sleeps between
//! presentation beats with `ControlFlow::WaitUntil` — but Windows quantizes timed waits to
//! the system timer period, ~15.6 ms by default. A 16.6 ms beat then lands on 1, 2 or 3
//! timer ticks (16/31/47 ms → 60/30/20 FPS): exactly the oscillation the pacer was built to
//! remove. `timeBeginPeriod(1)` asks for 1 ms resolution for this process (per-process since
//! Windows 10 2004), making the pacer's sleeps land within a millisecond of the beat.
//!
//! This is the workspace's ONE unsafe block, quarantined behind a safe API: a single Win32
//! call with no pointers, no aliasing, no invariants beyond "the DLL exists".

/// Request 1 ms scheduler-timer resolution for this process. Returns whether the request was
/// applied (always `false` off Windows, where timed waits are already fine-grained). Windows
/// reverts the resolution automatically when the process exits.
pub fn request_millisecond_timer() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: `timeBeginPeriod` takes one integer and touches no memory; TIMERR_NOERROR
        // (0) signals success. The matching `timeEndPeriod` is deliberately skipped — the
        // request must live as long as the game loop, and the OS cleans up at process exit.
        unsafe { windows_sys::Win32::Media::timeBeginPeriod(1) == 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    /// On Windows the request must actually apply (the pacer's correctness depends on it);
    /// elsewhere it reports false and the pacer relies on the OS's native precision.
    #[test]
    fn the_millisecond_timer_request_applies_on_windows() {
        let applied = super::request_millisecond_timer();
        assert_eq!(applied, cfg!(windows));
    }
}
